//! GPU-raster back end: assembles a rasterising renderer from components (`mr-components`,
//! IDEAS §12):
//!
//! * **geometry** turns each chunk into packed face instances on the CPU (parallel,
//!   incremental; `greedy`, `naive`, `cubes`), drawn with vertex pulling and one
//!   multi-draw-indirect call after per-chunk frustum and per-face-direction culling;
//! * **visibility** `faces`: every face goes to the draw stage;
//! * **draw** decides how the front face of each pixel is found and shaded (the shared face
//!   drawer, `mr_components::draw`): `raster` (forward, with an optional depth pre-pass in
//!   lit mode), `gbuffer` or `visbuffer` (ids, then one compute shading pass);
//! * **shading** colours the hit (`shade_surface`, the same component as on the ray-walk back
//!   ends), with lit-mode shadow rays walking the composed **storage** with the
//!   **traversal** kernel.
//!
//! Replaces the monoliths gpu-raster, gpu-raster-naive, gpu-cubes and gpu-visbuffer.
//! Meshes (mesh mode) share the depth buffer (`mr_gpu::meshes::GpuMeshes`).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Mat4, Vec3, Vec4};
use mr_components::geometry::{GpuGeometry, Knobs, Merge};
use mr_components::storage::gpu_data::{self, StorageData};
use mr_components::shading::state::{self as shade_state, ShadeState};
use mr_components::draw::drawer::{DrawFrame, DrawMode, DrawerSpec, FaceDrawer};
use mr_components::{draw, names_on, require, Slot, components, storage};
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::meshes::{GpuMeshes, MeshPass, Occlusion};
use mr_gpu::voxels::{Shadows, VoxelWorld, VoxelWorldConfig, shadows_param};
use mr_gpu::{FrameRing, Gpu, GpuNeeds, GrowableBuffer, RangeAlloc, wgpu};
use rayon::prelude::*;
use serde::Deserialize;

const BACKEND: &str = "gpu-raster";

pub fn entry() -> RendererEntry {
    RendererEntry { info, create: |p, d, w| Ok(Box::new(GpuRasterBackend::new(p, d, w)?)) }
}

fn info() -> RendererInfo {
    RendererInfo {
        name: BACKEND.into(),
        description: "GPU-raster back end: renders a composition (storage and traversal kernel for shadow rays, geometry, draw, shading); faces meshed on the CPU, vertex pulling, multi-draw-indirect, chunk + face culling".into(),
        tags: vec!["gpu".into(), "raster".into(), "hybrid".into(), "modular".into(), "wgpu".into(), "meshes".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: mr_components::lighting_knobs(BACKEND, params()),
    }
}


fn describe(slot: Slot) -> String {
    components(slot).filter(|c| c.backends.contains(&BACKEND)).map(|c| format!("{}: {}", c.name, c.description)).collect::<Vec<_>>().join(" ")
}

fn params() -> Vec<ParamSpec> {
    let mut p = vec![
        ParamSpec::choice("storage", &format!("Storage component the lit-mode shadow rays walk (with the traversal kernel). {}", describe(Slot::Storage)), &names_on(BACKEND, Slot::Storage), "bricks", false),
        ParamSpec::choice("geometry", &format!("Geometry component. {}", describe(Slot::Geometry)), &names_on(BACKEND, Slot::Geometry), "greedy", true),
        ParamSpec::choice("visibility", &format!("Visibility strategy. {}", describe(Slot::Visibility)), &names_on(BACKEND, Slot::Visibility), "faces", false),
        ParamSpec::choice("draw", &format!("Draw component. {}", describe(Slot::Draw)), &names_on(BACKEND, Slot::Draw), "raster", true),
        ParamSpec::choice("traversal", &format!("Traversal kernel of the lit-mode shadow rays. {}", describe(Slot::Traversal)), &names_on(BACKEND, Slot::Traversal), "refloor", false),
        ParamSpec::choice("shading", &format!("Shading component. {}", describe(Slot::Shading)), &names_on(BACKEND, Slot::Shading), "direct", true),
        ParamSpec::choice("meshes", &format!("Meshes component (mesh mode). {}", describe(Slot::Meshes)), &names_on(BACKEND, Slot::Meshes), "depth", true),
        ParamSpec::choice("merge", "greedy: merge faces of the same block only (`block`), or any exposed coplanar faces with a per-pixel block lookup (`exposure`)", &["block", "exposure"], "block", true).of("geometry", &["greedy"]),
        ParamSpec::bool("cull", "naive / cubes: skip faces (blocks) hidden by solid neighbours (false = every face of every solid block)", true, true).of("geometry", &["naive", "cubes"]),
        ParamSpec::bool("depth_prepass", "raster, lit mode: depth pre-pass so lighting runs once per pixel", true, true).of("draw", &["raster"]),
        shadows_param(),
        ParamSpec::int("max_steps", "Shadow-ray step limit", 32, 16384, 2048, true),
        ParamSpec::int("mesh_budget", "Chunks remeshed per frame, nearest first (0 = all)", 0, 100000, 0, true),
        ParamSpec::int("pool_mb", "Voxel pool limit for shadow rays (MiB); for region storages (tree64) the limit of their own data", 64, 6144, 1536, false),
        ParamSpec::int("quad_buffer_mb", "Instance buffer limit (MiB; grows on demand)", 16, 2040, 1024, false),
        ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
    ];
    p.extend(shade_state::params());
    p
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    storage: String,
    geometry: String,
    visibility: String,
    draw: String,
    traversal: String,
    shading: String,
    meshes: String,
    merge: String,
    cull: bool,
    /// raster, lit mode: lay down depth first so the expensive fragment shader runs once
    /// per pixel.
    depth_prepass: bool,
    bounces: u32,
    spp: u32,
    accumulate: bool,
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
            storage: "bricks".into(),
            geometry: "greedy".into(),
            visibility: "faces".into(),
            draw: "raster".into(),
            traversal: "refloor".into(),
            shading: "direct".into(),
            meshes: "depth".into(),
            merge: "block".into(),
            cull: true,
            depth_prepass: true,
            bounces: 1,
            spp: 1,
            accumulate: true,
            shadows: Shadows::All,
            max_steps: 2048,
            mesh_budget: 0,
            pool_mb: 1536,
            quad_buffer_mb: 1024,
            frames_in_flight: 3,
        }
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
    /// The draw component (the visibility strategy is always `faces`).
    draw: String,
    /// The shadow rays' kernel.
    traversal: String,
    shading: String,
}

impl Composition {
    fn storage(&self) -> storage::GpuStorage {
        storage::gpu(&self.storage).expect("validated")
    }

    fn geometry(&self) -> GpuGeometry {
        mr_components::geometry::gpu(&self.geometry).expect("validated")
    }

    /// Deferred drawing (ids or a G-buffer, then a compute shading pass).
    fn deferred(&self) -> bool {
        self.draw == "visbuffer" || self.gbuffer()
    }

    fn gbuffer(&self) -> bool {
        self.draw == "gbuffer"
    }

    /// The shader: the shadow rays' walk as modules (`walk::`, the composed storage at group
    /// 2 and the traversal kernel), this back end's code with geometry, draw and shading as
    /// the root.
    fn program(&self) -> (mr_gpu::program::Program, String, Vec<String>) {
        let (p, walk) = mr_components::shaders::walk_program(&self.storage(), &self.traversal, false);
        let own = format!(
            "// Composition: storage = {}, geometry = {}, draw = {}, traversal = {}, shading = {}\n\
             {}\n{}\n{}\n{}\n{}\n{}",
            self.storage,
            self.geometry,
            self.draw,
            self.traversal,
            self.shading,
            self.geometry().wgsl,
            include_str!("main.wgsl"),
            ShadeState::wgsl(0, 3),
            draw::faces_wgsl(false),
            // Deferred drawing shades in compute; forward raster shades its fragments.
            if self.deferred() {
                draw::gpu_wgsl(&self.draw).expect("validated").to_string()
            } else {
                shade_state::fragment_wgsl(&self.shading, draw::gpu_wgsl(&self.draw).expect("validated"))
            },
            mr_components::shading::gpu_wgsl(&self.shading).expect("validated"),
        );
        let deps = mr_components::shaders::root_deps(&walk, &[]).into_iter().map(String::from).collect();
        (p, own, deps)
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

struct Layouts {
    frame: wgpu::BindGroupLayout,
    aux: wgpu::BindGroupLayout,
    images: wgpu::BindGroupLayout,
    render: wgpu::PipelineLayout,
    shade: wgpu::PipelineLayout,
}

pub struct GpuRasterBackend {
    gpu: Arc<Gpu>,
    world: VoxelWorld,
    opts: Options,
    comp: Composition,
    /// Composition the pipelines were built from (rebuilt lazily when knobs change it).
    built: Composition,
    /// The face drawer with the draw component's pipelines.
    drawer: FaceDrawer,
    layouts: Layouts,
    /// Mesh mode: drawn into the same depth buffer.
    mesh: GpuMeshes,
    frame_buf: wgpu::Buffer,
    quad_buf: GrowableBuffer,
    chunk_buf: wgpu::Buffer,
    indirect: wgpu::Buffer,
    bind0: Option<wgpu::BindGroup>,
    /// Shading state (pathtrace's accumulation).
    accum: ShadeState,
    /// The storage's own data for the shadow walk (group 2).
    data: StorageData,
    output: wgpu::Texture,
    depth: wgpu::TextureView,
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
            entries: &[
                buf(0, wgpu::BufferBindingType::Uniform),
                buf(1, ro),
                buf(2, ro),
                // Shading state: shading runs in fragment (raster) or compute (deferred) passes.
                ShadeState::layout_entries(3, wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE)[0],
                ShadeState::layout_entries(3, wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE)[1],
            ],
        });
        let aux = gpu_data::layout(gpu, st, all);
        let images = FaceDrawer::images_layout(&gpu.device);
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

impl Layouts {
    /// The face drawer for `comp` (its shader module and the draw component).
    fn drawer(&self, gpu: &Gpu, comp: &Composition, width: u32, height: u32) -> anyhow::Result<FaceDrawer> {
        if !comp.deferred() {
            if let Some(why) = shade_state::fragment_blocker(&comp.shading, &gpu.device) {
                anyhow::bail!(why);
            }
        }
        let (program, root, deps) = comp.program();
        let module = mr_gpu::program::compile(&gpu.device, "raster", &program, &root, &deps.iter().map(String::as_str).collect::<Vec<_>>());
        let spec = DrawerSpec { module: &module, render_layout: &self.render, shade_layout: &self.shade, strip: comp.geometry().strip };
        FaceDrawer::new(gpu, &spec, DrawMode::parse(&comp.draw).expect("validated"), &self.images, width, height)
    }
}

impl GpuRasterBackend {
    pub fn new(p: InitParams, devices: &Devices, waker: Waker) -> anyhow::Result<Self> {
        let opts: Options = p.options()?;
        require(BACKEND, Slot::Meshes, &opts.meshes)?;
        parse_merge(&opts.merge)?;
        let comp = Composition {
            storage: require(BACKEND, Slot::Storage, &opts.storage)?,
            geometry: require(BACKEND, Slot::Geometry, &opts.geometry)?,
            draw: {
                require(BACKEND, Slot::Visibility, &opts.visibility)?;
                require(BACKEND, Slot::Draw, &opts.draw)?
            },
            traversal: require(BACKEND, Slot::Traversal, &opts.traversal)?,
            shading: require(BACKEND, Slot::Shading, &opts.shading)?,
        };
        // The host hands the device in (GPU slot 0); this back end only states its needs.
        let gpu = mr_gpu::gpu(devices, 0, &GpuNeeds { experimental: false, required_features: wgpu::Features::INDIRECT_FIRST_INSTANCE, optional_features: wgpu::Features::TIMESTAMP_QUERY | shade_state::FRAGMENT_FEATURE })?;
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
        let drawer = layouts.drawer(&gpu, &comp, p.width, p.height)?;
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
        let (occ, walk) = mr_components::shaders::walk_program(&comp.storage(), &comp.traversal, false);
        let mesh = GpuMeshes::new(&gpu, &world, wgpu::TextureFormat::Rgba8Unorm, Occlusion { program: &occ, rays: &walk.rays, layout: &layouts.aux });
        let ring = FrameRing::new(&gpu, p.width, p.height, opts.frames_in_flight.max(1), waker.clone());
        let accum = ShadeState::new(&gpu, p.width, p.height, &comp.shading, opts.bounces, opts.spp, opts.accumulate);
        tracing::info!("gpu-raster: storage {}, geometry {}, draw {}, traversal {}, shading {}", comp.storage, comp.geometry, comp.draw, comp.traversal, comp.shading);
        Ok(Self {
            mesh,
            output: mr_gpu::output_texture(&gpu, p.width, p.height),
            depth: depth_view(&gpu, p.width, p.height),
            ring,
            world,
            opts,
            built: comp.clone(),
            comp,
            drawer,
            layouts,
            frame_buf,
            quad_buf,
            chunk_buf,
            indirect,
            bind0: None,
            accum,
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
        if !updates.is_empty() {
            self.accum.invalidate();
        }
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
        // The meshes stage draws meshes (`depth`), or not (`none`).
        let want_meshes = req.meshes && self.opts.meshes != "none";
        if self.comp != self.built {
            self.drawer = self.layouts.drawer(&self.gpu, &self.comp, self.width, self.height)?;
            if self.comp.geometry != self.built.geometry {
                self.remesh_all();
            }
            if self.accum.set_shading(&self.gpu, &self.comp.shading) {
                self.bind0 = None;
            }
            self.built = self.comp.clone();
        }
        if !self.built.deferred() && req.mode == RenderMode::Lit && !self.opts.depth_prepass && shade_state::stateful(&self.built.shading) {
            anyhow::bail!("`{}` shading with `raster` draw needs depth_prepass in lit mode (exactly one fragment per pixel may update its state)", self.built.shading);
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
        self.accum.begin(&self.gpu, c, req.mode);
        let world_bg = self.world.prepare(&self.gpu, c).clone();
        if want_meshes {
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
                    self.accum.bind_entries(3)[0].clone(),
                    self.accum.bind_entries(3)[1].clone(),
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
        let frame = DrawFrame {
            out: &out_view,
            depth: &self.depth,
            clear,
            prepass: self.opts.depth_prepass && req.mode == RenderMode::Lit,
            keep_depth: false,
            binds: &[&bind0, &world_bg, &bind2],
            ring: &self.ring,
            ticket,
            ts: (true, true),
        };
        let draw = |pass: &mut wgpu::RenderPass<'_>| {
            if n > 0 {
                pass.multi_draw_indirect(&self.indirect, 0, n);
            }
        };
        // Meshes: in the faces' forward passes; after shading (depth-tested) when deferred.
        let meshes = want_meshes && (!self.drawer.mode().deferred() || self.mesh.triangles() > 0);
        let mesh_draw = |pass: &mut wgpu::RenderPass<'_>, mp: MeshPass| self.mesh.draw(pass, mp, &world_bg);
        self.drawer.encode(&self.gpu, &mut enc, &frame, &draw, meshes.then_some(&mesh_draw as &dyn Fn(&mut wgpu::RenderPass<'_>, MeshPass)), None);
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            legend: None,
            counters: vec![
                ("instances".into(), drawn as f64),
                ("instances_total".into(), total as f64),
                ("draws".into(), n as f64),
                ("remeshed".into(), remeshed as f64),
                ("mesh_ms".into(), mesh_ms),
                ("mesh_triangles".into(), if want_meshes { self.mesh.triangles() as f64 } else { 0.0 }),
            ]
            .into_iter()
            .chain(self.data.counters())
            .chain(self.accum.counter(req.mode))
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
            "geometry" => self.comp.geometry = require(BACKEND, Slot::Geometry, &s())?,
            "visibility" => drop(require(BACKEND, Slot::Visibility, &s())?),
            "draw" => self.comp.draw = require(BACKEND, Slot::Draw, &s())?,
            "shading" => self.comp.shading = require(BACKEND, Slot::Shading, &s())?,
            "meshes" => self.opts.meshes = require(BACKEND, Slot::Meshes, &s())?,
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
            _ if self.accum.set_param(name, value).is_some() => {}
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
        self.drawer.resize(&self.gpu, width, height);
        self.accum.resize(&self.gpu, width, height);
        self.bind0 = None;
        self.ring = FrameRing::new(&self.gpu, width, height, self.opts.frames_in_flight.max(1), self.waker.clone());
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        let gpu = self.world.memory_bytes() + self.data.memory_bytes() + self.quad_buf.size() + self.chunk_buf.size() + self.indirect.size() + self.accum.memory_bytes() + self.drawer.memory_bytes() + self.width as u64 * self.height as u64 * 8;
        MemoryReport { cpu_bytes: 0, gpu_bytes: gpu }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every composition's shader composes from its modules and validates (no GPU needed).
    #[test]
    fn programs_compose() {
        use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        let mut n = 0;
        for st in names_on(BACKEND, Slot::Storage) {
            for k in names_on(BACKEND, Slot::Traversal) {
                for g in names_on(BACKEND, Slot::Geometry) {
                    for d in names_on(BACKEND, Slot::Draw) {
                        for shading in names_on(BACKEND, Slot::Shading) {
                            let c = Composition { storage: st.into(), geometry: g.into(), draw: d.into(), traversal: k.into(), shading: shading.into() };
                            let (p, root, deps) = c.program();
                            let deps: Vec<&str> = deps.iter().map(String::as_str).collect();
                            let m = p.naga(&root, &deps).unwrap_or_else(|e| panic!("{c:?}: {e}"));
                            Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("{c:?}: {e:?}"));
                            n += 1;
                        }
                    }
                }
            }
        }
        assert!(n > 100, "{n}");
    }
}
