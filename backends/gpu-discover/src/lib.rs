//! GPU-discover back end: self-directed visibility discovery, entirely on the GPU,
//! rendered by rasterisation. Assembled from components (IDEAS §12): the **storage**
//! (discovery rays, exposure tests and lit-mode shadow rays all read the composed storage,
//! group 2, and rays walk it with `refloor`), the **visibility** component `quadtree`
//! (the discovery below) and the **shading** component (`direct`, via `shade_surface`). Was the
//! gpu-discover renderer, which read the voxel pool only.
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
//! `audit` adds one ray per pixel after the timed passes and counts pixels whose visible
//! face is not in S.
//!
//! Event-driven frames (`events`, with `incremental`): the host classifies each frame.
//! *static* (camera pose unchanged, no world changes) skips discovery and redraws the last
//! face list; *still* (rotation and/or world changes, no translation) skips seen-marking
//! and the parallax test (with `still_holes = late` also the face-id raster, finding holes
//! a frame late); *moving* (translation) runs the incremental passes; *full* re-discovers
//! everything. World changes mark the screen tiles of edited blocks (exactly) and of
//! loaded chunks. Holes (uncovered pixels that were neither sky nor discovered) are told
//! apart from sky after the raster and re-discovered. `view` shows why tiles were
//! discovered (`tiles`), where rays were cast (`rays`) and which faces are new (`churn`).

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use glam::IVec3;
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_components::storage::gpu_data::{self, StorageData};
use mr_components::shading::state::{self as shade_state, ShadeState};
use mr_components::draw::drawer::{DrawFrame, DrawMode, DrawerSpec, FaceDrawer};
use mr_components::{draw, names_on, require, Slot, storage};
use mr_gpu::voxels::{Shadows, VoxelWorld, VoxelWorldConfig, hash3, shadows_param};
use mr_gpu::{FrameRing, Gpu, GpuNeeds, wgpu};
use serde::Deserialize;

pub fn entry() -> RendererEntry {
    RendererEntry { info, create: |p, d, w| Ok(Box::new(GpuDiscover::new(p, d, w)?)) }
}

fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu-discover".into(),
        description: "GPU-discover back end (storage, quadtree visibility, shading). Self-directed visibility: GPU adaptive quadtree of corner rays discovers the visible block faces (coherent tiles add a whole plane footprint), faces kept in a GPU hash set across frames, then rasterised as one quad per face; no read-back".into(),
        tags: vec!["gpu".into(), "raster".into(), "raytrace".into(), "hybrid".into(), "modular".into(), "wgpu".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: mr_components::lighting_knobs(BACKEND, params()),
    }
}

const MAX_TILE: u32 = 64;
const MAX_LEVELS: usize = 7;
const MAX_SLOTS: usize = 1 << 14;
const CHUNK_TABLE: usize = MAX_SLOTS * 4;

const BACKEND: &str = "gpu-discover";



fn params() -> Vec<ParamSpec> {
    vec![
        ParamSpec::choice("storage", "Storage component: what discovery rays, exposure tests and shadow rays read (walked with the traversal kernel)", &names_on(BACKEND, Slot::Storage), "bricks", false),
        ParamSpec::choice("visibility", "Visibility component", &names_on(BACKEND, Slot::Visibility), "quadtree", false),
        ParamSpec::choice("traversal", "Traversal kernel of the discovery, shadow and bounce rays", &names_on(BACKEND, Slot::Traversal), "refloor", false),
        ParamSpec::choice("shading", "Shading component", &names_on(BACKEND, Slot::Shading), "direct", false),
        ParamSpec::int("tile", "Initial tile size in pixels (power of two)", 2, MAX_TILE as i64, 8, true).of("visibility", &["quadtree"]),
        ParamSpec::choice("criterion", "Tile coherence: all corners hit the same face plane, or the very same face", &["plane", "face"], "plane", true).of("visibility", &["quadtree"]),
        ParamSpec::int("sky_tile", "Tiles whose corners all miss are accepted as sky at or below this size (px)", 1, MAX_TILE as i64, 4, true).of("visibility", &["quadtree"]),
        ParamSpec::int("keep_frames", "Frames a face stays in S without being re-discovered", 1, 1000, 8, true).of("visibility", &["quadtree"]),
        ParamSpec::int("max_footprint", "Max cells a coherent tile may add (larger footprints subdivide)", 1, 4096, 128, true).of("visibility", &["quadtree"]),
        ParamSpec::bool("strict", "Plane criterion: every footprint cell must be an exposed face of that plane, else subdivide", true, true).of("visibility", &["quadtree"]),
        ParamSpec::int("inline_fp", "Footprints of up to this many cells are expanded by the classifying thread; larger ones get a cooperative 64-thread workgroup (0 = always cooperative)", 0, 4096, 8, true).of("visibility", &["quadtree"]),
        ParamSpec::bool("morton", "Enumerate level-0 tiles and rays along a Z-curve (waves cover compact screen squares; children keep their parents' order) instead of row by row. Measured: no difference; deep levels are bound by each pass's slowest ray, not by coherence", false, true).of("visibility", &["quadtree"]),
        ParamSpec::int("closure_px", "Non-coherent tiles up to this size (px) whose corner hits fit a small box add all its exposed faces instead of splitting (0 = off)", 0, MAX_TILE as i64, 2, true).of("visibility", &["quadtree"]),
        ParamSpec::int("closure_box", "Local-closure box size in cells per axis", 1, 8, 2, true).of("visibility", &["quadtree"]),
        ParamSpec::bool("block_faces", "With each discovered face also add its block's other camera-facing exposed faces (closes raster-vs-ray edge cracks)", true, true).of("visibility", &["quadtree"]),
        ParamSpec::bool("jitter", "Shift the tile lattice randomly every frame (temporal coverage)", true, true).of("visibility", &["quadtree"]),
        ParamSpec::bool("incremental", "Rasterise last frame's faces first and only discover tiles with uncovered pixels, parallax-revealable occlusion edges, edited blocks or changed chunks, or their refresh turn (jitter off)", false, true).of("visibility", &["quadtree"]),
        ParamSpec::int("keep_unseen", "Incremental: frames a face inside the view stays in S without covering a pixel (faces outside the view use keep_frames)", 1, 1000, 64, true).of("visibility", &["quadtree"]),
        ParamSpec::int("refresh", "Incremental: re-discover every Nth tile each frame regardless (0 = never)", 0, 1024, 16, true).of("visibility", &["quadtree"]),
        ParamSpec::float("parallax_px", "Incremental: an occlusion edge needs discovery if the camera's translation can reveal more than this many pixels behind it", 0.05, 16.0, false, 0.5, true).of("visibility", &["quadtree"]),
        ParamSpec::bool("events", "Incremental: classify frames by what changed. Unchanged camera and world: skip discovery and redraw (frame_kind 0). Rotation or world changes only: skip seen-marking and the parallax test (1). Translation: the full incremental passes (2). Edits mark only the edited blocks' screen tiles", true, true).of("visibility", &["quadtree"]),
        ParamSpec::choice("merge", "Faces drawn as: none = one quad per face; rows = runs of same-block faces along each face's u axis merged into one quad (one GPU pass over S; ignored with order = painter)", &["none", "rows"], "none", true).of("visibility", &["quadtree"]),
        ParamSpec::choice("order", "Face drawing order. depth: any order, depth-tested (lit mode adds a depth pre-pass). painter: sorted back to front by Manhattan distance from the camera cell (an exact order for voxels; GPU counting sort), drawn without depth testing, so every fragment is shaded", &["depth", "painter"], "depth", true).of("draw", &["raster"]),
        ParamSpec::choice("still_holes", "Events, rotation-only frames: now = rasterise the old face list first and discover holes (faces that rotation moved onto a pixel centre) in the same frame; late = skip that raster and discover them next frame (cheaper, holes visible for one frame)", &["now", "late"], "now", true).of("visibility", &["quadtree"]),
        ParamSpec::choice("view", "image: the rendered scene. quads: one colour per drawn quad (shows merging). overdraw: fragments shaded per pixel on a colour scale (linear; white above view_max; compare order = depth vs painter). tiles: level-0 tiles tinted by why they were discovered this frame (white full, blue edited block / changed chunk, cyan entering strip (still_holes = late), red uncovered or hole, yellow parallax, green refresh; dark = skipped). rays: a dot per ray cast this frame. churn: faces new to the set this frame in red, carried-over faces grey", &["image", "tiles", "rays", "churn", "overdraw", "quads"], "image", true).of("visibility", &["quadtree"]),
        ParamSpec::int("view_max", "overdraw view: the top of the colour scale (fragments per pixel)", 1, 255, 8, true).of("visibility", &["quadtree"]),
        ParamSpec::int("reveal_tiles", "Incremental: largest reveal radius around an edge, in level-0 tiles", 1, 16, 4, true).of("visibility", &["quadtree"]),
        ParamSpec::bool("audit", "Also cast one ray per pixel and count pixels whose face is missing from S", false, true).of("visibility", &["quadtree"]),
        shadows_param(),
        ParamSpec::int("max_steps", "DDA step limit", 32, 16384, 2048, true),
        ParamSpec::choice("draw", "Draw component (the shared face drawer): raster = depth-tested and shaded per fragment (order and the overdraw view apply); visbuffer = face ids, then one compute pass shades each pixel; gbuffer = hit points and blocks, then the same (no diagnostic tints)", &names_on(BACKEND, Slot::Draw), "raster", true),
        ParamSpec::bool("depth_prepass", "Lit mode, draw = raster: depth pre-pass so lighting runs once per pixel", true, true),
        ParamSpec::int("max_faces", "Capacity of the face set S (faces)", 1 << 16, 1 << 24, 1 << 20, false).of("visibility", &["quadtree"]),
        ParamSpec::int("pool_mb", "Voxel pool limit (MiB); for region storages the limit of their own data", 64, 6144, 1536, false),
        ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
    ]
    .into_iter()
    .chain(shade_state::params())
    .collect()
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    storage: String,
    visibility: String,
    traversal: String,
    shading: String,
    tile: u32,
    criterion: String,
    sky_tile: u32,
    keep_frames: u32,
    max_footprint: u32,
    strict: bool,
    inline_fp: u32,
    morton: bool,
    closure_px: u32,
    closure_box: u32,
    block_faces: bool,
    jitter: bool,
    incremental: bool,
    keep_unseen: u32,
    refresh: u32,
    parallax_px: f32,
    reveal_tiles: u32,
    events: bool,
    order: String,
    merge: String,
    still_holes: String,
    view: String,
    view_max: u32,
    audit: bool,
    shadows: Shadows,
    max_steps: u32,
    draw: String,
    depth_prepass: bool,
    bounces: u32,
    spp: u32,
    accumulate: bool,
    max_faces: u32,
    pool_mb: u32,
    frames_in_flight: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            storage: "bricks".into(),
            visibility: "quadtree".into(),
            traversal: "refloor".into(),
            shading: "direct".into(),
            tile: 8,
            criterion: "plane".into(),
            sky_tile: 4,
            keep_frames: 8,
            max_footprint: 128,
            strict: true,
            inline_fp: 8,
            morton: false,
            closure_px: 2,
            closure_box: 2,
            block_faces: true,
            jitter: true,
            incremental: false,
            keep_unseen: 64,
            refresh: 16,
            parallax_px: 0.5,
            reveal_tiles: 4,
            events: true,
            order: "depth".into(),
            merge: "none".into(),
            still_holes: "now".into(),
            view: "image".into(),
            view_max: 8,
            audit: false,
            shadows: Shadows::All,
            max_steps: 2048,
            draw: "raster".into(),
            depth_prepass: true,
            bounces: 1,
            spp: 1,
            accumulate: true,
            max_faces: 1 << 20,
            pool_mb: 1536,
            frames_in_flight: 3,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct FrameUniform {
    view_proj: [[f32; 4]; 4],
    cam_frac: [f32; 3],
    mode: u32,
    forward: [f32; 3],
    tan_half_fov: f32,
    right: [f32; 3],
    aspect: f32,
    up: [f32; 3],
    view: u32,
    size: [f32; 2],
    _p1: [f32; 2],
    /// Deferred drawing: the colour of uncovered pixels.
    sky: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Params {
    forward: [f32; 3],
    tan_half_fov: f32,
    right: [f32; 3],
    aspect: f32,
    up: [f32; 3],
    frame: u32,
    origin: [f32; 3],
    tile: u32,
    width: u32,
    height: u32,
    off_x: u32,
    off_y: u32,
    lat_w: u32,
    lat_h: u32,
    cols: u32,
    rows: u32,
    keep: u32,
    criterion: u32,
    sky_tile: u32,
    max_fp: u32,
    table_mask: u32,
    list_cap: u32,
    chunk_mask: u32,
    chunk_probe: u32,
    queue_cap: u32,
    parity: u32,
    strict: u32,
    still: u32,
    closure_px: u32,
    closure_box: i32,
    block_faces: u32,
    inline_fp: u32,
    incr: u32,
    refresh: u32,
    par_k: f32,
    changed: u32,
    full: u32,
    r_max: u32,
    need_off: u32,
    par_thresh: f32,
    prev_forward: [f32; 3],
    prev_tan: f32,
    prev_right: [f32; 3],
    has_prev: u32,
    prev_up: [f32; 3],
    keep_out: u32,
    morton: u32,
    morton_side: u32,
    _q: [u32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct LevelUniform {
    level: u32,
    size: u32,
    in_off: u32,
    out_off: u32,
    parity: u32,
    cap: u32,
    list_cap: u32,
    _p: u32,
    _pad: [u32; 56],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct OverlayUniform {
    view: u32,
    frame: u32,
    tile: u32,
    cols: u32,
    rows: u32,
    need_off: u32,
    lat_w: u32,
    off_x: u32,
    off_y: u32,
    view_max: u32,
    _p: [u32; 2],
}

/// What an event-driven frame has to do (see the module docs).
#[derive(Copy, Clone, PartialEq, Debug)]
enum FrameKind {
    Static,
    Still,
    Moving,
    Full,
}

/// `view` knob: 0 image, 1 churn (raster tint), 2 tiles, 3 rays (overlays).
fn view_code(v: &str) -> u32 {
    match v {
        "churn" => 1,
        "tiles" => 2,
        "rays" => 3,
        "overdraw" => 4,
        "quads" => 5,
        _ => 0,
    }
}

const LEVEL_STRIDE: u64 = 256;
const ARGS_WORDS: u64 = 112;
const JOB_CAP: u64 = 262144;
const STATS_WORDS: u64 = 64;
const STAGES: usize = 8;
/// GPU timestamps per frame for the phase breakdown (begin, survive, L0, levels, raster).
const PROFILE_TS: u32 = 4 + 2 * MAX_LEVELS as u32;

/// Per-resolution buffers.
struct Sized {
    lat_w: u32,
    lat_h: u32,
    cache_c: wgpu::Buffer,
    cache_t: wgpu::Buffer,
    claim: wgpu::Buffer,
    /// Per lattice point: the frame its ray was last cast (`rays` view).
    cast_frame: wgpu::Buffer,
    rayq: wgpu::Buffer,
    tileq: wgpu::Buffer,
    output: wgpu::Texture,
    /// `view = overdraw`: a copy of the counts (red, in 1/255 steps) the overlay ramps.
    counts: wgpu::Texture,
    depth: wgpu::TextureView,
    /// Incremental mode: last frame's faces rasterised at the current camera (list index + 1).
    idtex: wgpu::TextureView,
    /// Per level-0 tile: reveal radius + 1, then (from `need_off`) why it was discovered
    /// (0 = not), then (from `2 * need_off`) frames since the tile was last discovered, then
    /// (from `3 * need_off`) a hole seen after the last raster.
    need: wgpu::Buffer,
    need_off: u32,
    /// Incremental mode: per level-0 tile, 1 if an edited block or changed chunk covers it.
    changed_tiles: wgpu::Buffer,
    /// Per pixel, 1 where the final raster was sky; double-buffered by frame parity.
    sky: [wgpu::Buffer; 2],
}

struct Stage {
    buf: wgpu::Buffer,
    /// 0 free, 1 mapping, 2 mapped.
    state: Arc<AtomicU8>,
    /// (frame, parity) whose counters the buffer holds.
    tag: (u32, u32),
}

pub struct GpuDiscover {
    gpu: Arc<Gpu>,
    world: VoxelWorld,
    /// The storage's own data (group 2 of the discovery and raster passes).
    data: StorageData,
    layout_st: wgpu::BindGroupLayout,
    /// This frame's group 2 (set in `render`).
    bind_st: Option<wgpu::BindGroup>,
    opts: Options,
    pipe_survive: wgpu::ComputePipeline,
    pipe_rays: wgpu::ComputePipeline,
    pipe_classify: wgpu::ComputePipeline,
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
    pipe_paint: wgpu::RenderPipeline,
    pipe_over_depth: wgpu::RenderPipeline,
    pipe_over_paint: wgpu::RenderPipeline,
    /// Painter order: bucket counts / offsets, and this frame's list sorted far to near.
    hist: wgpu::Buffer,
    sorted: wgpu::Buffer,
    bind_sorted: wgpu::BindGroup,
    pipe_merge: wgpu::ComputePipeline,
    pipe_merge_args: wgpu::ComputePipeline,
    /// Row merge: this frame's runs (start key, length, block, fresh); count in lcount[2].
    merged: wgpu::Buffer,
    bind_merged: wgpu::BindGroup,
    pipe_overlay: wgpu::RenderPipeline,
    layout_ov: wgpu::BindGroupLayout,
    overlay_buf: wgpu::Buffer,
    bind_ov: Option<wgpu::BindGroup>,
    /// Frames of each kind so far (static, still, moving, full) and the last one's kind.
    kind_counts: [u64; 4],
    last_kind: FrameKind,
    pipe_id: wgpu::RenderPipeline,
    /// The face drawer (`draw`: the raster visibility component's pipelines); painter
    /// order and the overdraw view replace its forward pipeline.
    drawer: FaceDrawer,
    /// The face source and everything but the visibility component (for a draw change),
    /// compiled against `program` (the walk's modules) with `deps`.
    draw_src: String,
    program: mr_gpu::program::Program,
    deps: Vec<String>,
    pl_draw: wgpu::PipelineLayout,
    pl_shade: wgpu::PipelineLayout,
    images: wgpu::BindGroupLayout,
    layout_disc: wgpu::BindGroupLayout,
    params_buf: wgpu::Buffer,
    level_buf: wgpu::Buffer,
    frame_buf: wgpu::Buffer,
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
    layout_draw: wgpu::BindGroupLayout,
    /// Shading state (pathtrace's accumulation).
    accum: ShadeState,
    shading: String,
    stages: Vec<Stage>,
    profile: Option<wgpu::QuerySet>,
    profile_resolve: Option<wgpu::Buffer>,
    ts_period: f32,
    last_stats: Option<Vec<u32>>,
    last_parity: usize,
    ring: FrameRing,
    slots: HashMap<IVec3, u16>,
    free_slots: Vec<u16>,
    slots_dirty: bool,
    slot_overflow: u64,
    chunk_probe: u32,
    table_cap: u32,
    list_cap: u32,
    frame: u32,
    rng: u64,
    width: u32,
    height: u32,
    waker: Waker,
}

fn storage(gpu: &Gpu, label: &str, size: u64, extra: wgpu::BufferUsages) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size.max(16),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | extra,
        mapped_at_creation: false,
    })
}

fn depth_view(gpu: &Gpu, w: u32, h: u32) -> wgpu::TextureView {
    gpu.device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

fn tiles_max(lat_w: u32, lat_h: u32) -> u32 {
    (lat_w / 2 + 1) * (lat_h / 2 + 1)
}

fn make_sized(gpu: &Gpu, w: u32, h: u32) -> Sized {
    // Lattice big enough for any tile size and jitter offset.
    let lat_w = w + 2 * MAX_TILE + 1;
    let lat_h = h + 2 * MAX_TILE + 1;
    let n = lat_w as u64 * lat_h as u64;
    let none = wgpu::BufferUsages::empty();
    Sized {
        lat_w,
        lat_h,
        cache_c: storage(gpu, "ray cache cells", n * 16, none),
        cache_t: storage(gpu, "ray cache t", n * 4, none),
        claim: storage(gpu, "ray claims", n * 4, none),
        cast_frame: storage(gpu, "ray cast frames", n * 4, none),
        rayq: storage(gpu, "ray queue", n * 4, none),
        tileq: storage(gpu, "tile queues", 2 * n * 4, none),
        output: mr_gpu::output_texture(gpu, w, h),
        counts: gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("overdraw counts"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        }),
        depth: depth_view(gpu, w, h),
        idtex: gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("face ids"),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Uint,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default()),
        // Tiles are at least 2 px, so the lattice has at most (lat_w/2) x (lat_h/2) tiles.
        need: storage(gpu, "tile needs", 4 * tiles_max(lat_w, lat_h) as u64 * 4, none),
        need_off: tiles_max(lat_w, lat_h),
        changed_tiles: storage(gpu, "changed tiles", tiles_max(lat_w, lat_h) as u64 * 4, none),
        sky: [0, 1].map(|_| storage(gpu, "sky mask", w as u64 * h as u64 * 4, none)),
    }
}

impl GpuDiscover {
    pub fn new(p: InitParams, devices: &Devices, waker: Waker) -> anyhow::Result<Self> {
        let opts: Options = p.options()?;
        let st = storage::gpu(&require(BACKEND, Slot::Storage, &opts.storage)?).expect("validated");
        require(BACKEND, Slot::Visibility, &opts.visibility)?;
        require(BACKEND, Slot::Traversal, &opts.traversal)?;
        require(BACKEND, Slot::Draw, &opts.draw)?;
        let shading = require(BACKEND, Slot::Shading, &opts.shading)?;
        // The host hands the device in (GPU slot 0); this back end only states its needs.
        let gpu = mr_gpu::gpu(devices, 0, &GpuNeeds { experimental: false, required_features: wgpu::Features::empty(), optional_features: wgpu::Features::TIMESTAMP_QUERY | shade_state::FRAGMENT_FEATURE })?;
        let world = VoxelWorld::new(
            &gpu,
            &p.blocks,
            p.environment,
            VoxelWorldConfig {
                max_lod: 0,
                // Region storages (tree64, ...) hold their own data instead of the voxel pool.
                upload_voxels: matches!(st.kind, storage::Kind::Pool { .. }),
                pool_mb: opts.pool_mb,
                view_radius: p.view_radius_chunks,
                shadows: opts.shadows,
                max_steps: opts.max_steps,
                visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::VERTEX_FRAGMENT,
            },
        )?;
        let dev = &gpu.device;
        let module = |label, src: String| mr_gpu::shader(dev, label, src);
        // The walk's modules (`walk::`): the composed storage at group 2 and the kernel.
        let (program, walk) = mr_components::shaders::walk_program(&st, &opts.traversal, false);
        let deps: Vec<String> = mr_components::shaders::root_deps(&walk, &[]).into_iter().map(String::from).collect();
        let modules = |label, root: &str| mr_gpu::program::compile(dev, label, &program, root, &deps.iter().map(String::as_str).collect::<Vec<_>>());
        // Discovery and raster read the world through the composed storage, walked with
        // refloor (rays and shadow rays).
        let layout_st = gpu_data::layout(&gpu, &st, wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::VERTEX_FRAGMENT);
        let data = StorageData::new(&gpu, st, gpu_data::Limits { mb: opts.pool_mb, view_radius: p.view_radius_chunks, wide_ids: p.blocks.blocks.len() > 256 });
        let disc_mod = modules("discover", include_str!("discover.wgsl"));
        let args_mod = module("discover args", include_str!("args.wgsl").to_string());
        let shade = mr_components::shading::gpu_wgsl(&shading).expect("validated");
        if let Some(why) = shade_state::fragment_blocker(&shading, &gpu.device) {
            anyhow::bail!(why);
        }
        // The face source and everything but the visibility component (the drawer's `draw`).
        let draw_src = format!("{}\n{shade}\n{}\n{}", ShadeState::wgsl(0, 3), include_str!("raster.wgsl"), draw::faces_wgsl(true));
        let draw_mod = modules("discover raster", &format!("{draw_src}\n{}", shade_state::fragment_wgsl(&shading, draw::gpu_wgsl("raster").unwrap())));

        let buf = |binding, ty, vis, dynamic| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: vis,
            ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: dynamic, min_binding_size: None },
            count: None,
        };
        let c = wgpu::ShaderStages::COMPUTE;
        let uni = wgpu::BufferBindingType::Uniform;
        let rw = wgpu::BufferBindingType::Storage { read_only: false };
        let ro = wgpu::BufferBindingType::Storage { read_only: true };
        let mut disc_entries = vec![buf(0, uni, c, false), buf(1, uni, c, true)];
        for b in 2..=10 {
            disc_entries.push(buf(b, rw, c, false));
        }
        for b in 11..=14 {
            disc_entries.push(buf(b, ro, c, false));
        }
        disc_entries.push(buf(15, rw, c, false));
        disc_entries.push(buf(16, rw, c, false));
        disc_entries.push(buf(17, rw, c, false));
        disc_entries.push(buf(18, rw, c, false));
        let tex = |binding, sample_type| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: c,
            ty: wgpu::BindingType::Texture { sample_type, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        disc_entries.push(tex(19, wgpu::TextureSampleType::Uint));
        disc_entries.push(tex(20, wgpu::TextureSampleType::Depth));
        disc_entries.push(buf(21, ro, c, false));
        disc_entries.push(buf(22, rw, c, false));
        disc_entries.push(buf(23, ro, c, false));
        disc_entries.push(buf(24, rw, c, false));
        disc_entries.push(buf(25, rw, c, false));
        disc_entries.push(buf(26, rw, c, false));
        disc_entries.push(buf(27, rw, c, false));
        let layout_disc = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("discover"), entries: &disc_entries });
        let layout_args = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("discover args"),
            entries: &[buf(0, uni, c, true), buf(1, ro, c, false), buf(2, rw, c, false), buf(3, ro, c, false)],
        });
        let vf = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let layout_draw = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("discover draw"),
            entries: &[
                buf(0, uni, vf | c, false),
                buf(1, ro, vf | c, false),
                buf(2, ro, vf | c, false),
                ShadeState::layout_entries(3, wgpu::ShaderStages::FRAGMENT | c)[0],
                ShadeState::layout_entries(3, wgpu::ShaderStages::FRAGMENT | c)[1],
            ],
        });
        let pl_disc = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("discover"), bind_group_layouts: &[Some(&layout_disc), Some(world.layout()), Some(&layout_st)], immediate_size: 0 });
        let pl_args = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("discover args"), bind_group_layouts: &[Some(&layout_args)], immediate_size: 0 });
        let compute = |pl: &wgpu::PipelineLayout, m: &wgpu::ShaderModule, entry: &str| {
            dev.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(pl),
                module: m,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let pipe_survive = compute(&pl_disc, &disc_mod, "survive");
        let pipe_rays = compute(&pl_disc, &disc_mod, "rays");
        let pipe_classify = compute(&pl_disc, &disc_mod, "classify");
        let pipe_audit = compute(&pl_disc, &disc_mod, "audit");
        let pipe_level_args = compute(&pl_args, &args_mod, "level_args");
        let pipe_finalize = compute(&pl_args, &args_mod, "finalize");
        let pipe_job_args = compute(&pl_args, &args_mod, "job_args");
        let pipe_footprint = compute(&pl_disc, &disc_mod, "footprint");
        let pipe_mark = compute(&pl_disc, &disc_mod, "mark");
        let pipe_need_raw = compute(&pl_disc, &disc_mod, "need_raw");
        let pipe_need_final = compute(&pl_disc, &disc_mod, "need_final");
        let pipe_sky = compute(&pl_disc, &disc_mod, "sky_update");
        let pipe_need_still = compute(&pl_disc, &disc_mod, "need_still");
        let pipe_sort_hist = compute(&pl_disc, &disc_mod, "sort_hist");
        let pipe_sort_scan = compute(&pl_disc, &disc_mod, "sort_scan");
        let pipe_sort_scatter = compute(&pl_disc, &disc_mod, "sort_scatter");
        let pipe_merge = compute(&pl_disc, &disc_mod, "merge_rows");
        let pipe_merge_args = compute(&pl_args, &args_mod, "merge_args");
        // Diagnostic overlays (`view = tiles | rays`), alpha-blended over the image.
        let ov_mod = module("discover overlay", format!("{}{}", mr_core::legend::ramp_wgsl(), include_str!("overlay.wgsl")));
        let f = wgpu::ShaderStages::FRAGMENT;
        let counts_entry = wgpu::BindGroupLayoutEntry {
            binding: 3,
            visibility: f,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let layout_ov = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("discover overlay"),
            entries: &[buf(0, uni, f, false), buf(1, ro, f, false), buf(2, ro, f, false), counts_entry],
        });
        let pl_ov = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("discover overlay"), bind_group_layouts: &[Some(&layout_ov)], immediate_size: 0 });
        let pipe_overlay = dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("discover overlay"),
            layout: Some(&pl_ov),
            vertex: wgpu::VertexState { module: &ov_mod, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &ov_mod,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba8Unorm, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let pl_draw = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("discover draw"), bind_group_layouts: &[Some(&layout_draw), Some(world.layout()), Some(&layout_st)], immediate_size: 0 });
        let pipeline = |label, write: bool, compare, fragment: bool| {
            let targets = [Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba8Unorm, blend: None, write_mask: wgpu::ColorWrites::ALL })];
            dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pl_draw),
                vertex: wgpu::VertexState { module: &draw_mod, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(write),
                    depth_compare: Some(compare),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: fragment.then(|| wgpu::FragmentState { module: &draw_mod, entry_point: Some("fs"), compilation_options: Default::default(), targets: &targets }),
                multiview_mask: None,
                cache: None,
            })
        };
        let images = FaceDrawer::images_layout(dev);
        let pl_shade = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("discover deferred shade"),
            bind_group_layouts: &[Some(&layout_draw), Some(world.layout()), Some(&layout_st), Some(&images)],
            immediate_size: 0,
        });
        let drawer = Self::drawer(&gpu, &program, &deps, &draw_src, &shading, &opts.draw, &pl_draw, &pl_shade, &images, p.width, p.height)?;
        // Painter order: no depth test, but depth is still written (the last fragment is the
        // nearest), since incremental mode reads it.
        let pipe_paint = pipeline("painter", true, wgpu::CompareFunction::Always, true);
        let overdraw = |label, compare| {
            let targets = [Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: Some(wgpu::BlendState { color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add }, alpha: wgpu::BlendComponent::OVER }),
                write_mask: wgpu::ColorWrites::ALL,
            })];
            dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pl_draw),
                vertex: wgpu::VertexState { module: &draw_mod, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
                primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, front_face: wgpu::FrontFace::Ccw, cull_mode: Some(wgpu::Face::Back), ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState { format: wgpu::TextureFormat::Depth32Float, depth_write_enabled: Some(true), depth_compare: Some(compare), stencil: Default::default(), bias: Default::default() }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState { module: &draw_mod, entry_point: Some("fs_overdraw"), compilation_options: Default::default(), targets: &targets }),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipe_over_depth = overdraw("overdraw depth", wgpu::CompareFunction::Greater);
        let pipe_over_paint = overdraw("overdraw painter", wgpu::CompareFunction::Always);
        let id_targets = [Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::R32Uint, blend: None, write_mask: wgpu::ColorWrites::ALL })];
        let pipe_id = dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("face ids"),
            layout: Some(&pl_draw),
            vertex: wgpu::VertexState { module: &draw_mod, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, front_face: wgpu::FrontFace::Ccw, cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Greater),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &draw_mod, entry_point: Some("fs_id"), compilation_options: Default::default(), targets: &id_targets }),
            multiview_mask: None,
            cache: None,
        });

        let list_cap = opts.max_faces.clamp(1 << 16, 1 << 24);
        let table_cap = (list_cap * 2).next_power_of_two();
        let none = wgpu::BufferUsages::empty();
        let uniform = |label, size| dev.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let params_buf = uniform("discover params", size_of::<Params>() as u64);
        let level_buf = uniform("discover levels", LEVEL_STRIDE * (MAX_LEVELS as u64 + 1));
        let frame_buf = uniform("frame", size_of::<FrameUniform>() as u64);
        let overlay_buf = uniform("discover overlay", size_of::<OverlayUniform>() as u64);
        let stats = storage(&gpu, "discover counters", STATS_WORDS * 4, wgpu::BufferUsages::COPY_SRC);
        let args = storage(&gpu, "discover args", ARGS_WORDS * 4, wgpu::BufferUsages::INDIRECT);
        let lcount = storage(&gpu, "list counts", 16, wgpu::BufferUsages::COPY_SRC);
        let keys = [0, 1].map(|_| storage(&gpu, "face set keys", table_cap as u64 * 4, none));
        let vals = [0, 1].map(|_| storage(&gpu, "face set frames", table_cap as u64 * 4, none));
        let lists = [0, 1].map(|_| storage(&gpu, "face lists", list_cap as u64 * 16, none));
        let hist = storage(&gpu, "sort buckets", 2048 * 4, none);
        let merged = storage(&gpu, "merged quads", list_cap as u64 * 16, none);
        let sorted = storage(&gpu, "sorted faces", list_cap as u64 * 16, none);
        let ctable = storage(&gpu, "chunk slot table", CHUNK_TABLE as u64 * 16, none);
        gpu.queue.write_buffer(&ctable, 0, bytemuck::cast_slice(&vec![[0i32, 0, 0, -1]; CHUNK_TABLE]));
        let cpos = storage(&gpu, "slot chunk positions", MAX_SLOTS as u64 * 16, none);
        let accum = ShadeState::new(&gpu, p.width, p.height, &shading, opts.bounces, opts.spp, opts.accumulate);
        let bind_merged = draw_bind(&gpu, &layout_draw, &frame_buf, &merged, &cpos, &accum);
        let bind_sorted = draw_bind(&gpu, &layout_draw, &frame_buf, &sorted, &cpos, &accum);
        let bind_args = dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("discover args"),
            layout: &layout_args,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &level_buf, offset: 0, size: wgpu::BufferSize::new(32) }) },
                wgpu::BindGroupEntry { binding: 1, resource: stats.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: args.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: lcount.as_entire_binding() },
            ],
        });
        let bind_draw = [0, 1].map(|i| draw_bind(&gpu, &layout_draw, &frame_buf, &lists[i], &cpos, &accum));
        let stages = (0..STAGES)
            .map(|_| Stage {
                buf: dev.create_buffer(&wgpu::BufferDescriptor { label: Some("counter stage"), size: STATS_WORDS * 4 + 16 + PROFILE_TS as u64 * 8, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false }),
                state: Arc::new(AtomicU8::new(0)),
                tag: (0, 0),
            })
            .collect();
        let depth = opts.frames_in_flight.max(1);
        let sized = make_sized(&gpu, p.width, p.height);
        Ok(Self {
            data,
            layout_st,
            bind_st: None,
            ring: FrameRing::new(&gpu, p.width, p.height, depth, waker.clone()),
            sized,
            world,
            opts,
            pipe_survive,
            pipe_rays,
            pipe_classify,
            pipe_audit,
            pipe_level_args,
            pipe_finalize,
            pipe_job_args,
            pipe_footprint,
            pipe_mark,
            pipe_need_raw,
            pipe_need_final,
            pipe_sky,
            pipe_need_still,
            pipe_sort_hist,
            pipe_sort_scan,
            pipe_sort_scatter,
            pipe_paint,
            pipe_over_depth,
            pipe_over_paint,
            hist,
            sorted,
            bind_sorted,
            pipe_merge,
            pipe_merge_args,
            merged,
            bind_merged,
            pipe_overlay,
            layout_ov,
            overlay_buf,
            bind_ov: None,
            kind_counts: [0; 4],
            last_kind: FrameKind::Full,
            pipe_id,
            drawer,
            draw_src,
            program,
            deps,
            pl_draw,
            pl_shade,
            images,
            layout_disc,
            params_buf,
            level_buf,
            frame_buf,
            stats,
            args,
            lcount,
            jobs: storage(&gpu, "footprint jobs", JOB_CAP * 32, wgpu::BufferUsages::empty()),
            keys,
            vals,
            lists,
            ctable,
            cpos,
            seen: storage(&gpu, "seen", list_cap as u64 * 4, none),
            pending: vec![],
            prev_pos: None,
            prev_cam: None,
            force_full: true,
            bind_disc: None,
            bind_args,
            bind_draw,
            layout_draw,
            accum,
            shading,
            profile: gpu.timestamps.then(|| dev.create_query_set(&wgpu::QuerySetDescriptor { label: Some("discover phases"), ty: wgpu::QueryType::Timestamp, count: STAGES as u32 * PROFILE_TS })),
            profile_resolve: gpu.timestamps.then(|| {
                dev.create_buffer(&wgpu::BufferDescriptor { label: Some("phase ts"), size: STAGES as u64 * 256, usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false })
            }),
            ts_period: gpu.queue.get_timestamp_period(),
            stages,
            last_stats: None,
            last_parity: 0,
            slots: HashMap::new(),
            free_slots: (0..MAX_SLOTS as u32).rev().map(|s| s as u16).collect(),
            slots_dirty: false,
            slot_overflow: 0,
            chunk_probe: 0,
            table_cap,
            list_cap,
            frame: 0,
            rng: 0x9e3779b97f4a7c15,
            width: p.width,
            height: p.height,
            waker,
            gpu,
        })
    }

    fn make_bind_disc(&self) -> [wgpu::BindGroup; 2] {
        let s = &self.sized;
        [0usize, 1].map(|par| {
            let prev = 1 - par;
            fn res(b: &wgpu::Buffer) -> wgpu::BindingResource<'_> {
                b.as_entire_binding()
            }
            let bufs: [(u32, wgpu::BindingResource); 28] = [
                (0, res(&self.params_buf)),
                (1, wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &self.level_buf, offset: 0, size: wgpu::BufferSize::new(32) })),
                (2, res(&self.stats)),
                (3, res(&s.cache_c)),
                (4, res(&s.cache_t)),
                (5, res(&s.claim)),
                (6, res(&s.rayq)),
                (7, res(&s.tileq)),
                (8, res(&self.keys[par])),
                (9, res(&self.vals[par])),
                (10, res(&self.lists[par])),
                (11, res(&self.vals[prev])),
                (12, res(&self.lists[prev])),
                (13, res(&self.ctable)),
                (14, res(&self.cpos)),
                (15, res(&self.lcount)),
                (16, res(&self.jobs)),
                (17, res(&self.seen)),
                (18, res(&s.need)),
                (19, wgpu::BindingResource::TextureView(&s.idtex)),
                (20, wgpu::BindingResource::TextureView(&s.depth)),
                (21, res(&s.changed_tiles)),
                (22, res(&s.sky[par])),
                (23, res(&s.sky[prev])),
                (24, res(&s.cast_frame)),
                (25, res(&self.hist)),
                (26, res(&self.sorted)),
                (27, res(&self.merged)),
            ];
            let entries: Vec<_> = bufs.into_iter().map(|(binding, resource)| wgpu::BindGroupEntry { binding, resource }).collect();
            self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("discover"), layout: &self.layout_disc, entries: &entries })
        })
    }

    /// Keeps one slot per loaded non-air chunk; face keys index chunks by slot.
    fn update_slots(&mut self, changed: Vec<IVec3>) {
        for pos in changed {
            let present = self.world.chunk(pos).is_some();
            match (present, self.slots.get(&pos).copied()) {
                (true, None) => match self.free_slots.pop() {
                    Some(s) => {
                        self.slots.insert(pos, s);
                        self.slots_dirty = true;
                    }
                    None => {
                        if self.slot_overflow == 0 {
                            tracing::error!("gpu-discover: more than {MAX_SLOTS} non-air chunks; faces of extra chunks are lost");
                        }
                        self.slot_overflow += 1;
                    }
                },
                (false, Some(s)) => {
                    self.slots.remove(&pos);
                    self.free_slots.push(s);
                    self.slots_dirty = true;
                }
                _ => {}
            }
        }
    }

    fn upload_slots(&mut self) {
        if !self.slots_dirty {
            return;
        }
        self.slots_dirty = false;
        let mut table = vec![[0i32, 0, 0, -1]; CHUNK_TABLE];
        let mut pos = vec![[0i32; 4]; MAX_SLOTS];
        let mask = CHUNK_TABLE - 1;
        let mut max_probe = 0;
        for (p, &s) in &self.slots {
            let mut i = hash3(*p) as usize & mask;
            let mut probe = 0;
            while table[i][3] >= 0 {
                i = (i + 1) & mask;
                probe += 1;
            }
            max_probe = max_probe.max(probe);
            table[i] = [p.x, p.y, p.z, s as i32];
            pos[s as usize] = [p.x, p.y, p.z, 1];
        }
        self.chunk_probe = max_probe;
        self.gpu.queue.write_buffer(&self.ctable, 0, bytemuck::cast_slice(&table));
        self.gpu.queue.write_buffer(&self.cpos, 0, bytemuck::cast_slice(&pos));
    }

    /// Takes counters of the most recent finished frame from the staging buffers.
    fn harvest(&mut self) {
        let mut newest: Option<(u32, Vec<u32>)> = None;
        let mut parity = 0;
        for st in &self.stages {
            if st.state.load(Ordering::SeqCst) == 2 {
                let words: Vec<u32> = {
                    let v = st.buf.get_mapped_range(..).expect("mapped");
                    bytemuck::cast_slice(&v[..]).to_vec()
                };
                st.buf.unmap();
                st.state.store(0, Ordering::SeqCst);
                let (frame, par) = st.tag;
                if newest.as_ref().is_none_or(|(f, _)| frame.wrapping_sub(*f) as i32 > 0) {
                    newest = Some((frame, words));
                    parity = par;
                }
            }
        }
        if let Some((_, w)) = newest {
            self.last_stats = Some(w);
            self.last_parity = parity as usize;
        }
    }

    /// What the active `view`'s colours mean.
    fn legend(&self) -> Option<Legend> {
        Some(match self.opts.view.as_str() {
            "overdraw" => Legend::ramp("fragments shaded per pixel", self.opts.view_max.clamp(1, 255) as f64, false),
            "tiles" => Legend::categories(&[
                ("full frame", Some([255, 255, 255])),
                ("edited block / changed chunk", Some([64, 102, 255])),
                ("entering strip", Some([26, 230, 255])),
                ("uncovered or hole", Some([255, 51, 38])),
                ("parallax", Some([255, 217, 26])),
                ("refresh", Some([51, 230, 77])),
                ("skipped (darkened)", Some([0, 0, 0])),
            ]),
            "rays" => Legend::categories(&[("ray cast this frame", Some([255, 255, 51])), ("no ray (darkened)", Some([0, 0, 0]))]),
            "churn" => Legend::categories(&[("face new to the set this frame", Some([255, 38, 26])), ("carried over (grey)", Some([128, 128, 128]))]),
            "quads" => Legend::categories(&[("one random colour per drawn quad", None)]),
            _ => return None,
        })
    }

    fn counters(&self, mode: RenderMode) -> Vec<(String, f64)> {
        let mut out = vec![("chunk_slots".into(), self.slots.len() as f64), ("slot_overflow".into(), self.slot_overflow as f64)];
        out.extend(self.accum.counter(mode));
        if self.opts.incremental && self.opts.events {
            // 0 static (skipped), 1 still (rotation / chunk changes), 2 moving, 3 full.
            out.push(("frame_kind".into(), self.last_kind as u32 as f64));
            let total = self.kind_counts.iter().sum::<u64>().max(1) as f64;
            for (k, name) in ["static", "still", "moving", "full"].iter().enumerate() {
                out.push((format!("frames_{name}_pct"), self.kind_counts[k] as f64 / total * 100.0));
            }
        }
        let Some(w) = &self.last_stats else { return out };
        if self.last_kind == FrameKind::Static {
            // Nothing was discovered: report the redraw, not the last discovery's work.
            let faces = w[STATS_WORDS as usize + self.last_parity] as f64;
            out.extend([("rays".into(), 0.0), ("rays_per_px".into(), 0.0), ("faces_s".into(), faces), ("faces_new".into(), 0.0), ("faces_dropped".into(), 0.0), ("tiles_needed_pct".into(), 0.0)]);
            return out;
        }
        let levels = self.levels();
        let rays: u64 = (0..levels).map(|l| w[8 + l] as u64).sum();
        let px = (self.width * self.height) as f64;
        let faces = w[STATS_WORDS as usize + self.last_parity];
        if self.profile.is_some() {
            let base = STATS_WORDS as usize + 4;
            let ts: Vec<u64> = (0..PROFILE_TS as usize).map(|k| w[base + 2 * k] as u64 | (w[base + 2 * k + 1] as u64) << 32).collect();
            let ms = |a: usize, b: usize| if ts[b] > ts[a] { (ts[b] - ts[a]) as f64 * self.ts_period as f64 / 1e6 } else { 0.0 };
            out.push(("ms_clear_survive".into(), ms(0, 1)));
            let (mut r, mut c) = (0.0, 0.0);
            for l in 0..levels {
                let (a, b) = (ms(1 + 2 * l, 2 + 2 * l), ms(2 + 2 * l, 3 + 2 * l));
                out.push((format!("ms_L{l}_rays"), a));
                out.push((format!("ms_L{l}_classify"), b));
                r += a;
                c += b;
            }
            out.push(("ms_rays".into(), r));
            out.push(("ms_classify".into(), c));
            let raster = PROFILE_TS as usize - 1;
            out.push(("ms_raster".into(), ms(raster - 1, raster)));
        }
        out.push(("rays".into(), rays as f64));
        out.push(("rays_per_px".into(), rays as f64 / px));
        out.push(("faces_s".into(), faces as f64));
        out.push(("survivors".into(), w[32] as f64));
        out.push(("faces_from_tiles".into(), w[39] as f64));
        out.push(("faces_from_1px".into(), w[38] as f64));
        out.push(("footprint_rejects".into(), w[37] as f64));
        out.push(("overflow".into(), w[34] as f64));
        out.push(("closure_tiles".into(), w[40] as f64));
        out.push(("faces_from_closure".into(), w[41] as f64));
        out.push(("no_chunk_slot".into(), w[35] as f64));
        for l in 0..levels {
            out.push((format!("acc_L{l}_{}px", self.opts.tile >> l), w[16 + l] as f64));
            out.push((format!("sky_L{l}"), w[24 + l] as f64));
            out.push((format!("tiles_L{l}"), w[l] as f64));
            out.push((format!("jobs_L{l}"), w[48 + l] as f64));
            out.push((format!("rays_L{l}"), w[8 + l] as f64));
        }
        let prev_faces = w[STATS_WORDS as usize + 1 - self.last_parity] as f64;
        out.push(("faces_new".into(), w[44] as f64));
        if self.merging() {
            let quads = w[STATS_WORDS as usize + 2] as f64;
            out.push(("quads".into(), quads));
            out.push(("faces_per_quad".into(), faces as f64 / quads.max(1.0)));
        }
        out.push(("faces_dropped".into(), (prev_faces - w[32] as f64).max(0.0)));
        if self.opts.incremental {
            let tiles = (self.width.div_ceil(self.opts.tile) * self.height.div_ceil(self.opts.tile)) as f64;
            out.push(("tiles_needed".into(), w[42] as f64));
            out.push(("tiles_needed_pct".into(), w[42] as f64 / tiles * 100.0));
            out.push(("reveal_clamped".into(), w[43] as f64));
        }
        if self.opts.audit {
            out.push(("audit_miss_px".into(), w[33] as f64));
            out.push(("audit_miss_pct".into(), w[33] as f64 / px * 100.0));
            out.push(("audit_hit_px".into(), w[36] as f64));
        }
        out
    }

    /// Rasterises list `parity` into the output (depth pre-pass + equal-depth shading in lit
    /// mode). `ts` = (begin, end) frame timestamps carried by these passes.
    fn encode_shade(&self, enc: &mut wgpu::CommandEncoder, parity: usize, world_bg: &wgpu::BindGroup, mode: RenderMode, ts: (bool, bool), ticket: mr_gpu::Ticket) {
        let painter = self.opts.order == "painter";
        let overdraw = view_code(&self.opts.view) == 4;
        let sky = if overdraw { [0, 0, 0, 255] } else { encode_srgb(self.world.env.sky) };
        let clear = wgpu::Color { r: sky[0] as f64 / 255.0, g: sky[1] as f64 / 255.0, b: sky[2] as f64 / 255.0, a: 1.0 };
        let out_view = self.sized.output.create_view(&Default::default());
        let prepass = self.opts.depth_prepass && mode == RenderMode::Lit && !painter && !overdraw;
        let merging = self.merging();
        let draw_ofs = if merging { 108 * 4 } else { (72 + parity as u64 * 4) * 4 };
        let bind_draw = if painter {
            &self.bind_sorted
        } else if merging {
            &self.bind_merged
        } else {
            &self.bind_draw[parity]
        };
        let st = self.bind_st.as_ref().expect("set in render");
        let frame = DrawFrame {
            out: &out_view,
            depth: &self.sized.depth,
            clear,
            prepass,
            keep_depth: self.opts.incremental,
            binds: &[bind_draw, world_bg, st],
            ring: &self.ring,
            ticket,
            ts,
        };
        let draw = |pass: &mut wgpu::RenderPass<'_>| pass.draw_indirect(&self.args, draw_ofs);
        let main = match (overdraw, painter) {
            (true, true) => Some(&self.pipe_over_paint),
            (true, false) => Some(&self.pipe_over_depth),
            (false, true) => Some(&self.pipe_paint),
            (false, false) => None,
        };
        self.drawer.encode(&self.gpu, enc, &frame, &draw, None, main);
    }

    /// The face drawer for `draw` (`draw_src` plus the visibility component).
    #[allow(clippy::too_many_arguments)]
    fn drawer(gpu: &Gpu, program: &mr_gpu::program::Program, deps: &[String], draw_src: &str, shading: &str, draw: &str, pl_draw: &wgpu::PipelineLayout, pl_shade: &wgpu::PipelineLayout, images: &wgpu::BindGroupLayout, width: u32, height: u32) -> anyhow::Result<FaceDrawer> {
        let mode = DrawMode::parse(draw).ok_or_else(|| anyhow::anyhow!("draw must be raster, visbuffer or gbuffer, not `{draw}`"))?;
        let vis = draw::gpu_wgsl(mode.name()).unwrap();
        let vis = if mode.deferred() { vis.to_string() } else { shade_state::fragment_wgsl(shading, vis) };
        let module = mr_gpu::program::compile(&gpu.device, "discover drawer", program, &format!("{draw_src}\n{vis}"), &deps.iter().map(String::as_str).collect::<Vec<_>>());
        FaceDrawer::new(gpu, &DrawerSpec { module: &module, render_layout: pl_draw, shade_layout: pl_shade, strip: true }, mode, images, width, height)
    }

    fn sky_rgba(&self) -> [f32; 4] {
        let s = encode_srgb(self.world.env.sky);
        [s[0] as f32 / 255.0, s[1] as f32 / 255.0, s[2] as f32 / 255.0, 1.0]
    }

    /// `view = tiles | rays`: blends the diagnostic overlay over the output (`overdraw`:
    /// replaces the counts with their colours). `frame` is the
    /// discovery frame to show (0: nothing was discovered this frame).
    fn encode_overlay(&mut self, enc: &mut wgpu::CommandEncoder, frame: u32, off_x: u32, off_y: u32, cols: u32, rows: u32) {
        let view = view_code(&self.opts.view);
        if view < 2 {
            return;
        }
        let u = OverlayUniform { view, frame, tile: self.opts.tile, cols, rows, need_off: self.sized.need_off, lat_w: self.sized.lat_w, off_x, off_y, view_max: self.opts.view_max.clamp(1, 255), _p: [0; 2] };
        if view == 4 {
            // The overlay ramps the counts, so it reads them from a copy of the output.
            let size = wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 };
            enc.copy_texture_to_texture(self.sized.output.as_image_copy(), self.sized.counts.as_image_copy(), size);
        }
        self.gpu.queue.write_buffer(&self.overlay_buf, 0, bytemuck::bytes_of(&u));
        if self.bind_ov.is_none() {
            self.bind_ov = Some(self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("discover overlay"),
                layout: &self.layout_ov,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.overlay_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: self.sized.need.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: self.sized.cast_frame.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.sized.counts.create_view(&Default::default())) },
                ],
            }));
        }
        let out_view = self.sized.output.create_view(&Default::default());
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("discover overlay"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &out_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipe_overlay);
        pass.set_bind_group(0, self.bind_ov.as_ref(), &[]);
        pass.draw(0..3, 0..1);
    }

    /// Event-driven static frame: camera and world unchanged since the last discovery, so
    /// the last face list is still complete. Only shading runs (lights may move).
    fn render_static(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink, ticket: mr_gpu::Ticket, start: Instant) -> anyhow::Result<()> {
        self.last_kind = FrameKind::Static;
        self.kind_counts[FrameKind::Static as usize] += 1;
        let parity = (self.frame & 1) as usize;
        let c = &req.camera;
        let (_, frac) = c.split_position();
        let aspect = self.width as f32 / self.height as f32;
        let view_proj = c.projection(aspect) * c.view_rotation();
        let fu = FrameUniform {
            view_proj: view_proj.to_cols_array_2d(),
            cam_frac: frac.to_array(),
            mode: (req.mode == RenderMode::Lit) as u32,
            forward: c.forward().to_array(),
            tan_half_fov: (c.fov_y * 0.5).tan(),
            right: c.right().to_array(),
            aspect,
            up: c.up().to_array(),
            // Nothing is new in a static frame.
            view: if view_code(&self.opts.view) == 5 { 2 } else { 0 },
            size: [self.width as f32, self.height as f32],
            _p1: [0.0; 2],
            sky: self.sky_rgba(),
        };
        self.gpu.queue.write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&fu));
        let world_bg = self.world.prepare(&self.gpu, c).clone();
        let storage_bg = self.data.prepare(&self.gpu, &self.world, &self.layout_st).clone();
        self.bind_st = Some(storage_bg.clone());
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        self.encode_shade(&mut enc, parity, &world_bg, req.mode, (true, true), ticket);
        let tile = self.opts.tile;
        self.encode_overlay(&mut enc, 0, 0, 0, self.width.div_ceil(tile), self.height.div_ceil(tile));
        let stats = FrameStats { cpu_ms: start.elapsed().as_secs_f32() * 1e3, gpu_ms: None, counters: self.counters(req.mode), legend: self.legend() };
        self.ring.submit(&self.gpu, enc, ticket, &self.sized.output, req.frame_id, req.readback, stats);
        self.ring.poll(&self.gpu, sink, false)
    }

    /// Row merge active (painter order sorts single faces, so it takes precedence).
    fn merging(&self) -> bool {
        self.opts.merge == "rows" && self.opts.order != "painter"
    }

    fn levels(&self) -> usize {
        (self.opts.tile.trailing_zeros() as usize + 1).min(MAX_LEVELS)
    }
}

impl Renderer for GpuDiscover {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        if !updates.is_empty() {
            self.accum.invalidate();
        }
        if self.opts.incremental {
            // Event areas for the next frame: edited blocks exactly, chunk loads whole.
            for u in &updates {
                match u {
                    WorldUpdate::SetBlocks(edits) => self.pending.extend(edits.iter().map(|e| (e.pos, 1))),
                    WorldUpdate::ChunkLoad { key, .. } | WorldUpdate::ChunkUnload { key } if key.lod == 0 => self.pending.push((key.pos * 32, 32)),
                    _ => {}
                }
            }
        }
        let changed = self.world.apply(&self.gpu, updates);
        self.data.update(&self.gpu, &self.world, &changed)?;
        self.update_slots(changed);
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let forward = !self.drawer.mode().deferred();
        if !forward && (self.opts.order == "painter" || view_code(&self.opts.view) == 4) {
            anyhow::bail!("order = painter and view = overdraw need draw = raster (both draw forward)");
        }
        if forward && shade_state::stateful(&self.shading) && req.mode == RenderMode::Lit && (self.opts.order == "painter" || !self.opts.depth_prepass) {
            anyhow::bail!("`{}` shading with draw = raster needs order = depth and depth_prepass in lit mode (exactly one fragment per pixel may update its state)", self.shading);
        }
        self.accum.begin(&self.gpu, &req.camera, req.mode);
        let ticket = self.ring.begin(&self.gpu, sink)?;
        let start = Instant::now();
        self.harvest();
        self.upload_slots();
        let events = self.opts.incremental && self.opts.events;
        let c = &req.camera;
        let tan_now = (c.fov_y * 0.5).tan();
        let same_pos = self.prev_pos == Some(c.position);
        let same_rot = self.prev_cam.is_some_and(|p| p == (c.forward().to_array(), c.right().to_array(), c.up().to_array(), tan_now));
        if events && same_pos && same_rot && self.pending.is_empty() && !self.force_full && self.frame > 0 {
            // Nothing that affects visibility changed: redraw the last face list.
            return self.render_static(req, sink, ticket, start);
        }
        self.frame = self.frame.wrapping_add(1).max(1);
        let parity = (self.frame & 1) as usize;
        let tile = self.opts.tile.next_power_of_two().clamp(2, MAX_TILE);
        self.opts.tile = tile;
        let levels = self.levels();
        let (off_x, off_y) = if self.opts.jitter && !self.opts.incremental {
            self.rng ^= self.rng << 13;
            self.rng ^= self.rng >> 7;
            self.rng ^= self.rng << 17;
            ((self.rng as u32) % tile, ((self.rng >> 32) as u32) % tile)
        } else {
            (0, 0)
        };
        let cols = (self.width + off_x).div_ceil(tile);
        let rows = (self.height + off_y).div_ceil(tile);
        let (_, frac) = c.split_position();
        let aspect = self.width as f32 / self.height as f32;
        let tan_half_fov = (c.fov_y * 0.5).tan();
        let queue_cap = self.sized.lat_w * self.sized.lat_h;
        // Incremental mode: camera translation since last frame, and changed chunks' tiles.
        let moved = self.prev_pos.map(|p| (c.position - p).length()).unwrap_or(f64::INFINITY);
        self.prev_pos = Some(c.position);
        let f_px = self.height as f32 * 0.5 / tan_half_fov;
        let mut full = self.force_full || moved > 64.0;
        let mut any_changed = false;
        if self.opts.incremental && !self.pending.is_empty() {
            let vp = c.projection(aspect) * c.view_rotation();
            let mut mask = vec![0u32; (cols * rows) as usize];
            for (lo, size) in self.pending.drain(..) {
                match box_rect(&vp, lo, size, c.position, self.width, self.height, tile, cols, rows) {
                    Some(Some([x0, y0, x1, y1])) => {
                        for y in y0..=y1 {
                            mask[(y * cols + x0) as usize..=(y * cols + x1) as usize].fill(1);
                        }
                        any_changed = true;
                    }
                    Some(None) => {}
                    None => full = true,
                }
            }
            if any_changed {
                self.gpu.queue.write_buffer(&self.sized.changed_tiles, 0, bytemuck::cast_slice(&mask));
            }
        }
        let kind = if full {
            FrameKind::Full
        } else if events && same_pos {
            FrameKind::Still
        } else {
            FrameKind::Moving
        };
        self.last_kind = kind;
        self.kind_counts[kind as usize] += 1;
        let prev_cam = if full { None } else { self.prev_cam };
        self.prev_cam = Some((c.forward().to_array(), c.right().to_array(), c.up().to_array(), tan_half_fov));
        self.force_full = false;
        let prm = Params {
            forward: c.forward().to_array(),
            tan_half_fov,
            right: c.right().to_array(),
            aspect,
            up: c.up().to_array(),
            frame: self.frame,
            origin: frac.to_array(),
            tile,
            width: self.width,
            height: self.height,
            off_x,
            off_y,
            lat_w: self.sized.lat_w,
            lat_h: self.sized.lat_h,
            cols,
            rows,
            keep: if self.opts.incremental { self.opts.keep_unseen } else { self.opts.keep_frames }.max(1),
            criterion: (self.opts.criterion == "face") as u32,
            sky_tile: self.opts.sky_tile,
            max_fp: self.opts.max_footprint,
            table_mask: self.table_cap - 1,
            list_cap: self.list_cap,
            chunk_mask: CHUNK_TABLE as u32 - 1,
            chunk_probe: self.chunk_probe,
            queue_cap,
            parity: parity as u32,
            strict: self.opts.strict as u32,
            still: (kind == FrameKind::Still) as u32,
            closure_px: self.opts.closure_px,
            closure_box: self.opts.closure_box as i32,
            block_faces: self.opts.block_faces as u32,
            inline_fp: self.opts.inline_fp,
            incr: self.opts.incremental as u32,
            refresh: self.opts.refresh,
            par_k: if moved.is_finite() { f_px * moved as f32 / c.near } else { 0.0 },
            changed: any_changed as u32,
            full: full as u32,
            r_max: self.opts.reveal_tiles.max(1),
            need_off: self.sized.need_off,
            par_thresh: self.opts.parallax_px,
            prev_forward: prev_cam.map(|p| p.0).unwrap_or_default(),
            prev_tan: prev_cam.map(|p| p.3).unwrap_or(1.0),
            prev_right: prev_cam.map(|p| p.1).unwrap_or_default(),
            has_prev: prev_cam.is_some() as u32,
            prev_up: prev_cam.map(|p| p.2).unwrap_or_default(),
            keep_out: self.opts.keep_frames.max(1),
            morton: self.opts.morton as u32,
            morton_side: (cols + 1).max(rows + 1).next_power_of_two(),
            _q: [0; 2],
        };
        self.gpu.queue.write_buffer(&self.params_buf, 0, bytemuck::bytes_of(&prm));
        let mut lvls = vec![LevelUniform::zeroed(); MAX_LEVELS + 1];
        for (l, u) in lvls.iter_mut().enumerate() {
            *u = LevelUniform {
                level: l as u32,
                size: tile >> l.min(31),
                in_off: (l as u32 & 1) * queue_cap,
                out_off: ((l as u32 + 1) & 1) * queue_cap,
                parity: parity as u32,
                cap: queue_cap,
                list_cap: self.list_cap,
                _p: 0,
                _pad: [0; 56],
            };
        }
        self.gpu.queue.write_buffer(&self.level_buf, 0, bytemuck::cast_slice(&lvls));
        let view_proj = c.projection(aspect) * c.view_rotation();
        let fu = FrameUniform {
            view_proj: view_proj.to_cols_array_2d(),
            cam_frac: frac.to_array(),
            mode: (req.mode == RenderMode::Lit) as u32,
            forward: c.forward().to_array(),
            tan_half_fov,
            right: c.right().to_array(),
            aspect,
            up: c.up().to_array(),
            view: match view_code(&self.opts.view) {
                1 => 1,
                5 => 2,
                _ => 0,
            },
            size: [self.width as f32, self.height as f32],
            _p1: [0.0; 2],
            sky: self.sky_rgba(),
        };
        self.gpu.queue.write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&fu));
        let world_bg = self.world.prepare(&self.gpu, c).clone();
        let storage_bg = self.data.prepare(&self.gpu, &self.world, &self.layout_st).clone();
        self.bind_st = Some(storage_bg.clone());
        if self.bind_disc.is_none() {
            self.bind_disc = Some(self.make_bind_disc());
        }
        let bind_disc = self.bind_disc.as_ref().unwrap()[parity].clone();

        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        // Counter/timestamp staging buffer for this frame (if one is free).
        let stage = self.stages.iter().position(|s| s.state.load(Ordering::SeqCst) == 0);
        let qs = self.profile.as_ref().zip(stage).map(|(q, i)| (q, i as u32 * PROFILE_TS));
        let mark = |enc: &mut wgpu::CommandEncoder, k: u32| {
            if let Some((q, base)) = qs {
                enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("ts"),
                    timestamp_writes: Some(wgpu::ComputePassTimestampWrites { query_set: q, beginning_of_pass_write_index: Some(base + k), end_of_pass_write_index: None }),
                });
            }
        };
        // Empty pass carrying the frame's begin timestamp, so the clears below are timed.
        enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("frame begin"), timestamp_writes: self.ring.compute_timestamps(ticket, true, false) });
        mark(&mut enc, 0);
        enc.clear_buffer(&self.stats, 0, None);
        enc.clear_buffer(&self.keys[parity], 0, None);
        enc.clear_buffer(&self.vals[parity], 0, None);
        enc.clear_buffer(&self.lcount, parity as u64 * 4, Some(4));
        enc.clear_buffer(&self.lcount, 8, Some(4));
        let lofs = |l: usize| (l as u64 * LEVEL_STRIDE) as u32;
        let aofs = |w: u64| w * 4;
        let late = self.opts.still_holes == "late";
        if self.opts.incremental && kind == FrameKind::Still && late {
            // Rotation / chunk changes only: the entering strip and changed chunks.
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("still needs"), timestamp_writes: None });
            pass.set_bind_group(0, &bind_disc, &[lofs(0)]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.set_bind_group(2, &storage_bg, &[]);
            pass.set_pipeline(&self.pipe_need_still);
            pass.dispatch_workgroups((cols * rows).div_ceil(256), 1, 1);
        } else if self.opts.incremental {
            // Last frame's faces at this frame's camera: which faces are still seen, and where
            // the image is uncovered or may reveal something new.
            {
                let id_view = &self.sized.idtex;
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("face ids"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: id_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.sized.depth,
                        depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&self.pipe_id);
                pass.set_bind_group(0, &self.bind_draw[1 - parity], &[]);
                pass.set_bind_group(1, &world_bg, &[]);
            pass.set_bind_group(2, &storage_bg, &[]);
                pass.draw_indirect(&self.args, aofs(72 + (1 - parity as u64) * 4));
            }
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("incremental needs"), timestamp_writes: None });
            pass.set_bind_group(0, &bind_disc, &[lofs(0)]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.set_bind_group(2, &storage_bg, &[]);
            if kind != FrameKind::Still {
                // Still frames count faces in view as seen in `survive` instead.
                pass.set_pipeline(&self.pipe_mark);
                let g = (self.width * self.height).div_ceil(256);
                pass.dispatch_workgroups(g.min(32768), g.div_ceil(32768), 1);
            }
            pass.set_pipeline(&self.pipe_need_raw);
            pass.dispatch_workgroups((cols * rows).div_ceil(256), 1, 1);
            pass.set_pipeline(&self.pipe_need_final);
            pass.dispatch_workgroups((cols * rows).div_ceil(256), 1, 1);
        }
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("survive"), timestamp_writes: None });
            pass.set_bind_group(1, &world_bg, &[]);
            pass.set_bind_group(2, &storage_bg, &[]);
            // Temporal survivors from last frame's list.
            pass.set_pipeline(&self.pipe_survive);
            pass.set_bind_group(0, &bind_disc, &[lofs(0)]);
            pass.dispatch_workgroups_indirect(&self.args, aofs(64 + (1 - parity as u64) * 4));
        }
        mark(&mut enc, 1);
        // Level-0 work items: the grid, or the Z-curve over its enclosing power-of-two square.
        let morton = self.opts.morton;
        let l0_items = |w: u32, h: u32| if morton { w.max(h).next_power_of_two().pow(2) } else { w * h };
        for l in 0..levels {
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("discover rays"), timestamp_writes: None });
                if l > 0 {
                    pass.set_pipeline(&self.pipe_level_args);
                    pass.set_bind_group(0, &self.bind_args, &[lofs(l)]);
                    pass.dispatch_workgroups(1, 1, 1);
                }
                pass.set_bind_group(0, &bind_disc, &[lofs(l)]);
                pass.set_bind_group(1, &world_bg, &[]);
            pass.set_bind_group(2, &storage_bg, &[]);
                pass.set_pipeline(&self.pipe_rays);
                if l == 0 {
                    pass.dispatch_workgroups(l0_items(cols + 1, rows + 1).div_ceil(256), 1, 1);
                } else {
                    pass.dispatch_workgroups_indirect(&self.args, aofs(l as u64 * 8));
                }
            }
            mark(&mut enc, 2 + 2 * l as u32);
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("discover classify"), timestamp_writes: None });
                pass.set_bind_group(0, &bind_disc, &[lofs(l)]);
                pass.set_bind_group(1, &world_bg, &[]);
            pass.set_bind_group(2, &storage_bg, &[]);
                pass.set_pipeline(&self.pipe_classify);
                if l == 0 {
                    pass.dispatch_workgroups(l0_items(cols, rows).div_ceil(256), 1, 1);
                } else {
                    pass.dispatch_workgroups_indirect(&self.args, aofs(l as u64 * 8 + 4));
                }
                pass.set_pipeline(&self.pipe_job_args);
                pass.set_bind_group(0, &self.bind_args, &[lofs(l)]);
                pass.dispatch_workgroups(1, 1, 1);
                pass.set_pipeline(&self.pipe_footprint);
                pass.set_bind_group(0, &bind_disc, &[lofs(l)]);
                pass.dispatch_workgroups_indirect(&self.args, aofs(80 + l as u64 * 4));
            }
            mark(&mut enc, 3 + 2 * l as u32);
        }
        // Unused level slots still get written so every resolved query is available.
        for l in levels..MAX_LEVELS {
            mark(&mut enc, 2 + 2 * l as u32);
            mark(&mut enc, 3 + 2 * l as u32);
        }
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("discover finalize"), timestamp_writes: None });
            pass.set_pipeline(&self.pipe_finalize);
            pass.set_bind_group(0, &self.bind_args, &[lofs(0)]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        mark(&mut enc, 2 + 2 * MAX_LEVELS as u32);
        if self.merging() {
            // Merge rows of faces into runs (timed with the raster).
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("row merge"), timestamp_writes: None });
            pass.set_bind_group(0, &bind_disc, &[lofs(0)]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.set_bind_group(2, &storage_bg, &[]);
            pass.set_pipeline(&self.pipe_merge);
            pass.dispatch_workgroups_indirect(&self.args, aofs(64 + parity as u64 * 4));
            pass.set_pipeline(&self.pipe_merge_args);
            pass.set_bind_group(0, &self.bind_args, &[lofs(0)]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        if self.opts.order == "painter" {
            // Sort the list back to front (timed with the raster).
            enc.clear_buffer(&self.hist, 0, None);
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("painter sort"), timestamp_writes: None });
            pass.set_bind_group(0, &bind_disc, &[lofs(0)]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.set_bind_group(2, &storage_bg, &[]);
            pass.set_pipeline(&self.pipe_sort_hist);
            pass.dispatch_workgroups_indirect(&self.args, aofs(64 + parity as u64 * 4));
            pass.set_pipeline(&self.pipe_sort_scan);
            pass.dispatch_workgroups(1, 1, 1);
            pass.set_pipeline(&self.pipe_sort_scatter);
            pass.dispatch_workgroups_indirect(&self.args, aofs(64 + parity as u64 * 4));
        }
        self.encode_shade(&mut enc, parity, &world_bg, req.mode, (false, true), ticket);
        mark(&mut enc, 3 + 2 * MAX_LEVELS as u32);
        if self.opts.incremental {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("sky mask"), timestamp_writes: None });
            pass.set_pipeline(&self.pipe_sky);
            pass.set_bind_group(0, &bind_disc, &[lofs(0)]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.set_bind_group(2, &storage_bg, &[]);
            let g = (self.width * self.height).div_ceil(256);
            pass.dispatch_workgroups(g.min(32768), g.div_ceil(32768), 1);
        }
        self.encode_overlay(&mut enc, self.frame, off_x, off_y, cols, rows);
        // Audit runs after the timed passes so it does not inflate gpu_ms.
        if self.opts.audit {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("discover audit"), timestamp_writes: None });
            pass.set_pipeline(&self.pipe_audit);
            pass.set_bind_group(0, &bind_disc, &[lofs(0)]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.set_bind_group(2, &storage_bg, &[]);
            let g = (self.width * self.height).div_ceil(256);
            pass.dispatch_workgroups(g.min(32768), g.div_ceil(32768), 1);
        }
        // Counters: copy into the staging buffer; mapped asynchronously.
        if let Some(i) = stage {
            let b = &self.stages[i].buf;
            enc.copy_buffer_to_buffer(&self.stats, 0, b, 0, STATS_WORDS * 4);
            enc.copy_buffer_to_buffer(&self.lcount, 0, b, STATS_WORDS * 4, 16);
            if let (Some(q), Some(res)) = (&self.profile, &self.profile_resolve) {
                let base = i as u32 * PROFILE_TS;
                enc.resolve_query_set(q, base..base + PROFILE_TS, res, i as u64 * 256);
                enc.copy_buffer_to_buffer(res, i as u64 * 256, b, STATS_WORDS * 4 + 16, PROFILE_TS as u64 * 8);
            }
            self.stages[i].tag = (self.frame, parity as u32);
        }
        let stats = FrameStats { cpu_ms: start.elapsed().as_secs_f32() * 1e3, gpu_ms: None, counters: self.counters(req.mode), legend: self.legend() };
        self.ring.submit(&self.gpu, enc, ticket, &self.sized.output, req.frame_id, req.readback, stats);
        if let Some(i) = stage {
            let st = &self.stages[i];
            st.state.store(1, Ordering::SeqCst);
            let flag = st.state.clone();
            st.buf.map_async(wgpu::MapMode::Read, .., move |r| flag.store(if r.is_ok() { 2 } else { 3 }, Ordering::SeqCst));
        }
        self.ring.poll(&self.gpu, sink, false)
    }

    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        let int = |d: i64| value.as_i64().unwrap_or(d) as u32;
        match name {
            "tile" => self.opts.tile = int(8),
            "criterion" => self.opts.criterion = value.as_str().unwrap_or("plane").into(),
            "sky_tile" => self.opts.sky_tile = int(4),
            "keep_frames" => self.opts.keep_frames = int(8),
            "max_footprint" => self.opts.max_footprint = int(128),
            "strict" => self.opts.strict = value.as_bool().unwrap_or(true),
            "inline_fp" => self.opts.inline_fp = int(8),
            "morton" => self.opts.morton = value.as_bool().unwrap_or(true),
            "closure_px" => self.opts.closure_px = int(2),
            "closure_box" => self.opts.closure_box = int(2),
            "block_faces" => self.opts.block_faces = value.as_bool().unwrap_or(true),
            "jitter" => self.opts.jitter = value.as_bool().unwrap_or(true),
            "incremental" => {
                self.opts.incremental = value.as_bool().unwrap_or(false);
                self.pending.clear();
            }
            "refresh" => self.opts.refresh = int(16),
            "keep_unseen" => self.opts.keep_unseen = int(64),
            "parallax_px" => self.opts.parallax_px = value.as_f64().unwrap_or(0.5) as f32,
            "reveal_tiles" => self.opts.reveal_tiles = int(4),
            "audit" => self.opts.audit = value.as_bool().unwrap_or(false),
            "events" => self.opts.events = value.as_bool().unwrap_or(true),
            "still_holes" => self.opts.still_holes = value.as_str().unwrap_or("now").into(),
            "merge" => {
                self.opts.merge = value.as_str().unwrap_or("none").into();
                return Ok(());
            }
            "order" => {
                // Drawing only: the face set is unaffected.
                self.opts.order = value.as_str().unwrap_or("depth").into();
                return Ok(());
            }
            "view" => {
                // Display only: no need to re-discover.
                self.opts.view = value.as_str().unwrap_or("image").into();
                return Ok(());
            }
            "view_max" => {
                self.opts.view_max = int(8).clamp(1, 255) as u32;
                return Ok(());
            }
            "shadows" => self.world.cfg.shadows.set(value)?,
            "max_steps" => self.world.cfg.max_steps = int(2048),
            "depth_prepass" => self.opts.depth_prepass = value.as_bool().unwrap_or(true),
            "draw" => {
                // Drawing only: the face set is unaffected.
                let draw = value.as_str().unwrap_or("raster");
                self.drawer = Self::drawer(&self.gpu, &self.program, &self.deps, &self.draw_src, &self.shading, draw, &self.pl_draw, &self.pl_shade, &self.images, self.width, self.height)?;
                self.opts.draw = draw.into();
                return Ok(());
            }
            _ if self.accum.set_param(name, value).is_some() => return Ok(()),
            _ => anyhow::bail!("`{name}` cannot change while running"),
        }
        // Any setting can change what is discoverable: start over from a full frame.
        self.force_full = true;
        Ok(())
    }

    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, true)
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.width = width;
        self.height = height;
        self.sized = make_sized(&self.gpu, width, height);
        self.drawer.resize(&self.gpu, width, height);
        self.accum.resize(&self.gpu, width, height);
        self.bind_merged = draw_bind(&self.gpu, &self.layout_draw, &self.frame_buf, &self.merged, &self.cpos, &self.accum);
        self.bind_sorted = draw_bind(&self.gpu, &self.layout_draw, &self.frame_buf, &self.sorted, &self.cpos, &self.accum);
        self.bind_draw = [0, 1].map(|i| draw_bind(&self.gpu, &self.layout_draw, &self.frame_buf, &self.lists[i], &self.cpos, &self.accum));
        self.bind_disc = None;
        self.bind_ov = None;
        self.force_full = true;
        self.ring = FrameRing::new(&self.gpu, width, height, self.opts.frames_in_flight.max(1), self.waker.clone());
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        let s = &self.sized;
        let set = (self.keys[0].size() + self.vals[0].size() + self.lists[0].size()) * 2 + self.jobs.size() + self.seen.size() + s.changed_tiles.size() + s.need.size() + self.width as u64 * self.height as u64 * 4;
        let gpu = self.world.memory_bytes() + self.data.memory_bytes() + set + s.cache_c.size() + s.cache_t.size() + s.claim.size() + s.rayq.size() + s.tileq.size() + self.ctable.size() + self.cpos.size() + self.accum.memory_bytes() + self.drawer.memory_bytes() + self.width as u64 * self.height as u64 * 8;
        MemoryReport { cpu_bytes: 0, gpu_bytes: gpu }
    }
}

/// A draw bind group: the frame, a face list, the chunk positions and the shading state.
fn draw_bind(gpu: &Gpu, layout: &wgpu::BindGroupLayout, frame: &wgpu::Buffer, list: &wgpu::Buffer, cpos: &wgpu::Buffer, accum: &ShadeState) -> wgpu::BindGroup {
    let [a, b] = accum.bind_entries(3);
    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("discover draw"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: frame.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: list.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: cpos.as_entire_binding() },
            a,
            b,
        ],
    })
}

/// Screen rectangle, in level-0 tiles, of the world box `[min, min + size)`: `Some(None)`
/// if it is off screen. A box straddling the camera plane that may be visible gets the
/// whole screen (its projection is unbounded); `None` is never returned for boxes behind
/// or beside the camera, only kept for callers that want a forced full frame.
#[allow(clippy::too_many_arguments)]
fn box_rect(view_proj: &glam::Mat4, min: IVec3, size: i32, cam: glam::DVec3, width: u32, height: u32, tile: u32, cols: u32, rows: u32) -> Option<Option<[u32; 4]>> {
    let clips: [glam::Vec4; 8] = std::array::from_fn(|k| {
        let k = k as i32;
        let corner = (min + IVec3::new(k & 1, (k >> 1) & 1, k >> 2) * size).as_dvec3();
        *view_proj * (corner - cam).as_vec3().extend(1.0)
    });
    // Outside one frustum plane entirely (left, right, bottom, top, near): off screen.
    let outside = |f: &dyn Fn(&glam::Vec4) -> bool| clips.iter().all(f);
    if outside(&|c| c.x < -c.w) || outside(&|c| c.x > c.w) || outside(&|c| c.y < -c.w) || outside(&|c| c.y > c.w) || outside(&|c| c.w <= 1e-3) {
        return Some(None);
    }
    if clips.iter().any(|c| c.w <= 1e-3) {
        return Some(Some([0, 0, cols - 1, rows - 1]));
    }
    let (mut lo, mut hi) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
    for clip in clips {
        let ndc = clip.truncate().truncate() / clip.w;
        let px = glam::Vec2::new((ndc.x * 0.5 + 0.5) * width as f32, (0.5 - ndc.y * 0.5) * height as f32);
        lo = lo.min(px);
        hi = hi.max(px);
    }
    if hi.x < 0.0 || hi.y < 0.0 || lo.x >= width as f32 || lo.y >= height as f32 {
        return Some(None);
    }
    let t = tile as f32;
    let x0 = (lo.x.max(0.0) / t) as u32;
    let y0 = (lo.y.max(0.0) / t) as u32;
    let x1 = ((hi.x / t) as u32).min(cols - 1);
    let y1 = ((hi.y / t) as u32).min(rows - 1);
    Some(Some([x0, y0, x1, y1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Discovery, raster and every drawer compose from their modules and validate for every
    /// storage, kernel and shading (no GPU needed), as `GpuDiscover::new` assembles them.
    #[test]
    fn programs_compose() {
        use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        let check = |p: &mr_gpu::program::Program, root: &str, deps: &[String], what: &str| {
            let deps: Vec<&str> = deps.iter().map(String::as_str).collect();
            let m = p.naga(root, &deps).unwrap_or_else(|e| panic!("{what}: {e}"));
            Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("{what}: {e:?}"));
        };
        for st in names_on(BACKEND, Slot::Storage) {
            for k in names_on(BACKEND, Slot::Traversal) {
                let (program, walk) = mr_components::shaders::walk_program(&storage::gpu(st).unwrap(), k, false);
                let deps: Vec<String> = mr_components::shaders::root_deps(&walk, &[]).into_iter().map(String::from).collect();
                check(&program, include_str!("discover.wgsl"), &deps, &format!("discover {st} {k}"));
                for shading in names_on(BACKEND, Slot::Shading) {
                    let shade = mr_components::shading::gpu_wgsl(shading).unwrap();
                    let draw_src = format!("{}\n{shade}\n{}\n{}", ShadeState::wgsl(0, 3), include_str!("raster.wgsl"), draw::faces_wgsl(true));
                    check(&program, &format!("{draw_src}\n{}", shade_state::fragment_wgsl(shading, draw::gpu_wgsl("raster").unwrap())), &deps, &format!("raster {st} {k} {shading}"));
                    for d in names_on(BACKEND, Slot::Draw) {
                        let mode = DrawMode::parse(d).unwrap();
                        let vis = draw::gpu_wgsl(mode.name()).unwrap();
                        let vis = if mode.deferred() { vis.to_string() } else { shade_state::fragment_wgsl(shading, vis) };
                        check(&program, &format!("{draw_src}\n{vis}"), &deps, &format!("drawer {st} {k} {shading} {d}"));
                    }
                }
            }
        }
    }
}
