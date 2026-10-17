//! The ray-query lane (`gpu-rt`; the primary rays' kernel API `rayquery`): hardware ray
//! tracing (RT cores) through wgpu ray queries, assembled from components (`mr-components`,
//! IDEAS §12):
//!
//! * **geometry** decides what goes into each chunk's BLAS: `greedy` / `naive` quads as
//!   triangles (traversal and intersection entirely in fixed-function hardware), or `boxes`,
//!   one AABB per occupied 4^3 brick with a small software DDA inside each candidate
//!   (procedural geometry);
//! * **visibility** `raycast` with the **traversal** kernel `rayquery`: primary, shadow and
//!   bounce rays are ray queries;
//! * **shading**: the same `shade_surface` as the other back ends (`direct`, `pathtrace`).
//!
//! The TLAS is rebuilt every frame with instances translated relative to the camera's
//! block, so precision never degrades far from the origin. Replaces gpu-rtx and
//! gpu-rtx-bricks.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use glam::IVec3;
use mr_components::geometry::{Knobs, Merge, RtKind};
use mr_components::graph::{self, Role};
use mr_components::shading::state as shade_state;
use mr_components::{Slot, components, names_on, placement_param, require, traversal};
use mr_core::chunk::cell_of_index;
use mr_core::*;
use mr_gpu::voxels::{Shadows, VoxelWorld, shadows_param};
use mr_gpu::{Gpu, GpuNeeds, GrowableBuffer, RangeAlloc, wgpu};
use rayon::prelude::*;
use serde::Deserialize;
use wgpu::util::DeviceExt;

use super::{Before, Cast, CastProgram, Passes, Ticket};
use crate::{Core, CoreSpec, LANE_BINDING};

pub(crate) const BACKEND: &str = "gpu-rt";

pub(crate) fn info() -> RendererInfo {
    RendererInfo {
        name: BACKEND.into(),
        description: "GPU-RT back end: hardware ray queries over a TLAS of per-chunk BLASes built from the geometry component (triangles or brick boxes); shading components as elsewhere".into(),
        tags: vec!["gpu".into(), "raytrace".into(), "hardware-rt".into(), "modular".into(), "wgpu".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: mr_components::lighting_knobs(BACKEND, params()),
    }
}

fn describe(slot: Slot) -> String {
    components(slot).filter(|c| mr_components::unavailable(BACKEND, slot, c.name).is_none()).map(|c| format!("{}: {}", c.name, c.description)).collect::<Vec<_>>().join(" ")
}

fn params() -> Vec<ParamSpec> {
    vec![
        ParamSpec::choice("geometry", &format!("Geometry component (what the BLASes hold). {}", describe(Slot::Geometry)), &names_on(BACKEND, Slot::Geometry), "greedy", false),
        ParamSpec::choice("visibility", &format!("Visibility strategy. {}", describe(Slot::Visibility)), &names_on(BACKEND, Slot::Visibility), "raycast", false),
        ParamSpec::choice("traversal", &format!("Traversal kernel. {}", describe(Slot::Traversal)), &names_on(BACKEND, Slot::Traversal), "rayquery", false),
        ParamSpec::choice("shading", &format!("Shading component. {}", describe(Slot::Shading)), &names_on(BACKEND, Slot::Shading), "direct", true),
        ParamSpec::choice("merge", "greedy: merge faces of the same block only (`exposure` is raster-only)", &["block"], "block", false).of("geometry", &["greedy"]),
        ParamSpec::bool("cull", "naive: skip faces hidden by solid neighbours (false = every face of every solid block)", true, false).of("geometry", &["naive"]),
        ParamSpec::choice("cull_faces", "Triangle facing culled for primary rays (triangle geometries)", &["back", "front", "none"], "back", true).of("geometry", &["greedy", "naive"]),
        shadows_param(),
        ParamSpec::bool("fast_build", "Prefer fast BVH builds over fast traversal (a build flag, not code)", false, true).baked(false),
        ParamSpec::int("build_budget", "Chunk BLAS rebuilds per frame, nearest first (0 = all)", 0, 100000, 0, true),
    ]
    .into_iter()
    .chain(shade_state::params())
    .chain([
        placement_param(BACKEND, "inline"),
        ParamSpec::int("pool_mb", "Voxel pool limit (MiB; boxes geometry)", 64, 6144, 2048, false),
        ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
    ])
    .collect()
}

#[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Cull {
    #[default]
    Back,
    Front,
    None,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    geometry: String,
    visibility: String,
    traversal: String,
    shading: String,
    placement: String,
    merge: String,
    cull: bool,
    /// Triangle facing culled for primary rays.
    cull_faces: Cull,
    shadows: Shadows,
    /// Prefer fast BVH builds over fast traversal (helps edit-heavy scenes).
    fast_build: bool,
    build_budget: usize,
    bounces: u32,
    spp: u32,
    accumulate: bool,
    /// Voxel pool for the boxes geometry.
    pool_mb: u32,
    frames_in_flight: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            geometry: "greedy".into(),
            visibility: "raycast".into(),
            traversal: "rayquery".into(),
            shading: "direct".into(),
            placement: "inline".into(),
            merge: "block".into(),
            cull: true,
            cull_faces: Cull::Back,
            shadows: Shadows::All,
            fast_build: false,
            build_budget: 0,
            bounces: 1,
            spp: 1,
            accumulate: true,
            pool_mb: 2048,
            frames_in_flight: 3,
        }
    }
}


const MAX_SLOTS: u32 = 1 << 16;
const NO_BRICK: u32 = 0xffff_ffff;

struct ChunkAs {
    slot: u32,
    blas: wgpu::Blas,
    /// Range in the shared geometry buffer (u32 units).
    range: (u32, u32),
    _inputs: Vec<wgpu::Buffer>,
}

/// CPU-side geometry for one chunk's BLAS.
enum Built {
    Triangles { vertices: Vec<[f32; 3]>, indices: Vec<u32>, quads: Vec<[u32; 2]> },
    Aabbs { boxes: Vec<[f32; 6]>, bricks: Vec<u32> },
}

fn tlas(gpu: &Gpu, cap: u32) -> wgpu::Tlas {
    gpu.device.create_tlas(&wgpu::CreateTlasDescriptor {
        label: Some("chunks"),
        max_instances: cap,
        flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
        update_mode: wgpu::AccelerationStructureUpdateMode::Build,
    })
}

/// The ray-query lane's own state: the chunks' acceleration structures and the geometry the
/// hits are decoded from.
pub(crate) struct RayQuery {
    gpu: Arc<Gpu>,
    rt: RtKind,
    knobs: Knobs,
    /// `geometry` and `traversal` components, for the shader.
    geometry: String,
    kernel: String,
    opts: Options,
    tlas: wgpu::Tlas,
    tlas_cap: u32,
    geo_buf: GrowableBuffer,
    geo_alloc: RangeAlloc,
    chunk_info: wgpu::Buffer,
    chunks: HashMap<IVec3, ChunkAs>,
    free_slots: Vec<u32>,
    dirty: HashSet<IVec3>,
    /// The bindings changed (the TLAS or the geometry buffer grew).
    rebind: bool,
    /// This frame's TLAS instances, BLAS rebuilds and their CPU time.
    instances: u32,
    rebuilt: usize,
    build_ms: f64,
    /// The nodes of the lane's timed stage: the acceleration structures (PLAN step 11).
    stage_nodes: Vec<String>,
}

impl RayQuery {
    pub(crate) fn new(p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked) -> anyhow::Result<(Core, Self, String)> {
        let opts: Options = p.options()?;
        let geometry = require(BACKEND, Slot::Geometry, &opts.geometry)?;
        require(BACKEND, Slot::Visibility, &opts.visibility)?;
        let kernel = require(BACKEND, Slot::Traversal, &opts.traversal)?;
        let shading = require(BACKEND, Slot::Shading, &opts.shading)?;
        anyhow::ensure!(opts.merge == "block", "merge = `{}`: gpu-rt has no per-pixel block lookup (only `block`)", opts.merge);
        let (_, rt) = mr_components::geometry::rt(&geometry).expect("validated");
        let core = Core::new(
            p,
            devices,
            waker,
            g,
            CoreSpec {
                gpu: GpuNeeds { experimental: true, required_features: wgpu::Features::EXPERIMENTAL_RAY_QUERY, optional_features: wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS },
                walks: &Default::default(),
                stages: wgpu::ShaderStages::COMPUTE,
                max_lod: 0,
                // Only the boxes' DDA reads voxels.
                voxels: matches!(rt, RtKind::Boxes),
                records: mr_components::parse_placement(&opts.placement)?,
                shading: &shading,
                bounces: opts.bounces,
                spp: opts.spp,
                accumulate: opts.accumulate,
                shadows: opts.shadows,
                max_steps: 0,
                pool_mb: opts.pool_mb,
                frames_in_flight: opts.frames_in_flight,
            },
        )?;
        let gpu = core.gpu.clone();
        let geo_buf = GrowableBuffer::new(&gpu, "geometry", 1 << 20, 1 << 30, wgpu::BufferUsages::STORAGE);
        let geo_words = (geo_buf.size() / 4) as u32;
        let chunk_info = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("chunk info"), size: MAX_SLOTS as u64 * 16, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let tlas_cap = 4096;
        tracing::info!("gpu-rt: geometry {}, shading {shading}", opts.geometry);
        let lane = Self {
            tlas: tlas(&gpu, tlas_cap),
            tlas_cap,
            geo_buf,
            rt,
            knobs: Knobs { merge: Merge::Block, cull: opts.cull },
            geometry,
            kernel,
            opts,
            geo_alloc: RangeAlloc::new(geo_words),
            chunk_info,
            chunks: HashMap::new(),
            free_slots: (0..MAX_SLOTS).rev().collect(),
            dirty: HashSet::new(),
            rebind: false,
            instances: 0,
            rebuilt: 0,
            build_ms: 0.0,
            stage_nodes: g.ids(&[Role::Geometry]),
            gpu,
        };
        Ok((core, lane, shading))
    }
}

impl Cast for RayQuery {
    const BACKEND: &'static str = BACKEND;

    fn layout_entries(&self, stages: wgpu::ShaderStages) -> Vec<wgpu::BindGroupLayoutEntry> {
        let e = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: stages, ty, count: None };
        let ro = wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None };
        vec![e(LANE_BINDING, wgpu::BindingType::AccelerationStructure { vertex_return: false }), e(LANE_BINDING + 1, ro), e(LANE_BINDING + 2, ro)]
    }

    fn bind_entries(&self) -> Vec<wgpu::BindGroupEntry<'_>> {
        vec![
            wgpu::BindGroupEntry { binding: LANE_BINDING, resource: wgpu::BindingResource::AccelerationStructure(&self.tlas) },
            wgpu::BindGroupEntry { binding: LANE_BINDING + 1, resource: self.geo_buf.buffer().as_entire_binding() },
            wgpu::BindGroupEntry { binding: LANE_BINDING + 2, resource: self.chunk_info.as_entire_binding() },
        ]
    }

    fn layouts<'a>(&'a self, _core: &'a Core, [layout0, world]: [&'a wgpu::BindGroupLayout; 2]) -> Vec<Option<&'a wgpu::BindGroupLayout>> {
        vec![Some(layout0), Some(world)]
    }

    fn program(&self, shading: &str, _meshed: bool) -> CastProgram {
        let (geo_wgsl, _) = mr_components::geometry::rt(&self.geometry).expect("validated");
        program(&self.geometry, &self.kernel, geo_wgsl, traversal::rt_wgsl(&self.kernel).expect("validated"), shading)
    }

    fn stale(&self) -> bool {
        false
    }

    fn compiled(&mut self) {}

    fn camera(&self) -> [u32; 2] {
        let cull = match self.opts.cull_faces {
            Cull::Back => 0x10,
            Cull::Front => 0x20,
            Cull::None => 0,
        };
        [0, if let RtKind::Triangles(_) = self.rt { cull } else { 0 }]
    }

    fn updated(&mut self, core: &Core, changed: &[IVec3]) -> anyhow::Result<()> {
        for &pos in changed {
            self.dirty.insert(pos);
            if let RtKind::Triangles(_) = self.rt {
                // Face culling depends on neighbours.
                for f in Face::ALL {
                    let nb = pos + f.normal();
                    if self.chunks.contains_key(&nb) || core.world.chunk(nb).is_some() {
                        self.dirty.insert(nb);
                    }
                }
            }
        }
        Ok(())
    }

    fn prepare(&mut self, core: &mut Core, passes: &mut Passes, enc: &mut wgpu::CommandEncoder, ticket: Ticket, req: &FrameRequest) {
        let start = Instant::now();
        let (cam_block, _) = req.camera.split_position();
        // GPU time should include the acceleration-structure builds, which happen outside
        // any pass: start the clock in the encoder when the device allows it.
        if let (true, Some((qs, begin, _))) = (self.encoder_ts(), core.ring.timestamps(ticket)) {
            enc.write_timestamp(qs, begin);
        }
        core.ring.stage(enc, ticket, "geometry", &self.stage_nodes);
        self.rebuilt = self.rebuild(&core.world, enc, cam_block >> 5);
        self.build_ms = start.elapsed().as_secs_f64() * 1e3;
        // TLAS: one instance per chunk, translated relative to the camera block.
        let n = self.chunks.len() as u32;
        if n > self.tlas_cap {
            self.tlas_cap = n.next_power_of_two();
            self.tlas = tlas(&self.gpu, self.tlas_cap);
            self.rebind = true;
        }
        for (k, (pos, ch)) in self.chunks.iter().enumerate() {
            let t = (*pos * 32 - cam_block).as_vec3();
            let transform = [1.0, 0.0, 0.0, t.x, 0.0, 1.0, 0.0, t.y, 0.0, 0.0, 1.0, t.z];
            self.tlas[k] = Some(wgpu::TlasInstance::new(&ch.blas, transform, ch.slot, 0xff));
        }
        let used = self.tlas.get().iter().filter(|i| i.is_some()).count() as u32;
        for k in n..used.max(n) {
            self.tlas[k as usize] = None;
        }
        enc.build_acceleration_structures(std::iter::empty(), std::iter::once(&self.tlas));
        self.instances = n;
        if std::mem::take(&mut self.rebind) {
            passes.unbind();
        }
    }

    fn before(&mut self, _core: &mut Core, _passes: &Passes, _enc: &mut wgpu::CommandEncoder, _ticket: Ticket, _req: &FrameRequest, _world: &wgpu::BindGroup) -> Before {
        Before { first: !self.encoder_ts(), meshed: false }
    }

    fn groups(&self) -> impl Iterator<Item = (u32, &wgpu::BindGroup)> {
        std::iter::empty()
    }

    fn stats(&self, _core: &Core) -> (Vec<(String, f64)>, Option<Legend>, Vec<StageTime>) {
        let counters = vec![("instances".into(), self.instances as f64), ("blas_rebuilt".into(), self.rebuilt as f64), ("build_ms".into(), self.build_ms)];
        (counters, None, vec![StageTime::cpu("geometry", &self.stage_nodes, self.build_ms as f32)])
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        match name {
            "fast_build" => self.opts.fast_build = value.as_bool().unwrap_or(false),
            "build_budget" => self.opts.build_budget = value.as_i64().unwrap_or(0) as usize,
            "cull_faces" => {
                self.opts.cull_faces = match value.as_str() {
                    Some("front") => Cull::Front,
                    Some("none") => Cull::None,
                    _ => Cull::Back,
                }
            }
            _ => return None,
        }
        Some(Ok(()))
    }

    fn resize(&mut self, _core: &Core) {}

    fn memory_bytes(&self) -> u64 {
        // BLAS sizes are driver-internal; BLAS input buffers are a lower bound.
        let inputs: u64 = self.chunks.values().flat_map(|c| c._inputs.iter()).map(|b| b.size()).sum();
        self.geo_buf.size() + self.chunk_info.size() + inputs
    }
}

/// The shader: the world, lighting and the shading component as modules; the ray services
/// (`rt::rays`) virtual functions the root overrides with the ray-query kernel, which (like
/// the geometry's `g_rt_trace`) stays in the root: naga cannot write ray queries back into
/// a module.
fn program(geometry: &str, vis: &str, geo_wgsl: &str, vis_wgsl: &str, shading: &str) -> CastProgram {
    use mr_components::shaders;
    let mut program = mr_gpu::program::Program::world(Some("rt::rays"));
    program.add(
        "rt::rays",
        "virtual fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool { return false; }\n\
         virtual fn strace(o: vec3<f32>, d: vec3<f32>, max_t: f32) -> Hit { var h: Hit; return h; }\n",
        &[mr_gpu::program::VOXELS],
    );
    shaders::shading(&mut program, shading, &["rt::rays"], 0, crate::STATE_BINDING);
    let root = format!("enable wgpu_ray_query;\n// Composition: geometry = {geometry}, traversal = {vis}, shading = {shading}\n{}\n{geo_wgsl}\n{vis_wgsl}", include_str!("rayquery.wgsl"));
    CastProgram { program, root, deps: ROOT_DEPS.iter().map(|d| d.to_string()).collect() }
}

const ROOT_DEPS: &[&str] = &[mr_components::shaders::SHADING, mr_components::shaders::STATE, "rt::rays", mr_gpu::program::LIGHTING, mr_gpu::program::VOXELS, mr_gpu::program::COMMON];

impl RayQuery {
    /// The device writes timestamps inside encoders (the frame's clock starts before the
    /// acceleration-structure builds).
    fn encoder_ts(&self) -> bool {
        self.gpu.device.features().contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS)
    }

    fn drop_chunk(&mut self, pos: IVec3) {
        if let Some(c) = self.chunks.remove(&pos) {
            self.geo_alloc.free(c.range.0, c.range.1);
            self.free_slots.push(c.slot);
        }
    }

    fn build_geometry(&self, world: &VoxelWorld, pos: IVec3) -> Option<Built> {
        match self.rt {
            RtKind::Triangles(mesh) => {
                let center = world.chunk(pos)?;
                let neighbors = Face::ALL.map(|f| world.chunk(pos + f.normal()));
                let mesh = mesh(center, neighbors, 0, self.knobs);
                if mesh.quads.is_empty() {
                    return None;
                }
                let mut vertices = Vec::with_capacity(mesh.quads.len() * 4);
                let mut indices = Vec::with_capacity(mesh.quads.len() * 6);
                for q in &mesh.quads {
                    let base = vertices.len() as u32;
                    vertices.extend(mr_mesh::Quad::decode(*q).corners().map(|c| c.to_array()));
                    indices.extend([base, base + 1, base + 2, base + 2, base + 1, base + 3]);
                }
                Some(Built::Triangles { vertices, indices, quads: mesh.quads })
            }
            RtKind::Boxes => match world.chunk(pos)? {
                ChunkData::Uniform(b) if b.is_air() => None,
                ChunkData::Uniform(_) => Some(Built::Aabbs { boxes: vec![[0.0, 0.0, 0.0, 32.0, 32.0, 32.0]], bricks: vec![NO_BRICK] }),
                ChunkData::Dense(d) => {
                    let mut occupied = [false; 512];
                    for (i, b) in d.0.iter().enumerate() {
                        if b.is_solid() {
                            let l: IVec3 = cell_of_index(i) >> 2;
                            occupied[(l.x | (l.z << 3) | (l.y << 6)) as usize] = true;
                        }
                    }
                    let mut boxes = vec![];
                    let mut bricks = vec![];
                    for (bi, _) in occupied.iter().enumerate().filter(|(_, o)| **o) {
                        let b = bi as i32;
                        let lo = IVec3::new(b & 7, (b >> 6) & 7, (b >> 3) & 7).as_vec3() * 4.0;
                        boxes.push([lo.x, lo.y, lo.z, lo.x + 4.0, lo.y + 4.0, lo.z + 4.0]);
                        bricks.push(bi as u32);
                    }
                    (!boxes.is_empty()).then_some(Built::Aabbs { boxes, bricks })
                }
            },
        }
    }

    /// Rebuilds BLASes of dirty chunks into `enc`. Returns how many were rebuilt.
    fn rebuild(&mut self, world: &VoxelWorld, enc: &mut wgpu::CommandEncoder, near: IVec3) -> usize {
        if self.dirty.is_empty() {
            return 0;
        }
        let mut dirty: Vec<IVec3> = self.dirty.drain().collect();
        if self.opts.build_budget > 0 && dirty.len() > self.opts.build_budget {
            dirty.sort_unstable_by_key(|p| (*p - near).length_squared());
            for p in dirty.split_off(self.opts.build_budget) {
                self.dirty.insert(p);
            }
        }
        let built: Vec<(IVec3, Option<Built>)> = dirty.par_iter().map(|&p| (p, self.build_geometry(world, p))).collect();
        let flags = if self.opts.fast_build { wgpu::AccelerationStructureFlags::PREFER_FAST_BUILD } else { wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE };
        // Everything referenced by the build entries must outlive the build call.
        let mut staged: Vec<(IVec3, u32, wgpu::Blas, wgpu::BlasGeometrySizeDescriptors, Vec<wgpu::Buffer>, (u32, u32))> = vec![];
        for (pos, b) in built {
            let old_slot = self.chunks.get(&pos).map(|c| c.slot);
            if let Some(c) = self.chunks.remove(&pos) {
                self.geo_alloc.free(c.range.0, c.range.1);
                if b.is_none() {
                    self.free_slots.push(c.slot);
                }
            }
            let Some(b) = b else { continue };
            let Some(slot) = old_slot.or_else(|| self.free_slots.pop()) else { continue };
            let usage = wgpu::BufferUsages::BLAS_INPUT;
            let (sizes, inputs, words, info) = match &b {
                Built::Triangles { vertices, indices, quads } => {
                    let vb = self.gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(vertices), usage });
                    let ib = self.gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(indices), usage });
                    let sizes = wgpu::BlasGeometrySizeDescriptors::Triangles {
                        descriptors: vec![wgpu::BlasTriangleGeometrySizeDescriptor {
                            vertex_format: wgpu::VertexFormat::Float32x3,
                            vertex_count: vertices.len() as u32,
                            index_format: Some(wgpu::IndexFormat::Uint32),
                            index_count: Some(indices.len() as u32),
                            flags: wgpu::AccelerationStructureGeometryFlags::OPAQUE,
                        }],
                    };
                    (sizes, vec![vb, ib], bytemuck::cast_slice::<[u32; 2], u32>(quads).to_vec(), None)
                }
                Built::Aabbs { boxes, bricks } => {
                    let ab = self.gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(boxes), usage });
                    let sizes = wgpu::BlasGeometrySizeDescriptors::AABBs {
                        descriptors: vec![wgpu::BlasAABBGeometrySizeDescriptor {
                            primitive_count: boxes.len() as u32,
                            flags: wgpu::AccelerationStructureGeometryFlags::OPAQUE,
                        }],
                    };
                    (sizes, vec![ab], bricks.clone(), Some(world.table_lookup(pos).unwrap_or(0xffff_ffff)))
                }
            };
            let off = match self.geo_buf.alloc(&self.gpu, &mut self.geo_alloc, words.len() as u32, 4) {
                Ok((off, grew)) => {
                    if grew {
                        self.rebind = true;
                    }
                    off
                }
                Err(e) => {
                    tracing::error!("{e:#}; chunk {pos} skipped");
                    self.free_slots.push(slot);
                    continue;
                }
            };
            self.gpu.queue.write_buffer(self.geo_buf.buffer(), off as u64 * 4, bytemuck::cast_slice(&words));
            // Triangles: first quad; boxes: chunk-table value and first brick.
            let info = match info {
                None => [off / 2, 0, 0, 0],
                Some(value) => [value, off, 0, 0],
            };
            self.gpu.queue.write_buffer(&self.chunk_info, slot as u64 * 16, bytemuck::cast_slice(&info));
            let blas = self.gpu.device.create_blas(&wgpu::CreateBlasDescriptor { label: None, flags, update_mode: wgpu::AccelerationStructureUpdateMode::Build }, sizes.clone());
            staged.push((pos, slot, blas, sizes, inputs, (off, words.len() as u32)));
        }
        let n = staged.len();
        {
            let tri_geo: Vec<Vec<wgpu::BlasTriangleGeometry>> = staged
                .iter()
                .map(|(_, _, _, sizes, inputs, _)| match sizes {
                    wgpu::BlasGeometrySizeDescriptors::Triangles { descriptors } => vec![wgpu::BlasTriangleGeometry {
                        size: &descriptors[0],
                        vertex_buffer: &inputs[0],
                        first_vertex: 0,
                        vertex_stride: 12,
                        index_buffer: Some(&inputs[1]),
                        first_index: Some(0),
                        transform_buffer: None,
                        transform_buffer_offset: None,
                    }],
                    _ => vec![],
                })
                .collect();
            let entries: Vec<wgpu::BlasBuildEntry> = staged
                .iter()
                .zip(tri_geo)
                .map(|((_, _, blas, sizes, inputs, _), tri)| wgpu::BlasBuildEntry {
                    blas,
                    geometry: match sizes {
                        wgpu::BlasGeometrySizeDescriptors::Triangles { .. } => wgpu::BlasGeometries::TriangleGeometries(tri),
                        wgpu::BlasGeometrySizeDescriptors::AABBs { descriptors } => wgpu::BlasGeometries::AabbGeometries(vec![wgpu::BlasAabbGeometry {
                            size: &descriptors[0],
                            stride: 24,
                            aabb_buffer: &inputs[0],
                            primitive_offset: 0,
                        }]),
                    },
                })
                .collect();
            enc.build_acceleration_structures(entries.iter(), std::iter::empty());
        }
        for (pos, slot, blas, _, inputs, range) in staged {
            self.chunks.insert(pos, ChunkAs { slot, blas, range, _inputs: inputs });
        }
        n
    }
}

impl Drop for RayQuery {
    fn drop(&mut self) {
        let _ = self.gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let positions: Vec<IVec3> = self.chunks.keys().copied().collect();
        for p in positions {
            self.drop_chunk(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shader composes from its modules (ray services overridden by the root's ray
    /// queries) and validates for every geometry and shading, inline and deferred (no GPU
    /// needed).
    #[test]
    fn programs_compose() {
        for g in names_on(BACKEND, Slot::Geometry) {
            let (geo_wgsl, _) = mr_components::geometry::rt(g).unwrap();
            for shading in names_on(BACKEND, Slot::Shading) {
                for deferred in [false, true] {
                    let lp = program(g, "rayquery", geo_wgsl, traversal::rt_wgsl("rayquery").unwrap(), shading);
                    super::super::tests::validate(lp, deferred, false, false, &format!("{g} {shading} {deferred}"));
                }
            }
        }
    }
}
