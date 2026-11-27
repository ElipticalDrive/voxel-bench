//! Meshing (`raster-gpu`): the face source of view-independent geometry. The executor's
//! drawer rasterises the geometry component's faces:
//!
//! * **geometry** turns each chunk into packed face instances on the CPU (parallel,
//!   incremental; `greedy`, `naive`, `cubes`), uploaded per chunk (the schedule's transfer of
//!   the faces from the CPU to the GPU) and drawn with vertex pulling and one
//!   multi-draw-indirect call after per-chunk frustum and per-face-direction culling;
//! * **visibility** `faces`: every face goes to the draw stage;
//! * **draw** decides how the front face of each pixel is found and shaded (the executor's
//!   face drawer): `raster` (forward, with an optional depth pre-pass in lit mode), `gbuffer`
//!   or `visbuffer` (ids, then one compute shading pass);
//! * **shading** colours the hit (`shade_surface`, the same component as on the ray-walk back
//!   ends), with lit-mode shadow rays walking the composed **storage** with the
//!   **traversal** kernel.
//!
//! Meshes (mesh mode) share the depth buffer (`mr_gpu::meshes::GpuMeshes`): shaded forward
//! with `raster`, written into the ids or records with the deferred drawers.
//!
//! A gate on the geometry's faces (the visibility's `faces` edge) sits at the per-chunk
//! upload: each remeshed or dropped chunk crosses as one message ([`ChunkFaces`]: position,
//! slot, per-face counts, quads), one ordered stream per chunk, and the GPU's quads are
//! written as messages arrive. A chunk on its way keeps its old faces (or none); a lost one
//! keeps them until the chunk is remeshed. Slots are the sender's: under `reorder` a slot
//! freed and taken by another chunk can be drawn at the newer chunk's position until the
//! older chunk's drop arrives.
//!
//! A gate on the culled faces (the drawer's `faces` edge) sits at the upload of the indirect
//! draws: each frame's draws and their batches cross as one message ([`DrawList`]) with its
//! frame id, and the drawer draws the last list that arrived (nothing before one has). The
//! draws index the GPU's quads, so a late list draws whatever the quads hold now, from this
//! frame's camera (culled with its own frame's).

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Mat4, Vec3, Vec4};
use mr_components::draw::drawer::{DrawFrame, DrawMode, DrawNodes};
use mr_components::geometry::{GpuGeometry, Knobs, Merge};
use mr_components::graph::{Checked, Role};
use mr_components::storage::gpu_data::Bound;
use mr_components::shaders::Walks;
use mr_components::shading::state as shade_state;
use mr_core::gate::{GateChain, GateReport, Get, Put, Wire, WireError};
use mr_core::*;
use mr_gpu::meshes::{ForwardSpec, GpuMeshes, MeshPass};
use mr_gpu::voxels::{Shadows, shadows_param};
use mr_gpu::{PagedBuffer, wgpu};
use rayon::prelude::*;
use serde::Deserialize;

use super::{CoreOpts, Draw, SourceSpec};
use crate::exec::{self, Cx, Node, Region, State};
use crate::Core;

pub(crate) const REGION: &mr_components::graph::Builds = &mr_components::graph::GPU_MESHING;

pub(crate) fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu".into(),
        description: "GPU rasterisation (wgpu): the geometry component turns chunks into faces on the CPU, drawn with vertex pulling and multi-draw-indirect after chunk and face-direction culling; the storage is what lit-mode shadow rays walk".into(),
        tags: vec!["gpu".into(), "raster".into(), "hybrid".into(), "modular".into(), "wgpu".into()],
        features: Features::of(&[Feature::Lit, Feature::Volume, Feature::Meshes, Feature::MeshRays]),
        params: REGION.lighting_knobs(params()),
    }
}



fn params() -> Vec<ParamSpec> {
    let mut p = vec![
        ParamSpec::choice("merge", "greedy: merge faces of the same block only (`block`), or any exposed coplanar faces with a per-pixel block lookup (`exposure`)", &["block", "exposure"], "block", true).of("geometry", &["greedy"]),
        ParamSpec::bool("cull", "naive / cubes: skip faces (blocks) hidden by solid neighbours (false = every face of every solid block)", true, true).of("geometry", &["naive", "cubes"]),
        ParamSpec::bool("depth_prepass", "raster, lit mode: depth pre-pass so lighting runs once per pixel", true, true).of("draw", &["raster"]),
        shadows_param(),
        ParamSpec::int("max_steps", "Shadow-ray step limit (compiled into the kernels)", 32, 16384, 2048, false).baked(true),
        ParamSpec::int("mesh_budget", "Chunks remeshed per frame, nearest first (0 = all)", 0, 100000, 0, true),
        ParamSpec::int("pool_mb", "Voxel pool limit for shadow rays (MiB); for region storages (tree64) the limit of their own data", 64, 6144, 1536, false),
        ParamSpec::int("quad_buffer_mb", "Instance memory limit (MiB; grows on demand, in buffers of at most what the device binds)", 16, 16384, 8192, false),
        ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
    ];
    p.extend(shade_state::params());
    p
}

/// The region's own knobs (the nodes' are read from the graph).
#[derive(Deserialize)]
#[serde(default)]
struct Options {
    max_steps: u32,
    mesh_budget: usize,
    /// Voxel pool for lighting rays (unused in unlit mode but always allocated).
    pool_mb: u32,
    quad_buffer_mb: u32,
    frames_in_flight: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { max_steps: 2048, mesh_budget: 0, pool_mb: 1536, quad_buffer_mb: 8192, frames_in_flight: 3 }
    }
}

/// The geometry node's knobs.
#[derive(Deserialize)]
#[serde(default)]
struct GeometryKnobs {
    merge: String,
    cull: bool,
}

impl Default for GeometryKnobs {
    fn default() -> Self {
        Self { merge: "block".into(), cull: true }
    }
}

/// The draw node's knobs.
#[derive(Deserialize)]
#[serde(default)]
struct DrawKnobs {
    /// raster, lit mode: lay down depth first so the expensive fragment shader runs once
    /// per pixel.
    depth_prepass: bool,
}

impl Default for DrawKnobs {
    fn default() -> Self {
        Self { depth_prepass: true }
    }
}

/// The shading node's knobs.
#[derive(Deserialize)]
#[serde(default)]
struct ShadingKnobs {
    shadows: Shadows,
}

impl Default for ShadingKnobs {
    fn default() -> Self {
        Self { shadows: Shadows::All }
    }
}

/// The components and geometry knobs live knobs change.
struct Live {
    geometry: String,
    shading: String,
    meshes: String,
    knobs: Knobs,
}


fn parse_merge(s: &str) -> anyhow::Result<Merge> {
    match s {
        "block" => Ok(Merge::Block),
        "exposure" => Ok(Merge::Exposure),
        _ => anyhow::bail!("merge must be `block` or `exposure`, not `{s}`"),
    }
}

/// Bind groups of the storage instances: the shadow rays' at 2, the others' after the
/// deferred drawers' images (group 3).
const STORAGE_GROUPS: &[u32] = &[2, 4, 5];

/// The ray-casting sites' walks: the shadow rays' first (the storage and
/// traversal stages), bounces and mesh shadows their own or the same; returns the walks and
/// the indices of (shadow, bounce, mesh shadow).
fn walks(g: &Checked) -> anyhow::Result<(Walks, usize, usize, usize)> {
    let mut walks = Walks::of(g);
    let shadow = g.site(Role::Shading, "shadow").ok_or_else(|| anyhow::anyhow!("shadow rays are not bound"))?;
    let shadow = walks.site(g, shadow, STORAGE_GROUPS, false);
    let bounce = g.site(Role::Shading, "bounce").map_or(shadow, |s| walks.site(g, s, STORAGE_GROUPS, false));
    let mesh_shadow = g.site(Role::Meshes, "shadow").map_or(shadow, |s| walks.site(g, s, STORAGE_GROUPS, false));
    Ok((walks, shadow, bounce, mesh_shadow))
}

fn geometry(name: &str) -> GpuGeometry {
    mr_components::geometry::gpu(name).expect("validated")
}

/// The face source's WGSL: the geometry component's vertex placement and the chunks' faces.
fn source_wgsl(geometry: &str) -> String {
    format!("{}\n{}", self::geometry(geometry).wgsl, include_str!("chunks.wgsl"))
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct DrawArgs {
    vertex_count: u32,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
}

const MAX_SLOTS: usize = 1 << 16;
const QUAD_BYTES: u64 = 8;
/// Quads staged for upload before the writes are sent.
const FLUSH_BYTES: u64 = 256 << 20;

/// A chunk's faces as they leave the geometry for the GPU: its position, its slot in the
/// chunk positions, and its mesh (`None`: the chunk is gone).
struct ChunkFaces {
    pos: IVec3,
    slot: u16,
    mesh: Option<mr_components::geometry::Mesh>,
}

impl Wire for ChunkFaces {
    /// One stream per chunk: its remeshes and its drop stay in order.
    fn stream(&self) -> Option<u64> {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.pos.to_array().hash(&mut h);
        Some(h.finish() | 1)
    }

    fn pieces(&self) -> u32 {
        self.mesh.is_some() as u32
    }

    fn encode(&self, out: &mut Vec<u8>) {
        let quads = self.mesh.as_ref().map_or(0, |m| m.quads.len());
        out.reserve(48 + quads * 8);
        let mut p = Put(out);
        self.pos.to_array().into_iter().for_each(|c| p.i32(c));
        p.u16(self.slot);
        p.u8(self.mesh.is_some() as u8);
        if let Some(m) = &self.mesh {
            m.counts.iter().for_each(|&c| p.u32(c));
            p.u32(m.quads.len() as u32);
            p.bytes(bytemuck::cast_slice(&m.quads));
        }
    }

    fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut g = Get(bytes);
        let pos = IVec3::new(g.i32()?, g.i32()?, g.i32()?);
        let slot = g.u16()?;
        let mesh = match g.u8()? {
            0 => None,
            1 => {
                let mut counts = [0u32; 6];
                for c in &mut counts {
                    *c = g.u32()?;
                }
                let n = g.count(8)?;
                let mut quads = vec![[0u32; 2]; n];
                bytemuck::cast_slice_mut::<[u32; 2], u8>(&mut quads).copy_from_slice(g.take(n * 8)?);
                if counts.iter().map(|&c| c as u64).sum::<u64>() != n as u64 {
                    return Err(WireError("a chunk's face counts do not add up to its quads".into()));
                }
                Some(mr_components::geometry::Mesh { quads, counts })
            }
            k => return Err(WireError(format!("a chunk's faces of kind {k}"))),
        };
        g.end()?;
        Ok(ChunkFaces { pos, slot, mesh })
    }
}

/// The gates on the geometry's faces and the host's time in them this frame.
struct FacesGate {
    chain: GateChain,
    ms: f32,
}

/// A frame's culled faces as they leave the visibility for the GPU: the indirect draws and
/// their batches (page of the quads, first draw, draws), with the instances drawn and in all.
#[derive(Clone, Default)]
struct DrawList {
    frame_id: u64,
    draws: Vec<DrawArgs>,
    batches: Vec<(usize, u32, u32)>,
    drawn: u64,
    total: u64,
}

impl Wire for DrawList {
    fn encode(&self, out: &mut Vec<u8>) {
        out.reserve(32 + self.draws.len() * 16 + self.batches.len() * 12);
        let mut p = Put(out);
        p.u64(self.frame_id);
        p.u64(self.drawn);
        p.u64(self.total);
        p.u32(self.draws.len() as u32);
        p.bytes(bytemuck::cast_slice(&self.draws));
        p.u32(self.batches.len() as u32);
        for &(page, first, n) in &self.batches {
            p.u32(page as u32);
            p.u32(first);
            p.u32(n);
        }
    }

    fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut g = Get(bytes);
        let (frame_id, drawn, total) = (g.u64()?, g.u64()?, g.u64()?);
        let n = g.count(16)?;
        let mut draws = vec![DrawArgs::zeroed(); n];
        bytemuck::cast_slice_mut::<DrawArgs, u8>(&mut draws).copy_from_slice(g.take(n * 16)?);
        let mut batches = Vec::with_capacity(g.count(12)?.min(1 << 16));
        for _ in 0..batches.capacity() {
            let (page, first, n) = (g.u32()? as usize, g.u32()?, g.u32()?);
            if first as u64 + n as u64 > draws.len() as u64 {
                return Err(WireError("a batch of draws past the draws".into()));
            }
            batches.push((page, first, n));
        }
        g.end()?;
        Ok(DrawList { frame_id, draws, batches, drawn, total })
    }
}

/// The gates on the culled faces, the last list that arrived and the host's time in them
/// this frame.
struct DrawsGate {
    chain: GateChain,
    last: DrawList,
    ms: f32,
}

struct ChunkMesh {
    slot: u16,
    /// Its quads' page of the quad buffer and their offset in it.
    page: usize,
    offset: u32,
    counts: [u32; 6],
}

impl ChunkMesh {
    fn len(&self) -> u32 {
        self.counts.iter().sum()
    }
}

/// The region the faces' nodes share: the drawing, the meshed faces and their buffers, the
/// frame's culled draws, world and storages.
pub(crate) struct Meshing {
    opts: Options,
    live: Live,
    drawing: Draw,
    /// Geometry the faces were meshed with; `stale`: a compile-time knob (geometry, shading,
    /// meshes) changed, the drawer is rebuilt at the next frame (one rebuild for several).
    built: String,
    stale: bool,
    /// The face source's storage is bound: faces may merge across block types (`merge =
    /// exposure`).
    lookup: bool,
    /// Mesh mode: drawn into the same depth buffer.
    mesh: GpuMeshes,
    /// The quads, in pages (draw batches) of at most what the device binds.
    quads: PagedBuffer,
    chunk_buf: wgpu::Buffer,
    indirect: wgpu::Buffer,
    /// Group 0 per page of the quads (empty: stale).
    bind0: Vec<wgpu::BindGroup>,
    /// The nodes of the timed stages: geometry and culling on the CPU (the
    /// drawer's are the core's).
    geometry_nodes: Vec<String>,
    cull_nodes: Vec<String>,
    /// The GPU's side: the meshed chunks as they arrived.
    meshes: HashMap<IVec3, ChunkMesh>,
    /// The geometry's side: the chunks meshed and their slots, the ones to remesh.
    slots: HashMap<IVec3, u16>,
    dirty: HashSet<IVec3>,
    free_slots: Vec<u16>,
    /// The gates on the faces between the geometry and the GPU, if any.
    gate: Option<FacesGate>,
    /// The gates on the culled faces between the visibility and the drawer, if any.
    draws_gate: Option<DrawsGate>,
    /// This frame's state: what its nodes did so far, its counters and its groups.
    frame: Frame,
}

/// What a frame's nodes share.
#[derive(Default)]
struct Frame {
    culled: bool,
    stored: bool,
    meshed: bool,
    drawn: bool,
    /// Meshes drawn this frame (the meshes stage draws them, `depth`, or not, `none`).
    want_meshes: bool,
    view_proj: Mat4,
    /// The frame's id and time (the culled faces' gates).
    frame_id: u64,
    time: f64,
    /// The camera's block and its fraction.
    camera: (IVec3, Vec3),
    world: Option<wgpu::BindGroup>,
    bound: Option<Bound>,
    out: Option<wgpu::TextureView>,
    /// Indirect draws, and their batches (page, first draw, draws); instances drawn and in
    /// all; chunks remeshed.
    draws: u32,
    batches: Vec<(usize, u32, u32)>,
    drawn_instances: u64,
    total: u64,
    remeshed: usize,
    mesh_ms: f64,
    cull_ms: f32,
}

impl Meshing {
    fn new(p: &InitParams, devices: &Devices, waker: Waker, g: &Checked) -> anyhow::Result<(Core, Self, Option<crate::remote::Far>)> {
        let opts: Options = p.options()?;
        let node = |role| g.find(role).ok_or_else(|| anyhow::anyhow!("no {role:?} node"));
        let (geometry_id, draw_id) = (node(Role::Geometry)?, node(Role::Draw)?);
        let gate = g.gates_on(node(Role::Visibility)?, "faces").map(|e| FacesGate { chain: GateChain::new(&e.edge(), &e.gates), ms: 0.0 });
        let draws_gate = g.gates_on(node(Role::Draw)?, "faces").map(|e| DrawsGate { chain: GateChain::new(&e.edge(), &e.gates), last: DrawList::default(), ms: 0.0 });
        let geometry = g.node(geometry_id).component.clone();
        let draw = g.node(draw_id).component.clone();
        let shading = g.node(g.shading()).component.clone();
        let meshes_node = g.find(Role::Meshes).map_or("none", |id| g.node(id).component.as_str()).to_string();
        let gk: GeometryKnobs = g.knobs(geometry_id).map_err(anyhow::Error::msg)?;
        let dk: DrawKnobs = g.knobs(draw_id).map_err(anyhow::Error::msg)?;
        let sk: ShadingKnobs = g.knobs(g.shading()).map_err(anyhow::Error::msg)?;
        let knobs = Knobs { merge: parse_merge(&gk.merge)?, cull: gk.cull };
        // The faces cross from the CPU (meshing) to the GPU (the drawer): the schedule's
        // transfer; its adapter is the per-chunk upload.
        let schedule = g.schedule().map_err(|e| anyhow::anyhow!(e))?;
        if !schedule.transfers.iter().any(|e| e.port == "faces" && g.node(&e.to).role == Role::Visibility) {
            anyhow::bail!("the faces do not cross from the CPU to the GPU");
        }
        let (mut walks, shadow, bounce, mesh_shadow) = walks(g)?;
        // Faces merged across block types look their blocks up in the face source's storage.
        let lookup = g.input(Role::Visibility, "storage").map(|st| walks.storage(g, st, STORAGE_GROUPS));
        if knobs.merge == Merge::Exposure && lookup.is_none() {
            anyhow::bail!("merge = exposure: the visibility node's storage is not bound (the merged faces' blocks are looked up there)");
        }
        walks.set_max_steps(opts.max_steps);
        tracing::info!("raster-gpu: storage {}, geometry {geometry}, draw {draw}, traversal {}, shading {shading}, {} walks", walks.storages[0].component, walks.kernels[0].component, walks.kernels.len());
        let o = CoreOpts { shading: &shading, draw: &draw, shadows: sk.shadows, max_steps: opts.max_steps, pool_mb: opts.pool_mb, frames_in_flight: opts.frames_in_flight };
        let meshes = meshes_node != "none";
        let (core, drawing, far) = super::new(
            p,
            devices,
            waker,
            g,
            &o,
            dk.depth_prepass,
            SourceSpec {
                wgsl: source_wgsl(&geometry),
                walks,
                lead: None,
                lookup,
                sites: [shadow, bounce],
                varyings: false,
                strip: self::geometry(&geometry).strip,
                meshes: Some(mesh_shadow),
                draw_meshes: meshes,
                features: wgpu::Features::INDIRECT_FIRST_INSTANCE,
                depth_read: false,
                nodes: DrawNodes { draw: g.ids(&[Role::Visibility, Role::Draw]), shade: g.ids(&[Role::Shading]), extra: g.ids(&[Role::Meshes]) },
            },
        )?;
        let gpu = &core.gpu;
        let storage = |label, size: u64, extra| gpu.storage(label, size, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | extra, || "fixed sizes (the BLAS slots, the first draws)".into());
        // Pages hold fewer than 2^32 quads (offsets and instance indices are u32).
        let quads = PagedBuffer::new(gpu, "geometry: quads", 1 << 20, u32::MAX as u64 * QUAD_BYTES, (opts.quad_buffer_mb as u64) << 20, wgpu::BufferUsages::STORAGE);
        let chunk_buf = storage("chunk positions", MAX_SLOTS as u64 * 16, wgpu::BufferUsages::empty());
        let indirect = storage("indirect", 6 * 16 * 4096, wgpu::BufferUsages::INDIRECT);
        // Forward drawers shade meshes through the region's shading component (lit through the
        // mesh-shadow site's rays: `shaders::meshes`); drawing only (the shading on another
        // GPU), no meshes.
        let module = drawing.mesh_module(&core);
        let mesh = GpuMeshes::new(gpu, module.as_ref().map(|module| ForwardSpec { module, layout: &drawing.pl_draw, format: core.output_format() }));
        let s = Self {
            built: geometry.clone(),
            stale: false,
            lookup: lookup.is_some(),
            mesh,
            quads,
            chunk_buf,
            indirect,
            bind0: vec![],
            geometry_nodes: g.ids(&[Role::Geometry]),
            cull_nodes: g.ids(&[Role::Draw]),
            meshes: HashMap::new(),
            slots: HashMap::new(),
            gate,
            draws_gate,
            dirty: HashSet::new(),
            free_slots: (0..MAX_SLOTS as u32).rev().map(|s| s as u16).collect(),
            frame: Frame::default(),
            opts,
            live: Live { geometry, shading, meshes: meshes_node, knobs },
            drawing,
        };
        Ok((core, s, far))
    }

    fn knobs(&self) -> Knobs {
        self.live.knobs
    }

    fn remesh_all(&mut self) {
        self.dirty.extend(self.slots.keys().copied());
    }

    /// Remeshes dirty chunks in parallel, nearest to `near` first, at most `mesh_budget` per
    /// call, and uploads the results (the faces' transfer), through the gates on the faces
    /// if any (at `now`). Returns the chunks remeshed.
    fn remesh(&mut self, core: &Core, near: IVec3, now: f64) -> usize {
        let sent = self.mesh_dirty(core, near);
        let n = sent.iter().filter(|c| c.mesh.is_some()).count();
        let arrived = match &mut self.gate {
            Some(g) => {
                let start = Instant::now();
                let got = g.chain.gates.pass_all(now, sent);
                g.ms = start.elapsed().as_secs_f32() * 1e3;
                got
            }
            None => sent,
        };
        self.upload(core, arrived);
        n
    }

    /// The geometry's side: the dirty chunks meshed (or dropped), at most `mesh_budget`,
    /// nearest to `near` first.
    fn mesh_dirty(&mut self, core: &Core, near: IVec3) -> Vec<ChunkFaces> {
        if self.dirty.is_empty() {
            return vec![];
        }
        let mut dirty: Vec<IVec3> = self.dirty.drain().collect();
        if self.opts.mesh_budget > 0 && dirty.len() > self.opts.mesh_budget {
            dirty.sort_unstable_by_key(|p| (*p - near).length_squared());
            for p in dirty.split_off(self.opts.mesh_budget) {
                self.dirty.insert(p);
            }
        }
        let (gpu, world) = (&core.gpu, &core.world);
        let (mut work, mut out) = (vec![], vec![]);
        for pos in dirty {
            if world.chunk(pos).is_none() {
                if let Some(slot) = self.slots.remove(&pos) {
                    self.free_slots.push(slot);
                    out.push(ChunkFaces { pos, slot, mesh: None });
                }
                continue;
            }
            let slot = match self.slots.get(&pos) {
                Some(&s) => s,
                None => match self.free_slots.pop() {
                    Some(s) => {
                        self.slots.insert(pos, s);
                        s
                    }
                    None => {
                        gpu.fail(format!("geometry: more than {MAX_SLOTS} meshed chunks; frames would miss the rest; lower the client's radius"));
                        continue;
                    }
                },
            };
            work.push((pos, slot));
        }
        let (mesh_fn, knobs) = (geometry(&self.built).mesh, self.knobs());
        out.par_extend(work.par_iter().map(|&(pos, slot)| {
            let center = world.chunk(pos).unwrap();
            let neighbors = Face::ALL.map(|f| world.chunk(pos + f.normal()));
            ChunkFaces { pos, slot, mesh: Some(mesh_fn(center, neighbors, slot, knobs)) }
        }));
        out
    }

    /// The GPU's side: the chunks' faces written into the quads (a chunk's old ones freed),
    /// dropped chunks' freed.
    fn upload(&mut self, core: &Core, chunks: Vec<ChunkFaces>) {
        let gpu = &core.gpu;
        let mut staged = 0;
        for ChunkFaces { pos, slot, mesh } in chunks {
            let Some(mesh) = mesh else {
                if let Some(m) = self.meshes.remove(&pos) {
                    self.quads.free(m.page, m.offset, m.len());
                }
                continue;
            };
            let m = self.meshes.entry(pos).or_insert(ChunkMesh { slot, page: 0, offset: 0, counts: [0; 6] });
            m.slot = slot;
            gpu.queue.write_buffer(&self.chunk_buf, slot as u64 * 16, bytemuck::cast_slice(&[pos.x, pos.y, pos.z, 0]));
            let old = (m.page, m.offset, m.len());
            m.counts = [0; 6];
            self.quads.free(old.0, old.1, old.2);
            let len = mesh.quads.len() as u32;
            if len == 0 {
                continue;
            }
            let Some((page, off, grew)) = self.quads.alloc(gpu, len, QUAD_BYTES, "`quad_buffer_mb`") else { continue };
            if grew {
                self.bind0.clear();
            }
            let m = self.meshes.get_mut(&pos).unwrap();
            (m.page, m.offset) = (page, off);
            m.counts = mesh.counts;
            gpu.queue.write_buffer(self.quads.buffer(page), off as u64 * QUAD_BYTES, bytemuck::cast_slice(&mesh.quads));
            // Large uploads (a whole world at once) are sent in parts, or their staging
            // copies take as much memory as the quads.
            staged += len as u64 * QUAD_BYTES;
            if staged >= FLUSH_BYTES {
                gpu.flush_writes();
                staged = 0;
            }
        }
    }
}

/// Frustum planes (left, right, bottom, top, near) from a reverse-Z infinite projection.
fn frustum_planes(m: Mat4) -> [Vec4; 5] {
    let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));
    [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r3 - r2]
}

fn aabb_visible(planes: &[Vec4; 5], min: Vec3, max: Vec3) -> bool {
    planes.iter().all(|p| {
        let v = Vec3::new(if p.x >= 0.0 { max.x } else { min.x }, if p.y >= 0.0 { max.y } else { min.y }, if p.z >= 0.0 { max.z } else { min.z });
        p.truncate().dot(v) + p.w >= 0.0
    })
}

impl Meshing {
    /// Visibility `faces`: culls chunks and face directions on the CPU into the indirect draws.
    fn cull(&mut self, core: &Core) {
        if std::mem::replace(&mut self.frame.culled, true) {
            return;
        }
        let cull_start = Instant::now();
        let (cam_block, frac) = self.frame.camera;
        let planes = frustum_planes(self.frame.view_proj);
        let geo = geometry(&self.built);
        // A list of draws per page of the quads, one batch each.
        let mut pages: Vec<Vec<DrawArgs>> = (0..self.quads.pages()).map(|_| Vec::new()).collect();
        let (mut drawn, mut total) = (0u64, 0u64);
        for (pos, m) in &self.meshes {
            total += m.len() as u64;
            if m.len() == 0 {
                continue;
            }
            let draws = &mut pages[m.page];
            let min = (*pos * 32 - cam_block).as_vec3() - frac;
            let max = min + 32.0;
            if !aabb_visible(&planes, min, max) {
                continue;
            }
            if !geo.per_face {
                // The hardware back-face culls the hidden half of each instance.
                draws.push(DrawArgs { vertex_count: geo.verts, instance_count: m.len(), first_vertex: 0, first_instance: m.offset });
                drawn += m.len() as u64;
                continue;
            }
            let mut off = m.offset;
            for (f, &n) in m.counts.iter().enumerate() {
                let axis = f / 2;
                // Faces pointing +axis are only visible from beyond the chunk's min plane.
                let visible = if f % 2 == 0 { 0.0 > min[axis] } else { 0.0 < max[axis] };
                if n > 0 && visible {
                    draws.push(DrawArgs { vertex_count: geo.verts, instance_count: n, first_vertex: 0, first_instance: off });
                    drawn += n as u64;
                }
                off += n;
            }
        }
        let mut batches: Vec<(usize, u32, u32)> = Vec::with_capacity(pages.len());
        let mut first = 0;
        for (page, d) in pages.iter().enumerate() {
            if !d.is_empty() {
                batches.push((page, first, d.len() as u32));
                first += d.len() as u32;
            }
        }
        let mut draws = if pages.len() == 1 { pages.pop().unwrap() } else { pages.concat() };
        let mut cull_ms = cull_start.elapsed().as_secs_f32() * 1e3;
        // Through the gates on the culled faces: the last list that arrived is drawn, its
        // batches on pages the quads no longer have left out.
        if let Some(g) = &mut self.draws_gate {
            let start = Instant::now();
            let sent = DrawList { frame_id: self.frame.frame_id, draws, batches, drawn, total };
            if let Some(got) = g.chain.gates.pass(self.frame.time, Some(sent)).pop() {
                g.last = got;
            }
            let pages = self.quads.pages();
            let last = &g.last;
            (draws, drawn, total) = (last.draws.clone(), last.drawn, last.total);
            batches = last.batches.iter().copied().filter(|b| b.0 < pages).collect();
            g.ms = start.elapsed().as_secs_f32() * 1e3;
            cull_ms = start.duration_since(cull_start).as_secs_f32() * 1e3;
        }
        let bytes = (draws.len() * 16) as u64;
        if bytes > self.indirect.size() {
            self.indirect = core.gpu.storage("indirect", bytes.next_power_of_two(), wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::INDIRECT, || {
                format!("the draws in view ({}, 16 bytes each)", draws.len())
            });
        }
        if !draws.is_empty() {
            core.gpu.queue.write_buffer(&self.indirect, 0, bytemuck::cast_slice(&draws));
        }
        let f = &mut self.frame;
        (f.draws, f.batches, f.drawn_instances, f.total) = (draws.len() as u32, batches, drawn, total);
        f.cull_ms = cull_ms;
    }

    /// The storages' groups for the shadow rays (and the meshes' occlusion), after the
    /// acceleration structures.
    fn store(&mut self, core: &mut Core, enc: &mut wgpu::CommandEncoder, ticket: mr_gpu::Ticket, req: &FrameRequest) {
        if std::mem::replace(&mut self.frame.stored, true) {
            return;
        }
        let bound = core.storages.prepare(&core.gpu, &core.world);
        self.frame.bound = Some(bound);
        if core.build_accel(enc, ticket, req) {
            self.bind0.clear();
        }
        if self.bind0.is_empty() {
            self.bind0 = (0..self.quads.pages()).map(|p| self.drawing.bind(core, self.quads.buffer(p), &self.chunk_buf)).collect();
        }
    }

    /// The meshes' frame (mesh mode), drawn into the faces' depth.
    fn prepare_meshes(&mut self, core: &Core, req: &FrameRequest) {
        if std::mem::replace(&mut self.frame.meshed, true) {
            return;
        }
        if self.frame.want_meshes {
            // The images group is built each frame, so a new vertex buffer needs nothing more.
            self.mesh.prepare(&core.gpu, &req.camera, self.frame.view_proj, req.mode);
        }
    }

    /// The drawer's passes over the frame's groups.
    fn encode(&self, core: &Core, ticket: mr_gpu::Ticket, req: &FrameRequest, pass: impl FnOnce(&DrawFrame)) {
        let f = &self.frame;
        let bound = f.bound.as_ref().expect("stored");
        let (bind2, more) = bound.split();
        let binds: Vec<&wgpu::BindGroup> = [self.bind0.first(), f.world.as_ref(), bind2].into_iter().flatten().collect();
        let sky = core.sky();
        let frame = DrawFrame {
            out: f.out.as_ref().expect("set up"),
            depth: &self.drawing.depth,
            ids: self.drawing.ids(core),
            clear: wgpu::Color { r: sky[0], g: sky[1], b: sky[2], a: 1.0 },
            prepass: self.drawing.depth_prepass && req.mode == RenderMode::Lit,
            keep_depth: false,
            binds: &binds,
            more: &more,
            // Bound where the images group has them (not drawing only: no mesh capability).
            mesh_verts: self.drawing.meshed.as_ref().map(|_| self.mesh.vertices()),
            ring: &core.ring,
            ticket,
            ts: (true, true),
            nodes: &self.drawing.draw_nodes,
        };
        pass(&frame);
    }

    /// Meshes go into the faces' forward passes, and into their ids or records when deferred.
    fn draws_meshes(&self) -> bool {
        self.frame.want_meshes && (!self.drawing.drawer.mode().deferred() || self.mesh.triangles() > 0)
    }

    /// The draw node: the faces with one multi-draw-indirect per page of the quads, each
    /// binding its page (and the meshes), shaded in the fragments when forward.
    fn draw(&mut self, core: &mut Core, enc: &mut wgpu::CommandEncoder, ticket: mr_gpu::Ticket, req: &FrameRequest) {
        self.cull(core);
        self.store(core, enc, ticket, req);
        self.prepare_meshes(core, req);
        if std::mem::replace(&mut self.frame.drawn, true) {
            return;
        }
        let (batches, binds) = (&self.frame.batches, &self.bind0);
        let draw = |pass: &mut wgpu::RenderPass<'_>| {
            for &(page, first, n) in batches {
                if binds.len() > 1 {
                    pass.set_bind_group(0, &binds[page], &[]);
                }
                pass.multi_draw_indirect(&self.indirect, first as u64 * 16, n);
            }
            if binds.len() > 1 {
                pass.set_bind_group(0, &binds[0], &[]);
            }
        };
        let mesh_draw = |pass: &mut wgpu::RenderPass<'_>, mp: MeshPass| self.mesh.draw(pass, mp);
        let meshes = self.draws_meshes().then_some(&mesh_draw as &dyn Fn(&mut wgpu::RenderPass<'_>, MeshPass));
        self.encode(core, ticket, req, |f| {
            self.drawing.drawer.encode_draw(enc, f, &draw, meshes, None);
            self.drawing.drawer.encode_decode(&core.gpu, enc, f);
        });
    }

    /// The shading node: the deferred drawers' pass over the ids or records.
    fn shade(&mut self, core: &mut Core, enc: &mut wgpu::CommandEncoder, ticket: mr_gpu::Ticket, req: &FrameRequest) {
        self.draw(core, enc, ticket, req);
        let meshes = self.draws_meshes();
        let lit = req.mode == RenderMode::Lit;
        self.encode(core, ticket, req, |f| self.drawing.drawer.encode_shade(&core.gpu, enc, f, meshes, lit));
    }
}

impl State for Meshing {
    fn updated(&mut self, core: &Core, changed: &[IVec3]) -> anyhow::Result<()> {
        for &pos in changed {
            self.dirty.insert(pos);
            for f in Face::ALL {
                let nb = pos + f.normal();
                if self.slots.contains_key(&nb) || core.world.chunk(nb).is_some() {
                    self.dirty.insert(nb);
                }
            }
        }
        Ok(())
    }

    fn update_meshes(&mut self, update: MeshUpdate) {
        self.mesh.apply(update);
    }

    fn set_param(&mut self, core: &mut Core, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        self.set_knob(name, value).or_else(|| self.drawing.set_param(core, name, value))
    }

    /// The geometry the drawer draws, the shading and the meshes are compiled in (rebuilt at
    /// the next frame); the drawer is the drawing's.
    fn switch(&mut self, core: &mut Core, s: &mr_components::graph::Switch) -> Option<anyhow::Result<()>> {
        let field = match s.role {
            Role::Geometry => &mut self.live.geometry,
            Role::Shading => &mut self.live.shading,
            Role::Meshes => &mut self.live.meshes,
            _ => return self.drawing.switch(core, s),
        };
        let v = match (&s.component, s.role) {
            (Some(c), _) => c.clone(),
            (None, Role::Meshes) => "none".into(),
            (None, _) => return None,
        };
        self.stale |= *field != v;
        *field = v;
        Some(Ok(()))
    }

    fn resize(&mut self, core: &Core) {
        self.drawing.resize(core);
        self.bind0.clear();
    }

    fn memory(&self, core: &Core) -> (u64, u64) {
        (0, self.quads.size() + self.chunk_buf.size() + self.indirect.size() + self.drawing.memory_bytes(core))
    }
}

impl Meshing {
    /// Its own knobs; `None` if `name` is not one.
    fn set_knob(&mut self, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        let s = value.as_str().unwrap_or("").to_string();
        Some(match name {
            "mesh_budget" => {
                self.opts.mesh_budget = value.as_i64().unwrap_or(0) as usize;
                Ok(())
            }
            "merge" => parse_merge(&s).and_then(|m| {
                anyhow::ensure!(m != Merge::Exposure || self.lookup, "merge = exposure: the visibility node's storage is not bound (the merged faces' blocks are looked up there)");
                if m != self.live.knobs.merge {
                    self.live.knobs.merge = m;
                    self.remesh_all();
                }
                Ok(())
            }),
            "cull" => {
                let c = value.as_bool().unwrap_or(true);
                if c != self.live.knobs.cull {
                    self.live.knobs.cull = c;
                    self.remesh_all();
                }
                Ok(())
            }
            _ => return None,
        })
    }
}

impl Region for Meshing {
    /// The drawer when a compile-time knob changed it (and the faces when the geometry did).
    fn compile(&mut self, core: &mut Core, req: &FrameRequest) -> anyhow::Result<()> {
        if self.stale {
            let o = &self.live;
            if self.drawing.rebuild(core, source_wgsl(&o.geometry), geometry(&o.geometry).strip, &o.shading, o.meshes != "none")? {
                self.bind0.clear();
            }
            // The forward mesh passes shade through the drawer's shading.
            self.mesh.set_forward(&core.gpu, ForwardSpec { module: &self.drawing.mesh_module(core).expect("raster-gpu has meshes"), layout: &self.drawing.pl_draw, format: core.output_format() });
            if o.geometry != self.built {
                self.built = o.geometry.clone();
                self.remesh_all();
            }
            self.stale = false;
        }
        if req.meshes {
            self.drawing.mesh_shade(core, req.mode == RenderMode::Lit)?;
        }
        self.drawing.check_state(req.mode, None)
    }

    /// The geometry on the CPU (the dirty chunks remeshed and uploaded: the faces' transfer),
    /// then the frame's camera and world.
    fn setup(&mut self, core: &mut Core, req: &FrameRequest) {
        let start = Instant::now();
        let remeshed = self.remesh(core, req.camera.split_position().0 >> 5, req.time);
        if self.quads.pages() > 1 && self.drawing.drawer.mode() == DrawMode::Visbuffer {
            // Its ids name an instance of one page (at most 2^28, about what one binds), and
            // its shading pass binds one.
            core.gpu.fail(format!("visbuffer: the quads take {} buffers; `visbuffer` reads one (draw = raster or gbuffer read any number)", self.quads.pages()));
        }
        let mesh_ms = start.elapsed().as_secs_f64() * 1e3;
        let c = &req.camera;
        self.drawing.write_frame(core, c, req.mode, IVec3::ZERO);
        core.accum.begin(&core.gpu, c, req.mode);
        self.frame = Frame {
            want_meshes: req.meshes && self.live.meshes != "none",
            view_proj: c.projection(core.width as f32 / core.height as f32) * c.view_rotation(),
            camera: c.split_position(),
            world: Some(core.world.prepare(&core.gpu, c).clone()),
            out: Some(core.output.create_view(&Default::default())),
            frame_id: req.frame_id,
            time: req.time,
            remeshed,
            mesh_ms,
            ..Frame::default()
        };
    }

    fn stats(&self, core: &Core, req: &FrameRequest) -> (Vec<(String, f64)>, Option<Legend>, Vec<StageTime>) {
        let f = &self.frame;
        let (accel, accel_stages) = core.accel_stats();
        let counters = vec![
            ("instances".into(), f.drawn_instances as f64),
            ("instances_total".into(), f.total as f64),
            ("draws".into(), f.draws as f64),
            ("remeshed".into(), f.remeshed as f64),
            ("mesh_ms".into(), f.mesh_ms),
            ("mesh_triangles".into(), if f.want_meshes { self.mesh.triangles() as f64 } else { 0.0 }),
        ]
        .into_iter()
        .chain(core.storages.counters())
        .chain(accel)
        .chain(core.accum.counter(req.mode))
        .collect();
        let chains = self.gate.as_ref().map(|g| (&g.chain, g.ms)).into_iter().chain(self.draws_gate.as_ref().map(|g| (&g.chain, g.ms)));
        let (nodes, ms) = chains.fold((Vec::new(), 0.0), |(mut nodes, ms), (c, t)| {
            nodes.extend(c.nodes.iter().map(|(id, _)| id.clone()));
            (nodes, ms + t)
        });
        let gate = (!nodes.is_empty()).then(|| StageTime::cpu("gate", &nodes, ms));
        let stages = [StageTime::cpu("geometry", &self.geometry_nodes, f.mesh_ms as f32), StageTime::cpu("cull", &self.cull_nodes, f.cull_ms)].into_iter().chain(gate).chain(accel_stages).collect();
        (counters, None, stages)
    }

    fn gates(&self) -> Vec<GateReport> {
        let geometry = self.gate.as_ref().map_or(vec![], |g| g.chain.reports());
        geometry.into_iter().chain(self.draws_gate.as_ref().map_or(vec![], |g| g.chain.reports())).collect()
    }

    fn finish(&mut self, _: &Core, _: &mut wgpu::CommandEncoder, _: mr_gpu::Ticket) {}
}

/// Visibility `faces`: the CPU culling into the indirect draws.
struct Visibility;
/// The storages the shadow rays walk (after the acceleration structures).
struct Storage;
/// The meshes' frame (mesh mode).
struct Meshes;
/// The face drawer's draw passes.
struct DrawFaces;
/// The deferred drawers' shading pass.
struct Shading;

impl Node<Meshing> for Visibility {
    fn run(&mut self, cx: &mut Cx<Meshing>) {
        cx.region.cull(cx.core);
    }
}

impl Node<Meshing> for Storage {
    fn run(&mut self, cx: &mut Cx<Meshing>) {
        cx.region.store(cx.core, cx.enc, cx.ticket, cx.req);
    }
}

impl Node<Meshing> for Meshes {
    fn run(&mut self, cx: &mut Cx<Meshing>) {
        cx.region.prepare_meshes(cx.core, cx.req);
    }
}

impl Node<Meshing> for DrawFaces {
    fn run(&mut self, cx: &mut Cx<Meshing>) {
        cx.region.draw(cx.core, cx.enc, cx.ticket, cx.req);
    }
}

impl Node<Meshing> for Shading {
    fn run(&mut self, cx: &mut Cx<Meshing>) {
        cx.region.shade(cx.core, cx.enc, cx.ticket, cx.req);
    }
}

/// The meshing executor for `g`: culling, storages, meshes, drawing and shading as nodes (the
/// geometry is meshed on the CPU and uploaded before them: the geometry node and its transfer).
pub(crate) fn create(p: &InitParams, devices: &Devices, waker: Waker, g: &Checked) -> anyhow::Result<Box<dyn Renderer>> {
    let passes = exec::passes(g, crate::home_slot(g), |role| -> Option<Box<dyn Node<Meshing>>> {
        Some(match role {
            Role::Visibility => Box::new(Visibility),
            Role::Storage => Box::new(Storage),
            Role::Meshes => Box::new(Meshes),
            Role::Draw => Box::new(DrawFaces),
            Role::Shading => Box::new(Shading),
            _ => return None,
        })
    })?;
    crate::exec::run_or_split(Meshing::new(p, devices, waker, g)?, g, passes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every composition's shader composes from its modules and validates (no GPU needed).
    #[test]
    fn programs_compose() {
        use mr_components::graph::Change;
        let base = mr_components::graph::shipped("raster-gpu").unwrap();
        let mut n = 0;
        for st in base.choices("storage") {
            for k in base.choices("kernel") {
                for g in base.choices("geometry") {
                    for shading in base.choices("shading") {
                        let bounce = mr_components::shading::state::stateful(shading).then(|| Change::bind("shading", "bounce", "kernel"));
                        let edits: Vec<Change> = [("storage", st), ("kernel", k), ("geometry", g), ("shading", shading)].into_iter().map(|(n, c)| Change::component(n, c)).chain(bounce).collect();
                        let mut graphs = vec![base.with_edits(&edits).unwrap()];
                        // Bounces, mesh shadows and the merged faces' lookup through walks and a
                        // storage of their own.
                        if shading == "pathtrace" && g == "greedy" {
                            let mut gr = graphs[0].clone();
                            let node = |role, c: &str, ins: &[(&str, &str)]| {
                                let mut n = mr_components::graph::Node::new(role, c);
                                n.inputs.extend(ins.iter().map(|&(p, s)| (p.to_string(), s.to_string())));
                                n
                            };
                            gr.nodes.insert("b.storage".into(), node(Role::Storage, "distance", &[]));
                            gr.nodes.insert("b.kernel".into(), node(Role::Traversal, "sphere", &[("storage", "b.storage")]));
                            gr.nodes.insert("m.kernel".into(), node(Role::Traversal, "incremental", &[("storage", "storage")]));
                            gr.nodes.get_mut("shading").unwrap().inputs.insert("bounce".into(), "b.kernel".into());
                            gr.nodes.get_mut("meshes").unwrap().inputs.insert("shadow".into(), "m.kernel".into());
                            gr.nodes.insert("l.storage".into(), node(Role::Storage, "rows", &[]));
                            gr.nodes.get_mut("visibility").unwrap().inputs.insert("storage".into(), "l.storage".into());
                            graphs.push(gr);
                        }
                        for gr in graphs {
                            let ck = gr.check().unwrap();
                            gr.validate().unwrap_or_else(|e| panic!("{gr:?}: {e}"));
                            let what = format!("raster-gpu {st} {k} {g} {shading}");
                            let (mut walks, shadow, bounce, mesh_shadow) = walks(&ck).unwrap();
                            let lookup = ck.input(Role::Visibility, "storage").map(|st| walks.storage(&ck, st, STORAGE_GROUPS));
                            // The drawer (with mesh records for deferred drawers, and without).
                            let (program, deps, meshed) = crate::faces::programs(&walks, None, lookup, [shadow, bounce], Some(mesh_shadow), shading);
                            let draw_src = crate::faces::draw_src(&source_wgsl(g), false);
                            // The merged faces' blocks come from the face source's storage.
                            if let Some(st) = lookup {
                                let d: Vec<&str> = deps.iter().map(String::as_str).collect();
                                let text = program.text(&draw_src, &d).unwrap();
                                let want = format!("#import {}::{{s_block_at}}", walks.storage_path(st));
                                assert!(text.split("// ---- root").nth(1).unwrap().contains(&want), "{what}: {want}");
                            }
                            crate::faces::tests::drawers(&program, &deps, meshed.as_ref(), &draw_src, shading, &what, &base.choices("draw"), (&walks, None, lookup));
                            n += 1;
                        }
                    }
                }
            }
        }
        assert!(n > 30, "{n}");
    }
}
