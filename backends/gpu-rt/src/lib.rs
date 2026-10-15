//! GPU-RT back end: hardware ray tracing (RT cores) through wgpu ray queries, assembled from
//! components (`mr-components`, IDEAS §12):
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

use bytemuck::{Pod, Zeroable};
use glam::IVec3;
use mr_components::geometry::{Knobs, Merge, RtKind};
use mr_components::shading::state::{self as shade_state, ShadeState};
use mr_components::{names_on, require, Slot, components, traversal};
use mr_core::chunk::cell_of_index;
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::voxels::{Shadows, VoxelWorld, VoxelWorldConfig, shadows_param};
use mr_gpu::{FrameRing, Gpu, GpuNeeds, GrowableBuffer, RangeAlloc, wgpu};
use rayon::prelude::*;
use serde::Deserialize;
use wgpu::util::DeviceExt;

const BACKEND: &str = "gpu-rt";

pub fn entry() -> RendererEntry {
    RendererEntry { info, create: |p, d, w| Ok(Box::new(GpuRt::new(p, d, w)?)) }
}

fn info() -> RendererInfo {
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
    components(slot).filter(|c| c.backends.contains(&BACKEND)).map(|c| format!("{}: {}", c.name, c.description)).collect::<Vec<_>>().join(" ")
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
        ParamSpec::bool("fast_build", "Prefer fast BVH builds over fast traversal", false, true),
        ParamSpec::int("build_budget", "Chunk BLAS rebuilds per frame, nearest first (0 = all)", 0, 100000, 0, true),
    ]
    .into_iter()
    .chain(shade_state::params())
    .chain([
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


#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct CameraUniform {
    forward: [f32; 3],
    tan_half_fov: f32,
    right: [f32; 3],
    aspect: f32,
    up: [f32; 3],
    mode: u32,
    origin: [f32; 3],
    sky: u32,
    width: u32,
    height: u32,
    ray_flags: u32,
    _p0: u32,
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

pub struct GpuRt {
    gpu: Arc<Gpu>,
    world: VoxelWorld,
    rt: RtKind,
    knobs: Knobs,
    shading: String,
    /// Shading the pipeline was built with (rebuilt lazily on a live switch).
    built_shading: String,
    opts: Options,
    source: Box<dyn Fn(&str) -> (mr_gpu::program::Program, String) + Send + Sync>,
    pipeline: wgpu::ComputePipeline,
    pipeline_layout: wgpu::PipelineLayout,
    layout0: wgpu::BindGroupLayout,
    camera_buf: wgpu::Buffer,
    output: wgpu::Texture,
    ring: FrameRing,
    waker: Waker,
    width: u32,
    height: u32,
    tlas: wgpu::Tlas,
    tlas_cap: u32,
    geo_buf: GrowableBuffer,
    geo_alloc: RangeAlloc,
    chunk_info: wgpu::Buffer,
    chunks: HashMap<IVec3, ChunkAs>,
    free_slots: Vec<u32>,
    dirty: HashSet<IVec3>,
    /// Shading state (pathtrace's accumulation).
    accum: ShadeState,
    bind0: Option<wgpu::BindGroup>,
}

fn tlas(gpu: &Gpu, cap: u32) -> wgpu::Tlas {
    gpu.device.create_tlas(&wgpu::CreateTlasDescriptor {
        label: Some("chunks"),
        max_instances: cap,
        flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
        update_mode: wgpu::AccelerationStructureUpdateMode::Build,
    })
}

/// The shader: the world, lighting and the shading component as modules; the ray services
/// (`rt::rays`) virtual functions the root overrides with the ray-query kernel, which (like
/// the geometry's `g_rt_trace`) stays in the root: naga cannot write ray queries back into
/// a module.
fn program(geometry: &str, vis: &str, geo_wgsl: &str, vis_wgsl: &str, shading: &str) -> (mr_gpu::program::Program, String) {
    use mr_components::shaders;
    let mut p = mr_gpu::program::Program::world(Some("rt::rays"));
    p.add(
        "rt::rays",
        "virtual fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool { return false; }\n\
         virtual fn strace(o: vec3<f32>, d: vec3<f32>, max_t: f32) -> Hit { var h: Hit; return h; }\n",
        &[mr_gpu::program::VOXELS],
    );
    shaders::shading(&mut p, shading, "rt::rays", 0, 5);
    let root = format!("enable wgpu_ray_query;\n// Composition: geometry = {geometry}, traversal = {vis}, shading = {shading}\n{}\n{geo_wgsl}\n{vis_wgsl}\n", include_str!("main.wgsl"));
    (p, root)
}

const ROOT_DEPS: &[&str] = &[mr_components::shaders::SHADING, mr_components::shaders::STATE, "rt::rays", mr_gpu::program::LIGHTING, mr_gpu::program::VOXELS, mr_gpu::program::COMMON];

fn pipeline(gpu: &Gpu, layout: &wgpu::PipelineLayout, (program, root): (mr_gpu::program::Program, String)) -> wgpu::ComputePipeline {
    let module = mr_gpu::program::compile(&gpu.device, "gpu-rt", &program, &root, ROOT_DEPS);
    gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("gpu-rt"),
        layout: Some(layout),
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    })
}

impl GpuRt {
    fn new(p: InitParams, devices: &Devices, waker: Waker) -> anyhow::Result<Self> {
        let opts: Options = p.options()?;
        let geometry = require(BACKEND, Slot::Geometry, &opts.geometry)?;
        require(BACKEND, Slot::Visibility, &opts.visibility)?;
        let vis = require(BACKEND, Slot::Traversal, &opts.traversal)?;
        let shading = require(BACKEND, Slot::Shading, &opts.shading)?;
        anyhow::ensure!(opts.merge == "block", "merge = `{}`: gpu-rt has no per-pixel block lookup (only `block`)", opts.merge);
        let (geo_wgsl, rt) = mr_components::geometry::rt(&geometry).expect("validated");
        // The host hands the device in (GPU slot 0); this back end only states its needs.
        let gpu = mr_gpu::gpu(devices, 0, &GpuNeeds { experimental: true, required_features: wgpu::Features::EXPERIMENTAL_RAY_QUERY, optional_features: wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS })?;
        let world = VoxelWorld::new(
            &gpu,
            &p.blocks,
            p.environment,
            VoxelWorldConfig {
                max_lod: 0,
                // Only the boxes' DDA reads voxels.
                upload_voxels: matches!(rt, RtKind::Boxes),
                pool_mb: opts.pool_mb,
                view_radius: p.view_radius_chunks,
                shadows: opts.shadows,
                max_steps: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
            },
        )?;
        let vis_wgsl = traversal::rt_wgsl(&vis).expect("validated");
        let source = Box::new(move |shading: &str| program(&geometry, &vis, geo_wgsl, vis_wgsl, shading));
        let e = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::COMPUTE, ty, count: None };
        let storage = |read_only| wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only }, has_dynamic_offset: false, min_binding_size: None };
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gpu-rt"),
            entries: &[
                e(0, wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }),
                e(
                    1,
                    wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                ),
                e(2, wgpu::BindingType::AccelerationStructure { vertex_return: false }),
                e(3, storage(true)),
                e(4, storage(true)),
                ShadeState::layout_entries(5, wgpu::ShaderStages::COMPUTE)[0],
                ShadeState::layout_entries(5, wgpu::ShaderStages::COMPUTE)[1],
            ],
        });
        let pipeline_layout = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gpu-rt"),
            bind_group_layouts: &[Some(&layout0), Some(world.layout())],
            immediate_size: 0,
        });
        let pipeline = pipeline(&gpu, &pipeline_layout, source(&shading));
        let buf = |label, size: u64, usage| gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false });
        let camera_buf = buf("camera", size_of::<CameraUniform>() as u64, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
        let geo_buf = GrowableBuffer::new(&gpu, "geometry", 1 << 20, 1 << 30, wgpu::BufferUsages::STORAGE);
        let geo_words = (geo_buf.size() / 4) as u32;
        let chunk_info = buf("chunk info", MAX_SLOTS as u64 * 16, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        let depth = opts.frames_in_flight.max(1);
        let tlas_cap = 4096;
        tracing::info!("gpu-rt: geometry {}, shading {shading}", opts.geometry);
        Ok(Self {
            output: mr_gpu::output_texture(&gpu, p.width, p.height),
            ring: FrameRing::new(&gpu, p.width, p.height, depth, waker.clone()),
            tlas: tlas(&gpu, tlas_cap),
            tlas_cap,
            accum: ShadeState::new(&gpu, p.width, p.height, &shading, opts.bounces, opts.spp, opts.accumulate),
            geo_buf,
            world,
            rt,
            knobs: Knobs { merge: Merge::Block, cull: opts.cull },
            built_shading: shading.clone(),
            shading,
            opts,
            source,
            pipeline,
            pipeline_layout,
            layout0,
            camera_buf,
            waker,
            width: p.width,
            height: p.height,
            geo_alloc: RangeAlloc::new(geo_words),
            chunk_info,
            chunks: HashMap::new(),
            free_slots: (0..MAX_SLOTS).rev().collect(),
            dirty: HashSet::new(),
            bind0: None,
            gpu,
        })
    }

    fn drop_chunk(&mut self, pos: IVec3) {
        if let Some(c) = self.chunks.remove(&pos) {
            self.geo_alloc.free(c.range.0, c.range.1);
            self.free_slots.push(c.slot);
        }
    }

    fn build_geometry(&self, pos: IVec3) -> Option<Built> {
        match self.rt {
            RtKind::Triangles(mesh) => {
                let center = self.world.chunk(pos)?;
                let neighbors = Face::ALL.map(|f| self.world.chunk(pos + f.normal()));
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
            RtKind::Boxes => match self.world.chunk(pos)? {
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
    fn rebuild(&mut self, enc: &mut wgpu::CommandEncoder, near: IVec3) -> usize {
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
        let built: Vec<(IVec3, Option<Built>)> = dirty.par_iter().map(|&p| (p, self.build_geometry(p))).collect();
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
                    (sizes, vec![ab], bricks.clone(), Some(self.world.table_lookup(pos).unwrap_or(0xffff_ffff)))
                }
            };
            let off = match self.geo_buf.alloc(&self.gpu, &mut self.geo_alloc, words.len() as u32, 4) {
                Ok((off, grew)) => {
                    if grew {
                        self.bind0 = None;
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

impl Renderer for GpuRt {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        if !updates.is_empty() {
            self.accum.invalidate();
        }
        for pos in self.world.apply(&self.gpu, updates) {
            self.dirty.insert(pos);
            if let RtKind::Triangles(_) = self.rt {
                // Face culling depends on neighbours.
                for f in Face::ALL {
                    let nb = pos + f.normal();
                    if self.chunks.contains_key(&nb) || self.world.chunk(nb).is_some() {
                        self.dirty.insert(nb);
                    }
                }
            }
        }
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        if self.shading != self.built_shading {
            self.pipeline = pipeline(&self.gpu, &self.pipeline_layout, (self.source)(&self.shading));
            self.accum.set_shading(&self.gpu, &self.shading);
            self.built_shading = self.shading.clone();
            self.bind0 = None;
        }
        let ticket = self.ring.begin(&self.gpu, sink)?;
        let start = Instant::now();
        let c = &req.camera;
        let (cam_block, frac) = c.split_position();
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        // GPU time should include the acceleration-structure builds, which happen outside
        // any pass: start the clock in the encoder when the device allows it.
        let encoder_ts = self.gpu.device.features().contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS);
        if let (true, Some((qs, begin, _))) = (encoder_ts, self.ring.timestamps(ticket)) {
            enc.write_timestamp(qs, begin);
        }
        let rebuilt = self.rebuild(&mut enc, cam_block >> 5);
        let build_ms = start.elapsed().as_secs_f64() * 1e3;

        // TLAS: one instance per chunk, translated relative to the camera block.
        let n = self.chunks.len() as u32;
        if n > self.tlas_cap {
            self.tlas_cap = n.next_power_of_two();
            self.tlas = tlas(&self.gpu, self.tlas_cap);
            self.bind0 = None;
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

        let cull = match self.opts.cull_faces {
            Cull::Back => 0x10,
            Cull::Front => 0x20,
            Cull::None => 0,
        };
        self.accum.begin(&self.gpu, c, req.mode);
        let cu = CameraUniform {
            forward: c.forward().to_array(),
            tan_half_fov: (c.fov_y * 0.5).tan(),
            right: c.right().to_array(),
            aspect: self.width as f32 / self.height as f32,
            up: c.up().to_array(),
            mode: (req.mode == RenderMode::Lit) as u32,
            origin: frac.to_array(),
            sky: u32::from_le_bytes(encode_srgb(self.world.env.sky)),
            width: self.width,
            height: self.height,
            ray_flags: if let RtKind::Triangles(_) = self.rt { cull } else { 0 },
            _p0: 0,
        };
        self.gpu.queue.write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&cu));
        let world_bg = self.world.prepare(&self.gpu, c).clone();
        if self.bind0.is_none() {
            let view = self.output.create_view(&Default::default());
            self.bind0 = Some(self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("gpu-rt"),
                layout: &self.layout0,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.camera_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::AccelerationStructure(&self.tlas) },
                    wgpu::BindGroupEntry { binding: 3, resource: self.geo_buf.buffer().as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 4, resource: self.chunk_info.as_entire_binding() },
                    self.accum.bind_entries(5)[0].clone(),
                    self.accum.bind_entries(5)[1].clone(),
                ],
            }));
        }
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("gpu-rt"), timestamp_writes: self.ring.compute_timestamps(ticket, !encoder_ts, true) });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, self.bind0.as_ref().unwrap(), &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
        }
        let mut counters = vec![("instances".into(), n as f64), ("blas_rebuilt".into(), rebuilt as f64), ("build_ms".into(), build_ms)];
        counters.extend(self.accum.counter(req.mode));
        let stats = FrameStats { cpu_ms: start.elapsed().as_secs_f32() * 1e3, gpu_ms: None, counters, legend: None };
        self.ring.submit(&self.gpu, enc, ticket, &self.output, req.frame_id, req.readback, stats);
        self.ring.poll(&self.gpu, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        match name {
            "shadows" => self.world.cfg.shadows.set(value)?,
            "shading" => self.shading = require(BACKEND, Slot::Shading, value.as_str().unwrap_or(""))?,
            "fast_build" => self.opts.fast_build = value.as_bool().unwrap_or(false),
            "build_budget" => self.opts.build_budget = value.as_i64().unwrap_or(0) as usize,
            "cull_faces" => {
                self.opts.cull_faces = match value.as_str() {
                    Some("front") => Cull::Front,
                    Some("none") => Cull::None,
                    _ => Cull::Back,
                }
            }
            _ if self.accum.set_param(name, value).is_some() => {}
            _ => anyhow::bail!("`{name}` cannot change while running"),
        }
        Ok(())
    }

    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, false)
    }

    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, true)
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.width = width;
        self.height = height;
        self.output = mr_gpu::output_texture(&self.gpu, width, height);
        self.ring = FrameRing::new(&self.gpu, width, height, self.opts.frames_in_flight.max(1), self.waker.clone());
        self.accum.resize(&self.gpu, width, height);
        self.bind0 = None;
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        // BLAS sizes are driver-internal; BLAS input buffers are a lower bound.
        let inputs: u64 = self.chunks.values().flat_map(|c| c._inputs.iter()).map(|b| b.size()).sum();
        let gpu = self.world.memory_bytes() + self.geo_buf.size() + self.chunk_info.size() + self.accum.memory_bytes() + inputs;
        MemoryReport { cpu_bytes: 0, gpu_bytes: gpu }
    }
}

impl Drop for GpuRt {
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
    /// queries) and validates for every geometry and shading (no GPU needed).
    #[test]
    fn programs_compose() {
        use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        for g in names_on(BACKEND, Slot::Geometry) {
            let (geo_wgsl, _) = mr_components::geometry::rt(g).unwrap();
            for shading in names_on(BACKEND, Slot::Shading) {
                let (p, root) = program(g, "rayquery", geo_wgsl, traversal::rt_wgsl("rayquery").unwrap(), shading);
                let m = p.naga(&root, ROOT_DEPS).unwrap_or_else(|e| panic!("{g} {shading}: {e}"));
                Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("{g} {shading}: {e:?}"));
            }
        }
    }
}
