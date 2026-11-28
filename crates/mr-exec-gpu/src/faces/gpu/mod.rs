//! The GPU sampler (`discover-gpu`): self-directed visibility discovery, entirely on the
//! GPU, drawn by the executor's face drawer. Assembled from components: the **storage**
//! (discovery rays, exposure tests and lit-mode shadow rays all read the composed storage,
//! group 2, and rays walk it with `refloor`), the **visibility** component `quadtree`
//! (the discovery below) and the **shading** component (`direct`, via `shade_surface`). Was the
//! discover-gpu renderer, which read the voxel pool only.
//!
//! Instead of one ray per pixel, an adaptive screen-space quadtree decides where to cast
//! rays: rays at the corners of `tile`-pixel tiles (corners shared, deduplicated); a tile
//! whose corner rays all hit the same face plane (`criterion=plane`) or the same face
//! (`criterion=face`) is accepted and every exposed block face of that plane inside the
//! tile's footprint is added to the visible-face set S; otherwise it splits into four, down
//! to 1 px where the pixel rays themselves are the samples. S lives in a GPU hash set of
//! world-space face keys and persists `keep_frames` frames. The face list (appended on
//! first insertion) is then rasterised like `gpu-raster-naive` (one instanced quad per
//! face, analytic UVs, ray-traced shadows in lit mode) with one indirect draw. Nothing is
//! read back except the counters (asynchronously, a few frames late).
//!
//! Per frame (all GPU, one submission):
//! 1. clear this frame's hash set; `survive`: re-insert last frame's faces younger than
//!    `keep_frames` that are still exposed (block solid, neighbour air; edits handled).
//! 2. per quadtree level L: `rays` (level 0: the whole coarse lattice, jittered per frame;
//!    later levels: the ray queue, indirect) store (cell, face, block, t) per lattice point;
//!    `classify` tests each tile: all-sky tiles <= `sky_tile` px are accepted; coherent
//!    tiles push a footprint job (bounding box of the corner hits on the plane, <=
//!    `max_footprint` cells); `footprint` expands each job with one workgroup (strict: every
//!    footprint cell must be an exposed face of that plane, else the tile is rejected);
//!    rejected all-hit tiles <= `closure_px` px whose corner hit cells fit a `closure_box`^3
//!    box add every camera-facing exposed face in that box (local closure); everything else
//!    splits (children + the 5 new lattice points, claimed once, appended to the next
//!    level's queues with one global atomic per workgroup). 1-px tiles insert their corner
//!    pixel faces. Small single-thread passes turn counters into indirect args.
//! 3. raster the list (depth pre-pass + equal-depth shading in lit mode).
//!
//! `audit` adds one ray per pixel after the timed passes and counts pixels whose visible
//! face is not in S.
//!
//! The quadtree's shaders are the components': the sampler
//! (`mr_components::visibility::quadtree`) and hole detection (`mr_components::holes`, the
//! `holes = coverage` stage, was `incremental`), two roots over the shared face set.
//!
//! Event-driven frames (`events`, with `holes = coverage`): the host classifies each frame.
//! *static* (camera pose unchanged, no world changes) skips discovery and redraws the last
//! face list; *still* (rotation and/or world changes, no translation) skips seen-marking
//! and the parallax test (with `still_holes = late` also the face-id raster, finding holes
//! a frame late); *moving* (translation) runs the incremental passes; *full* re-discovers
//! everything. World changes mark the screen tiles of edited blocks (exactly) and of
//! loaded chunks. Holes (uncovered pixels that were neither sky nor discovered) are told
//! apart from sky after the raster and re-discovered. The post's `view` shows why tiles
//! were discovered (`tiles`), where rays were cast (`rays`), which faces are new (`churn`),
//! the drawn quads (`quads`) and fragments per pixel (`overdraw`).

mod buffers;
mod init;
mod knobs;
mod passes;
mod region;
mod report;
#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::sync::atomic::AtomicU8;

use std::collections::HashMap;

use glam::IVec3;
use mr_core::*;
use mr_components::graph::{Checked, Role};
use mr_components::storage::gpu_data::Bound;
use mr_gpu::wgpu;

use super::Draw;
use crate::exec::{self, Cx, Node};

use buffers::Sized;
use knobs::{HolesKnobs, VisibilityKnobs};

pub(crate) fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu".into(),
        description: "GPU discovery + raster (the user's idea): a GPU adaptive quadtree of corner rays discovers the visible block faces (coherent tiles add a whole plane footprint), kept in a GPU hash set across frames and rasterised as one quad per face; no read-back. Rays, exposure tests and shadow rays read the composed storage".into(),
        tags: vec!["gpu".into(), "raster".into(), "raytrace".into(), "hybrid".into(), "modular".into(), "wgpu".into()],
        features: Features::of(&[Feature::Lit, Feature::Volume, Feature::Meshes, Feature::MeshRays]),
        params: REGION.lighting_knobs(knobs::params()),
    }
}

const MAX_TILE: u32 = 64;
const MAX_LEVELS: usize = 7;
const MAX_SLOTS: usize = 1 << 14;
const CHUNK_TABLE: usize = MAX_SLOTS * 4;

pub(crate) const REGION: &mr_components::graph::Builds = &mr_components::graph::GPU_SAMPLER;

const LEVEL_STRIDE: u64 = 256;
const ARGS_WORDS: u64 = 112;
const JOB_CAP: u64 = 262144;
const STATS_WORDS: u64 = 64;
const STAGES: usize = 8;
/// GPU timestamps per frame for the phase breakdown (begin, survive, L0, levels, raster).
const PROFILE_TS: u32 = 4 + 2 * MAX_LEVELS as u32;

/// What an event-driven frame has to do (see the module docs).
#[derive(Copy, Clone, PartialEq, Debug)]
enum FrameKind {
    Static,
    Still,
    Moving,
    Full,
}

struct Stage {
    buf: wgpu::Buffer,
    /// 0 free, 1 mapping, 2 mapped.
    state: Arc<AtomicU8>,
    /// (frame, parity) whose counters the buffer holds.
    tag: (u32, u32),
}

/// Bind groups of the storage instances: the discovery rays' at 2, the other sites' after
/// the deferred drawers' images (group 3).
const STORAGE_GROUPS: &[u32] = &[2, 4, 5, 6];

pub(crate) struct GpuSampler {
    vis: VisibilityKnobs,
    cover: HolesKnobs,
    /// The draw node's face order.
    order: String,
    /// Hole detection on: a `holes` node in the graph (`coverage`).
    incremental: bool,
    /// The sampler's passes without and with hole detection's tiles (shader def `HOLES`).
    pipe_survive: [wgpu::ComputePipeline; 2],
    pipe_rays: [wgpu::ComputePipeline; 2],
    pipe_classify: [wgpu::ComputePipeline; 2],
    pipe_audit: wgpu::ComputePipeline,
    pipe_level_args: wgpu::ComputePipeline,
    pipe_finalize: wgpu::ComputePipeline,
    pipe_job_args: wgpu::ComputePipeline,
    pipe_footprint: wgpu::ComputePipeline,
    pipe_mark: wgpu::ComputePipeline,
    pipe_need_raw: wgpu::ComputePipeline,
    pipe_need_final: wgpu::ComputePipeline,
    pipe_sky: wgpu::ComputePipeline,
    pipe_need_still: wgpu::ComputePipeline,
    pipe_sort_hist: wgpu::ComputePipeline,
    pipe_sort_scan: wgpu::ComputePipeline,
    pipe_sort_scatter: wgpu::ComputePipeline,
    /// None when drawing only (the shading on another GPU: painter order draws forward).
    pipe_paint: Option<wgpu::RenderPipeline>,
    /// Painter order: bucket counts / offsets, and this frame's list sorted far to near.
    hist: wgpu::Buffer,
    sorted: wgpu::Buffer,
    bind_sorted: wgpu::BindGroup,
    pipe_merge: wgpu::ComputePipeline,
    pipe_merge_args: wgpu::ComputePipeline,
    /// Row merge: this frame's runs (start key, length, block, fresh); count in lcount[2].
    merged: wgpu::Buffer,
    bind_merged: wgpu::BindGroup,
    /// The post's view of a quantity of the sampler's: its passes and their uniform.
    views: Option<report::ViewPasses>,
    view_buf: wgpu::Buffer,
    /// Frames of each kind so far (static, still, moving, full) and the last one's kind.
    kind_counts: [u64; 4],
    last_kind: FrameKind,
    /// Hole detection's face-id raster.
    pipe_id: wgpu::RenderPipeline,
    /// The sampler's raster module (painter order's fragments, the face-id and view rasters).
    draw_mod: wgpu::ShaderModule,
    layout_disc: wgpu::BindGroupLayout,
    params_buf: wgpu::Buffer,
    level_buf: wgpu::Buffer,
    stats: wgpu::Buffer,
    args: wgpu::Buffer,
    lcount: wgpu::Buffer,
    jobs: wgpu::Buffer,
    keys: [wgpu::Buffer; 2],
    vals: [wgpu::Buffer; 2],
    lists: [wgpu::Buffer; 2],
    ctable: wgpu::Buffer,
    cpos: wgpu::Buffer,
    /// Incremental mode: per previous-list entry, the frame it last covered a pixel.
    seen: wgpu::Buffer,
    /// World boxes (min block, size) changed since the last frame: edited blocks (size 1)
    /// and loaded / unloaded chunks (size 32).
    pending: Vec<(IVec3, i32)>,
    prev_pos: Option<glam::DVec3>,
    /// Previous camera (forward, right, up, tan of half fov) for sky reprojection.
    prev_cam: Option<([f32; 3], [f32; 3], [f32; 3], f32)>,
    /// Re-discover the whole frame next frame (settings changed, resize).
    force_full: bool,
    sized: Sized,
    bind_disc: Option<[wgpu::BindGroup; 2]>,
    bind_args: wgpu::BindGroup,
    bind_draw: [wgpu::BindGroup; 2],
    /// A gate on the drawer's faces in the hole-detection loop: the list and arguments
    /// that last crossed (hole detection draws them: the loop's first iteration), and the
    /// list's draw bind group.
    replay_faces: Option<(wgpu::Buffer, wgpu::Buffer, wgpu::BindGroup)>,
    stages: Vec<Stage>,
    profile: Option<wgpu::QuerySet>,
    profile_resolve: Option<wgpu::Buffer>,
    ts_period: f32,
    last_stats: Option<Vec<u32>>,
    last_parity: usize,
    /// The nodes of the timed stages: discovery's, then the drawer's.
    vis_nodes: Vec<String>,
    /// Hole detection's node (timed stages `holes` and `sky mask`).
    holes_nodes: Vec<String>,
    /// The drawer's node (timed stage `sort`).
    draw_ids: Vec<String>,
    slots: HashMap<IVec3, u16>,
    free_slots: Vec<u16>,
    slots_dirty: bool,
    chunk_probe: u32,
    table_cap: u32,
    list_cap: u32,
    frame: u32,
    rng: u64,
    drawing: Draw,
    /// The meshes, drawn into the drawer's depth (a meshes node; else never prepared).
    mesh: mr_gpu::meshes::GpuMeshes,
    /// This frame's state: its discovery settings, what its nodes did so far and its groups.
    now: Frame,
}

/// What a frame's nodes share.
#[derive(Default)]
struct Frame {
    /// A static frame: camera and world unchanged since the last discovery, so the last face
    /// list is still complete; only drawing and shading run.
    redraw: bool,
    stored: bool,
    opened: bool,
    holed: bool,
    discovered: bool,
    drawn: bool,
    parity: usize,
    /// The lattice's jitter and its level-0 tiles.
    off: (u32, u32),
    cols: u32,
    rows: u32,
    levels: usize,
    world: Option<wgpu::BindGroup>,
    bound: Option<Bound>,
    bind_disc: Option<wgpu::BindGroup>,
    out: Option<wgpu::TextureView>,
    /// This frame's counter/timestamp staging buffer.
    stage: Option<usize>,
    /// Meshes drawn this frame (asked for, and the graph has a meshes node).
    meshes: bool,
}

/// The storages discovery's and the shadow rays walk (after the acceleration structures).
struct Storage;
/// Hole detection: where last frame's faces leave the image uncovered.
struct Holes;
/// Discovery: the quadtree's passes into this frame's face set and list.
struct Visibility;
/// The face drawer's draw passes over the list.
struct DrawFaces;
/// The deferred drawers' shading pass.
struct Shading;

impl Node<GpuSampler> for Storage {
    fn run(&mut self, cx: &mut Cx<GpuSampler>) {
        cx.region.store(cx.core, cx.enc, cx.ticket, cx.req);
    }
}

impl Node<GpuSampler> for Holes {
    fn run(&mut self, cx: &mut Cx<GpuSampler>) {
        cx.region.holes(cx.core, cx.enc, cx.ticket, cx.req);
    }
}

impl Node<GpuSampler> for Visibility {
    fn run(&mut self, cx: &mut Cx<GpuSampler>) {
        cx.region.discover(cx.core, cx.enc, cx.ticket, cx.req);
    }
}

impl Node<GpuSampler> for DrawFaces {
    fn run(&mut self, cx: &mut Cx<GpuSampler>) {
        cx.region.draw(cx.core, cx.enc, cx.ticket, cx.req);
    }
}

impl Node<GpuSampler> for Shading {
    fn run(&mut self, cx: &mut Cx<GpuSampler>) {
        cx.region.shade(cx.core, cx.enc, cx.ticket, cx.req);
    }
}

/// The GPU discovery executor for `g`: storages, hole detection, discovery, drawing and
/// shading as nodes, each running what it reads first (the drawer's faces iterate through
/// hole detection and discovery: last frame's list, then this frame's).
pub(crate) fn create(p: &InitParams, devices: &Devices, waker: Waker, g: &Checked) -> anyhow::Result<Box<dyn Renderer>> {
    let mut passes = exec::passes(g, crate::home_slot(g), |role| -> Option<Box<dyn Node<GpuSampler>>> {
        Some(match role {
            Role::Storage => Box::new(Storage),
            Role::Holes => Box::new(Holes),
            Role::Visibility => Box::new(Visibility),
            Role::Draw => Box::new(DrawFaces),
            Role::Shading => Box::new(Shading),
            _ => return None,
        })
    })?;
    // The schedule puts the loop's consumer (the drawer) first; its first iteration is hole
    // detection's (last frame's faces), so each node's pass runs in the order its work does:
    // hole detection, discovery, then the drawer. The commands are the same either way (each
    // pass runs what it reads first); this way a gate between two of them crosses where the
    // edge's value is made.
    let role = |id: &str| g.node(id).role;
    if let Some(d) = passes.iter().position(|(id, _)| role(id) == Role::Draw) {
        let last = passes.iter().rposition(|(id, _)| matches!(role(id), Role::Holes | Role::Visibility)).filter(|&l| l > d);
        if let Some(l) = last {
            let draw = passes.remove(d);
            passes.insert(l, draw);
        }
    }
    crate::exec::run_or_split(GpuSampler::new(p, devices, waker, g)?, g, passes)
}
