//! GPU-raster back end: assembles a rasterising renderer from components (`mr-components`,
//! IDEAS §12):
//!
//! * **geometry** turns each chunk into packed face instances on the CPU (parallel,
//!   incremental; `greedy`, `naive`, `cubes`), drawn with vertex pulling and one
//!   multi-draw-indirect call after per-chunk frustum and per-face-direction culling;
//! * **visibility** decides how the front face of each pixel is found and shaded:
//!   `raster` (forward, with an optional depth pre-pass in lit mode), `gbuffer` or `visbuffer` (ids,
//!   then one compute shading pass);
//! * **shading** colours the hit (`shade_hit`, the same component as on the ray-walk back
//!   ends), with lit-mode shadow rays walking the composed **storage** (refloor).
//!
//! Replaces the monoliths gpu-raster, gpu-raster-naive, gpu-cubes and gpu-visbuffer.
//! Meshes (mesh mode) share the depth buffer (`mr_gpu::meshes::GpuMeshes`).

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Mat4, Vec3, Vec4};
use mr_components::geometry::{GpuGeometry, Knobs, Merge};
use mr_components::storage::gpu_data::{self, StorageData};
use mr_components::{Slot, components, storage, visibility};
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::meshes::{GpuMeshes, MeshPass, Occlusion};
use mr_gpu::voxels::{Shadows, VoxelWorld, VoxelWorldConfig, shader_source, shadows_param};
use mr_gpu::{FrameRing, Gpu, GpuRequest, GrowableBuffer, RangeAlloc, wgpu};
use rayon::prelude::*;
use serde::Deserialize;

const BACKEND: &str = "gpu-raster";

pub fn entry() -> RendererEntry {
    RendererEntry { info, create: |p, w| Ok(Box::new(GpuRasterBackend::new(p, w)?)) }
}

fn info() -> RendererInfo {
    RendererInfo {
        name: BACKEND.into(),
        description: "GPU-raster back end: renders a composition (storage for shadow rays, geometry, raster visibility, shading); faces meshed on the CPU, vertex pulling, multi-draw-indirect, chunk + face culling".into(),
        tags: vec!["gpu".into(), "raster".into(), "hybrid".into(), "modular".into(), "wgpu".into(), "meshes".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: params(),
    }
}

fn names(slot: Slot) -> Vec<&'static str> {
    components(slot).filter(|c| c.backends.contains(&BACKEND)).map(|c| c.name).collect()
}

fn describe(slot: Slot) -> String {
    components(slot).filter(|c| c.backends.contains(&BACKEND)).map(|c| format!("{}: {}", c.name, c.description)).collect::<Vec<_>>().join(" ")
}

fn params() -> Vec<ParamSpec> {
    vec![
        ParamSpec::choice("storage", &format!("Storage component the lit-mode shadow rays walk (refloor). {}", describe(Slot::Storage)), &names(Slot::Storage), "bricks", false),
        ParamSpec::choice("geometry", &format!("Geometry component. {}", describe(Slot::Geometry)), &names(Slot::Geometry), "greedy", true),
        ParamSpec::choice("visibility", &format!("Visibility component. {}", describe(Slot::Visibility)), &names(Slot::Visibility), "raster", true),
        ParamSpec::choice("shading", &format!("Shading component. {}", describe(Slot::Shading)), &names(Slot::Shading), "direct", true),
        ParamSpec::choice("merge", "greedy: merge faces of the same block only (`block`), or any exposed coplanar faces with a per-pixel block lookup (`exposure`)", &["block", "exposure"], "block", true),
        ParamSpec::bool("cull", "naive / cubes: skip faces (blocks) hidden by solid neighbours (false = every face of every solid block)", true, true),
        ParamSpec::bool("depth_prepass", "raster, lit mode: depth pre-pass so lighting runs once per pixel", true, true),
        shadows_param(),
        ParamSpec::int("max_steps", "Shadow-ray step limit", 32, 16384, 2048, true),
        ParamSpec::int("mesh_budget", "Chunks remeshed per frame, nearest first (0 = all)", 0, 100000, 0, true),
        ParamSpec::int("pool_mb", "Voxel pool limit for shadow rays (MiB); for region storages (tree64) the limit of their own data", 64, 6144, 1536, false),
        ParamSpec::int("quad_buffer_mb", "Instance buffer limit (MiB; grows on demand)", 16, 2040, 1024, false),
        ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
    ]
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    adapter: Option<String>,
    storage: String,
    geometry: String,
    visibility: String,
    shading: String,
    merge: String,
    cull: bool,
    /// raster, lit mode: lay down depth first so the expensive fragment shader runs once
    /// per pixel.
    depth_prepass: bool,
    shadows: Shadows,
    max_steps: u32,
    mesh_budget: usize,
    /// Voxel pool for lighting rays (unused in unlit mode but always allocated).
    pool_mb: u32,
    quad_buffer_mb: u32,
    frames_in_flight: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            adapter: None,
            storage: "bricks".into(),
            geometry: "greedy".into(),
            visibility: "raster".into(),
            shading: "direct".into(),
            merge: "block".into(),
            cull: true,
            depth_prepass: true,
            shadows: Shadows::All,
            max_steps: 2048,
            mesh_budget: 0,
            pool_mb: 1536,
            quad_buffer_mb: 1024,
            frames_in_flight: 3,
        }
    }
}

/// Checks that `name` is a `slot` component with a gpu-raster implementation.
fn check(slot: Slot, name: &str) -> anyhow::Result<String> {
    match mr_components::unavailable(BACKEND, slot, name) {
        None => Ok(name.to_string()),
        Some(why) => anyhow::bail!(why),
    }
}

fn parse_merge(s: &str) -> anyhow::Result<Merge> {
    match s {
        "block" => Ok(Merge::Block),
        "exposure" => Ok(Merge::Exposure),
        _ => anyhow::bail!("merge must be `block` or `exposure`, not `{s}`"),
    }
}

/// The compile-time part of a composition: everything baked into the shaders.
#[derive(Clone, PartialEq, Debug)]
struct Composition {
    storage: String,
    geometry: String,
    visibility: String,
    shading: String,
}

impl Composition {
    fn storage(&self) -> storage::GpuStorage {
        storage::gpu(&self.storage).expect("validated")
    }

    fn geometry(&self) -> GpuGeometry {
        mr_components::geometry::gpu(&self.geometry).expect("validated")
    }

    /// Deferred visibility (ids or a G-buffer, then a compute shading pass).
    fn deferred(&self) -> bool {
        self.visibility == "visbuffer" || self.gbuffer()
    }

    fn gbuffer(&self) -> bool {
        self.visibility == "gbuffer"
    }

    fn source(&self) -> String {
        let st = self.storage();
        let own = format!(
            "// Composition: storage = {}, geometry = {}, visibility = {}, shading = {}\n\
             const LOD: bool = false;\n\
             {}\n{}\n{}\n{}\n{}\n{}",
            self.storage,
            self.geometry,
            self.visibility,
            self.shading,
            st.source(),
            visibility::gpu_wgsl("refloor").unwrap(),
            self.geometry().wgsl,
            include_str!("main.wgsl"),
            visibility::raster_wgsl(&self.visibility).expect("validated"),
            mr_components::shading::gpu_wgsl(&self.shading).expect("validated"),
        );
        let occlusion = "fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool { return ctrace(p, d, max_t).hit; }\n";
        shader_source(occlusion, &own)
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
    _p0: u32,
    size: [f32; 2],
    _p1: [f32; 2],
    sky: [f32; 4],
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

struct ChunkMesh {
    slot: u16,
    offset: u32,
    counts: [u32; 6],
}

impl ChunkMesh {
    fn len(&self) -> u32 {
        self.counts.iter().sum()
    }
}

/// The pipelines of one composition.
enum Pipelines {
    Raster { fill: wgpu::RenderPipeline, depth: wgpu::RenderPipeline, equal: wgpu::RenderPipeline },
    /// visbuffer and gbuffer: the raster pass writes `ids` (and with `gbuffer` the hit point
    /// first), then `shade` runs once per pixel.
    Deferred { ids: wgpu::RenderPipeline, shade: wgpu::ComputePipeline },
}

struct Layouts {
    frame: wgpu::BindGroupLayout,
    aux: wgpu::BindGroupLayout,
    images: wgpu::BindGroupLayout,
    render: wgpu::PipelineLayout,
    shade: wgpu::PipelineLayout,
}

pub struct GpuRasterBackend {
    gpu: Gpu,
    world: VoxelWorld,
    opts: Options,
    comp: Composition,
    /// Composition the pipelines were built from (rebuilt lazily when knobs change it).
    built: Composition,
    pipelines: Pipelines,
    layouts: Layouts,
    /// Mesh mode: drawn into the same depth buffer.
    mesh: GpuMeshes,
    frame_buf: wgpu::Buffer,
    quad_buf: GrowableBuffer,
    chunk_buf: wgpu::Buffer,
    indirect: wgpu::Buffer,
    bind0: Option<wgpu::BindGroup>,
    /// The storage's own data for the shadow walk (group 2).
    data: StorageData,
    output: wgpu::Texture,
    depth: wgpu::TextureView,
    /// visbuffer: face ids; gbuffer: block ids.
    ids: wgpu::TextureView,
    /// gbuffer: hit point and face (1x1 otherwise).
    gpos: wgpu::TextureView,
    ring: FrameRing,
    meshes: HashMap<IVec3, ChunkMesh>,
    dirty: HashSet<IVec3>,
    alloc: RangeAlloc,
    free_slots: Vec<u16>,
    width: u32,
    height: u32,
    waker: Waker,
    overflowed: bool,
}

fn target(gpu: &Gpu, label: &str, w: u32, h: u32, format: wgpu::TextureFormat, usage: wgpu::TextureUsages) -> wgpu::TextureView {
    gpu.device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

fn depth_view(gpu: &Gpu, w: u32, h: u32) -> wgpu::TextureView {
    target(gpu, "depth", w, h, wgpu::TextureFormat::Depth32Float, wgpu::TextureUsages::RENDER_ATTACHMENT)
}

fn ids_view(gpu: &Gpu, w: u32, h: u32) -> wgpu::TextureView {
    target(gpu, "visibility ids", w, h, wgpu::TextureFormat::R32Uint, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
}

/// The G-buffer's hit points, full size only while `gbuffer` is chosen.
fn gpos_view(gpu: &Gpu, w: u32, h: u32, used: bool) -> wgpu::TextureView {
    let (w, h) = if used { (w, h) } else { (1, 1) };
    target(gpu, "gbuffer points", w, h, wgpu::TextureFormat::Rgba32Float, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
}

impl Layouts {
    fn new(gpu: &Gpu, world: &VoxelWorld, st: &storage::GpuStorage) -> Self {
        let all = wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE;
        let buf = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: all,
            ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let ro = wgpu::BufferBindingType::Storage { read_only: true };
        let frame = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("raster frame"),
            entries: &[buf(0, wgpu::BufferBindingType::Uniform), buf(1, ro), buf(2, ro)],
        });
        let aux = gpu_data::layout(gpu, st, all);
        let images = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("deferred images"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Uint, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let render = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("raster"),
            bind_group_layouts: &[Some(&frame), Some(world.layout()), Some(&aux)],
            immediate_size: 0,
        });
        let shade = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("deferred shade"),
            bind_group_layouts: &[Some(&frame), Some(world.layout()), Some(&aux), Some(&images)],
            immediate_size: 0,
        });
        Self { frame, aux, images, render, shade }
    }
}

impl Pipelines {
    fn new(gpu: &Gpu, layouts: &Layouts, comp: &Composition) -> anyhow::Result<Self> {
        let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("raster"), source: wgpu::ShaderSource::Wgsl(comp.source().into()) });
        let geo = comp.geometry();
        let pipeline = |label, format, write: bool, compare, fragment: bool| {
            let targets = [Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })];
            gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layouts.render),
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
                primitive: wgpu::PrimitiveState {
                    topology: if geo.strip { wgpu::PrimitiveTopology::TriangleStrip } else { wgpu::PrimitiveTopology::TriangleList },
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
                fragment: fragment.then(|| wgpu::FragmentState { module: &module, entry_point: Some("fs"), compilation_options: Default::default(), targets: &targets }),
                multiview_mask: None,
                cache: None,
            })
        };
        use wgpu::CompareFunction::{Equal, Greater};
        use wgpu::TextureFormat::{R32Uint, Rgba8Unorm};
        let p = if comp.deferred() {
            let ids = if comp.gbuffer() {
                let targets = [Some(wgpu::TextureFormat::Rgba32Float.into()), Some(R32Uint.into())];
                gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("gbuffer"),
                    layout: Some(&layouts.render),
                    vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
                    primitive: wgpu::PrimitiveState {
                        topology: if geo.strip { wgpu::PrimitiveTopology::TriangleStrip } else { wgpu::PrimitiveTopology::TriangleList },
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: Some(wgpu::Face::Back),
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: wgpu::TextureFormat::Depth32Float,
                        depth_write_enabled: Some(true),
                        depth_compare: Some(Greater),
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    multisample: Default::default(),
                    fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs"), compilation_options: Default::default(), targets: &targets }),
                    multiview_mask: None,
                    cache: None,
                })
            } else {
                pipeline("visibility ids", R32Uint, true, Greater, true)
            };
            Pipelines::Deferred {
                ids,
                shade: gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("deferred shade"),
                    layout: Some(&layouts.shade),
                    module: &module,
                    entry_point: Some("shade"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
            }
        } else {
            Pipelines::Raster {
                fill: pipeline("fill", Rgba8Unorm, true, Greater, true),
                depth: pipeline("depth prepass", Rgba8Unorm, true, Greater, false),
                equal: pipeline("shade equal", Rgba8Unorm, false, Equal, true),
            }
        };
        if let Some(e) = pollster::block_on(scope.pop()) {
            anyhow::bail!("compiling {comp:?}: {e}");
        }
        Ok(p)
    }
}

impl GpuRasterBackend {
    pub fn new(p: InitParams, waker: Waker) -> anyhow::Result<Self> {
        let opts: Options = p.options()?;
        parse_merge(&opts.merge)?;
        let comp = Composition {
            storage: check(Slot::Storage, &opts.storage)?,
            geometry: check(Slot::Geometry, &opts.geometry)?,
            visibility: check(Slot::Visibility, &opts.visibility)?,
            shading: check(Slot::Shading, &opts.shading)?,
        };
        let gpu = Gpu::new(GpuRequest {
            experimental: false,
            adapter: opts.adapter.as_deref(),
            required_features: wgpu::Features::INDIRECT_FIRST_INSTANCE,
            optional_features: wgpu::Features::TIMESTAMP_QUERY,
            limits: &|a| a.clone(),
        })?;
        let world = VoxelWorld::new(
            &gpu,
            &p.blocks,
            p.environment,
            VoxelWorldConfig {
                max_lod: 0,
                // Region storages (tree64) hold their own data instead of the voxel pool.
                upload_voxels: matches!(comp.storage().kind, storage::Kind::Pool { .. }),
                pool_mb: opts.pool_mb,
                view_radius: p.view_radius_chunks,
                shadows: opts.shadows,
                max_steps: opts.max_steps,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
            },
        )?;
        let layouts = Layouts::new(&gpu, &world, &comp.storage());
        let pipelines = Pipelines::new(&gpu, &layouts, &comp)?;
        let max_bytes = (gpu.device.limits().max_storage_buffer_binding_size as u64).min(gpu.device.limits().max_buffer_size);
        // Capped below 2 GiB: GrowableBuffer rounds sizes up to powers of two, which would
        // overshoot a 2^31-4 storage-binding limit.
        let quad_capacity = ((opts.quad_buffer_mb.min(2040) as u64 * 1024 * 1024).min(max_bytes) / QUAD_BYTES) as u32;
        let storage = |label, size: u64, extra| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | extra,
                mapped_at_creation: false,
            })
        };
        let frame_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame"),
            size: size_of::<FrameUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let quad_buf = GrowableBuffer::new(&gpu, "quads", 1 << 20, quad_capacity as u64 * QUAD_BYTES, wgpu::BufferUsages::STORAGE);
        let chunk_buf = storage("chunk positions", MAX_SLOTS as u64 * 16, wgpu::BufferUsages::empty());
        let indirect = storage("indirect", 6 * 16 * 4096, wgpu::BufferUsages::INDIRECT);
        let data = StorageData::new(&gpu, comp.storage(), gpu_data::Limits { mb: opts.pool_mb, view_radius: p.view_radius_chunks, wide_ids: p.blocks.blocks.len() > 256 });
        // Mesh shadows walk the composed storage too.
        let occ = visibility::occlusion_wgsl(&comp.storage(), false);
        let mesh = GpuMeshes::new(&gpu, &world, wgpu::TextureFormat::Rgba8Unorm, Some(Occlusion { wgsl: &occ, layout: &layouts.aux }));
        let ring = FrameRing::new(&gpu, p.width, p.height, opts.frames_in_flight.max(1), waker.clone());
        tracing::info!("gpu-raster: storage {}, geometry {}, visibility {}, shading {}", comp.storage, comp.geometry, comp.visibility, comp.shading);
        Ok(Self {
            mesh,
            output: mr_gpu::output_texture(&gpu, p.width, p.height),
            depth: depth_view(&gpu, p.width, p.height),
            ids: ids_view(&gpu, p.width, p.height),
            gpos: gpos_view(&gpu, p.width, p.height, comp.gbuffer()),
            ring,
            world,
            opts,
            built: comp.clone(),
            comp,
            pipelines,
            layouts,
            frame_buf,
            quad_buf,
            chunk_buf,
            indirect,
            bind0: None,
            data,
            meshes: HashMap::new(),
            dirty: HashSet::new(),
            alloc: RangeAlloc::new(((1 << 20) / QUAD_BYTES) as u32),
            free_slots: (0..MAX_SLOTS as u32).rev().map(|s| s as u16).collect(),
            width: p.width,
            height: p.height,
            waker,
            overflowed: false,
            gpu,
        })
    }

    fn knobs(&self) -> Knobs {
        Knobs { merge: parse_merge(&self.opts.merge).unwrap_or_default(), cull: self.opts.cull }
    }

    fn remesh_all(&mut self) {
        self.dirty.extend(self.meshes.keys().copied());
    }

    fn drop_mesh(&mut self, pos: IVec3) {
        if let Some(m) = self.meshes.remove(&pos) {
            self.alloc.free(m.offset, m.len());
            self.free_slots.push(m.slot);
        }
    }

    /// Remeshes dirty chunks in parallel, nearest to `near` first, at most `mesh_budget` per
    /// call, and uploads the results.
    fn remesh(&mut self, near: IVec3) -> usize {
        if self.dirty.is_empty() {
            return 0;
        }
        let mut dirty: Vec<IVec3> = self.dirty.drain().collect();
        if self.opts.mesh_budget > 0 && dirty.len() > self.opts.mesh_budget {
            dirty.sort_unstable_by_key(|p| (*p - near).length_squared());
            for p in dirty.split_off(self.opts.mesh_budget) {
                self.dirty.insert(p);
            }
        }
        let mut work = vec![];
        for pos in dirty {
            if self.world.chunk(pos).is_none() {
                self.drop_mesh(pos);
                continue;
            }
            let slot = match self.meshes.get(&pos) {
                Some(m) => m.slot,
                None => match self.free_slots.pop() {
                    Some(s) => {
                        self.meshes.insert(pos, ChunkMesh { slot: s, offset: 0, counts: [0; 6] });
                        self.gpu.queue.write_buffer(&self.chunk_buf, s as u64 * 16, bytemuck::cast_slice(&[pos.x, pos.y, pos.z, 0]));
                        s
                    }
                    None => continue,
                },
            };
            work.push((pos, slot));
        }
        let world = &self.world;
        let (mesh_fn, knobs) = (self.comp.geometry().mesh, self.knobs());
        let results: Vec<(IVec3, mr_components::geometry::Mesh)> = work
            .par_iter()
            .map(|&(pos, slot)| {
                let center = world.chunk(pos).unwrap();
                let neighbors = Face::ALL.map(|f| world.chunk(pos + f.normal()));
                (pos, mesh_fn(center, neighbors, slot, knobs))
            })
            .collect();
        let n = results.len();
        for (pos, mesh) in results {
            let m = self.meshes.get_mut(&pos).unwrap();
            let old = (m.offset, m.len());
            m.counts = [0; 6];
            self.alloc.free(old.0, old.1);
            let len = mesh.quads.len() as u32;
            if len == 0 {
                continue;
            }
            let off = match self.quad_buf.alloc(&self.gpu, &mut self.alloc, len, QUAD_BYTES) {
                Ok((off, grew)) => {
                    if grew {
                        self.bind0 = None;
                    }
                    off
                }
                Err(e) => {
                    if !self.overflowed {
                        tracing::error!("{e:#}; raise `quad_buffer_mb`");
                        self.overflowed = true;
                    }
                    continue;
                }
            };
            let m = self.meshes.get_mut(&pos).unwrap();
            m.offset = off;
            m.counts = mesh.counts;
            self.gpu.queue.write_buffer(self.quad_buf.buffer(), off as u64 * QUAD_BYTES, bytemuck::cast_slice(&mesh.quads));
        }
        n
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

impl Renderer for GpuRasterBackend {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        let changed = self.world.apply(&self.gpu, updates);
        self.data.update(&self.gpu, &self.world, &changed)?;
        for pos in changed {
            self.dirty.insert(pos);
            for f in Face::ALL {
                let nb = pos + f.normal();
                if self.meshes.contains_key(&nb) || self.world.chunk(nb).is_some() {
                    self.dirty.insert(nb);
                }
            }
        }
        Ok(())
    }

    fn update_meshes(&mut self, update: MeshUpdate) -> anyhow::Result<()> {
        self.mesh.apply(update);
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        if self.comp != self.built {
            self.pipelines = Pipelines::new(&self.gpu, &self.layouts, &self.comp)?;
            if self.comp.geometry != self.built.geometry {
                self.remesh_all();
            }
            if self.comp.gbuffer() != self.built.gbuffer() {
                self.gpos = gpos_view(&self.gpu, self.width, self.height, self.comp.gbuffer());
            }
            self.built = self.comp.clone();
        }
        let ticket = self.ring.begin(&self.gpu, sink)?;
        let start = Instant::now();
        let remeshed = self.remesh(req.camera.split_position().0 >> 5);
        let mesh_ms = start.elapsed().as_secs_f64() * 1e3;
        let c = &req.camera;
        let (cam_block, frac) = c.split_position();
        let view_proj = c.projection(self.width as f32 / self.height as f32) * c.view_rotation();
        let sky = encode_srgb(self.world.env.sky);
        let fu = FrameUniform {
            view_proj: view_proj.to_cols_array_2d(),
            cam_frac: frac.to_array(),
            mode: (req.mode == RenderMode::Lit) as u32,
            forward: c.forward().to_array(),
            tan_half_fov: (c.fov_y * 0.5).tan(),
            right: c.right().to_array(),
            aspect: self.width as f32 / self.height as f32,
            up: c.up().to_array(),
            _p0: 0,
            size: [self.width as f32, self.height as f32],
            _p1: [0.0; 2],
            sky: [sky[0] as f32 / 255.0, sky[1] as f32 / 255.0, sky[2] as f32 / 255.0, 1.0],
        };
        self.gpu.queue.write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&fu));
        let world_bg = self.world.prepare(&self.gpu, c).clone();
        if req.meshes {
            self.mesh.prepare(&self.gpu, c, view_proj, req.mode);
        }

        // Cull chunks and face directions; build indirect draws.
        let planes = frustum_planes(view_proj);
        let geo = self.comp.geometry();
        let mut draws: Vec<DrawArgs> = Vec::with_capacity(self.meshes.len() * 3);
        let (mut drawn, mut total) = (0u64, 0u64);
        for (pos, m) in &self.meshes {
            total += m.len() as u64;
            if m.len() == 0 {
                continue;
            }
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
        let bytes = (draws.len() * 16) as u64;
        if bytes > self.indirect.size() {
            self.indirect = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("indirect"),
                size: bytes.next_power_of_two(),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::INDIRECT,
                mapped_at_creation: false,
            });
        }
        if !draws.is_empty() {
            self.gpu.queue.write_buffer(&self.indirect, 0, bytemuck::cast_slice(&draws));
        }

        if self.bind0.is_none() {
            self.bind0 = Some(self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("raster frame"),
                layout: &self.layouts.frame,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.frame_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: self.quad_buf.buffer().as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: self.chunk_buf.as_entire_binding() },
                ],
            }));
        }
        let bind2 = self.data.prepare(&self.gpu, &self.world, &self.layouts.aux).clone();
        self.mesh.set_occlusion_bind_group(&bind2);
        let bind0 = self.bind0.clone().unwrap();
        let clear = wgpu::Color { r: fu.sky[0] as f64, g: fu.sky[1] as f64, b: fu.sky[2] as f64, a: 1.0 };
        let out_view = self.output.create_view(&Default::default());
        let n = draws.len() as u32;
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        let depth_attachment = |load, store| {
            Some(wgpu::RenderPassDepthStencilAttachment { view: &self.depth, depth_ops: Some(wgpu::Operations { load, store }), stencil_ops: None })
        };
        let draw = |pass: &mut wgpu::RenderPass<'_>, pipeline: &wgpu::RenderPipeline| {
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.set_bind_group(2, &bind2, &[]);
            if n > 0 {
                pass.multi_draw_indirect(&self.indirect, 0, n);
            }
        };
        match &self.pipelines {
            Pipelines::Raster { fill, depth, equal } => {
                let prepass = self.opts.depth_prepass && req.mode == RenderMode::Lit;
                if prepass {
                    let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("depth prepass"),
                        color_attachments: &[],
                        depth_stencil_attachment: depth_attachment(wgpu::LoadOp::Clear(0.0), wgpu::StoreOp::Store),
                        timestamp_writes: self.ring.render_timestamps(ticket, true, false),
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    draw(&mut pass, depth);
                    if req.meshes {
                        self.mesh.draw(&mut pass, MeshPass::Depth, &world_bg);
                    }
                }
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("shade"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &out_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: depth_attachment(if prepass { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(0.0) }, wgpu::StoreOp::Discard),
                    timestamp_writes: self.ring.render_timestamps(ticket, !prepass, true),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                draw(&mut pass, if prepass { equal } else { fill });
                if req.meshes {
                    self.mesh.draw(&mut pass, if prepass { MeshPass::Equal } else { MeshPass::Fill }, &world_bg);
                }
            }
            Pipelines::Deferred { ids, shade } => {
                // Meshes are drawn after shading, depth-tested against the faces.
                let meshes = req.meshes && self.mesh.triangles() > 0;
                {
                    let attach = |view| {
                        Some(wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } })
                    };
                    let gbuffer = [attach(&self.gpos), attach(&self.ids)];
                    let idbuffer = [attach(&self.ids)];
                    let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("visibility ids"),
                        color_attachments: if self.comp.gbuffer() { &gbuffer } else { &idbuffer },
                        depth_stencil_attachment: depth_attachment(wgpu::LoadOp::Clear(0.0), if meshes { wgpu::StoreOp::Store } else { wgpu::StoreOp::Discard }),
                        timestamp_writes: self.ring.render_timestamps(ticket, true, false),
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    draw(&mut pass, ids);
                }
                let images = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("deferred images"),
                    layout: &self.layouts.images,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.ids) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&out_view) },
                        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.gpos) },
                    ],
                });
                {
                    let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("deferred shade"), timestamp_writes: if meshes { None } else { self.ring.compute_timestamps(ticket, false, true) } });
                    pass.set_pipeline(shade);
                    pass.set_bind_group(0, &bind0, &[]);
                    pass.set_bind_group(1, &world_bg, &[]);
                    pass.set_bind_group(2, &bind2, &[]);
                    pass.set_bind_group(3, &images, &[]);
                    pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
                }
                if meshes {
                    let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("meshes"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &out_view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                        })],
                        depth_stencil_attachment: depth_attachment(wgpu::LoadOp::Load, wgpu::StoreOp::Discard),
                        timestamp_writes: self.ring.render_timestamps(ticket, false, true),
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    self.mesh.draw(&mut pass, MeshPass::Fill, &world_bg);
                }
            }
        }
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            counters: vec![
                ("instances".into(), drawn as f64),
                ("instances_total".into(), total as f64),
                ("draws".into(), n as f64),
                ("remeshed".into(), remeshed as f64),
                ("mesh_ms".into(), mesh_ms),
                ("mesh_triangles".into(), if req.meshes { self.mesh.triangles() as f64 } else { 0.0 }),
            ]
            .into_iter()
            .chain(self.data.counters())
            .collect(),
        };
        self.ring.submit(&self.gpu, enc, ticket, &self.output, req.frame_id, req.readback, stats);
        self.ring.poll(&self.gpu, sink, false)
    }

    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        let s = || value.as_str().unwrap_or("").to_string();
        match name {
            "shadows" => self.world.cfg.shadows.set(value)?,
            "max_steps" => self.world.cfg.max_steps = value.as_i64().unwrap_or(2048) as u32,
            "depth_prepass" => self.opts.depth_prepass = value.as_bool().unwrap_or(true),
            "mesh_budget" => self.opts.mesh_budget = value.as_i64().unwrap_or(0) as usize,
            "geometry" => self.comp.geometry = check(Slot::Geometry, &s())?,
            "visibility" => self.comp.visibility = check(Slot::Visibility, &s())?,
            "shading" => self.comp.shading = check(Slot::Shading, &s())?,
            "merge" => {
                parse_merge(&s())?;
                if s() != self.opts.merge {
                    self.opts.merge = s();
                    self.remesh_all();
                }
            }
            "cull" => {
                let c = value.as_bool().unwrap_or(true);
                if c != self.opts.cull {
                    self.opts.cull = c;
                    self.remesh_all();
                }
            }
            _ => anyhow::bail!("`{name}` cannot change while running"),
        }
        Ok(())
    }

    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, true)
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.width = width;
        self.height = height;
        self.output = mr_gpu::output_texture(&self.gpu, width, height);
        self.depth = depth_view(&self.gpu, width, height);
        self.ids = ids_view(&self.gpu, width, height);
        self.gpos = gpos_view(&self.gpu, width, height, self.built.gbuffer());
        self.ring = FrameRing::new(&self.gpu, width, height, self.opts.frames_in_flight.max(1), self.waker.clone());
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        let gpu = self.world.memory_bytes() + self.data.memory_bytes() + self.quad_buf.size() + self.chunk_buf.size() + self.indirect.size() + self.width as u64 * self.height as u64 * 12;
        MemoryReport { cpu_bytes: 0, gpu_bytes: gpu }
    }
}
