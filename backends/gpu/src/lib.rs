//! The GPU executor (PLAN step 13e3): every pipeline whose image is made on the GPU, one
//! executor over one core with a lane per kind of visibility node:
//! - faces ([`faces`]): the shared face drawer over a set of block faces from meshing
//!   (`gpu-raster`) or discovery on the GPU or the CPU (`gpu-discover`, `cpu-discover`);
//! - ray casting ([`raycast`]): one primary ray per pixel, walked over storages
//!   (`gpu-compute`) or cast as ray queries (`gpu-rt`);
//! - splatting ([`splat`]): one point per exposed block (`gpu-splat`);
//! - columns ([`columns`]): the Voxel Space heightmap march (`gpu-voxelspace`).
//!
//! Which lane runs is the graph's: faces when a drawer draws them, else by the visibility
//! node's component. The rest is shared ([`Core`]): the GPU device, the voxel world, the
//! storages the GPU's rays walk, the shading state, the camera uniform and group 0 (camera,
//! output, shading state, hit records, then the lane's), the output, the frame ring and the
//! timed stages' nodes.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::IVec3;
use mr_components::graph::{self, Role};
use mr_components::shaders::Walks;
use mr_components::shading::state::ShadeState;
use mr_components::storage::gpu_data::{self, StorageSet};
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::voxels::{Shadows, VoxelWorld, VoxelWorldConfig};
use mr_gpu::{FrameRing, Gpu, GpuNeeds, wgpu};

mod columns;
mod faces;
mod raycast;
mod splat;

pub fn gpu_raster_entry() -> RendererEntry {
    RendererEntry { info: faces::mesh::info, create: |p, d, w| create(faces::mesh::BACKEND, p, d, w) }
}

pub fn gpu_discover_entry() -> RendererEntry {
    RendererEntry { info: faces::gpu::info, create: |p, d, w| create(faces::gpu::BACKEND, p, d, w) }
}

pub fn cpu_discover_entry() -> RendererEntry {
    RendererEntry { info: faces::cpu::info, create: |p, d, w| create(faces::cpu::BACKEND, p, d, w) }
}

pub fn gpu_compute_entry() -> RendererEntry {
    RendererEntry { info: raycast::walk::info, create: |p, d, w| create(raycast::walk::BACKEND, p, d, w) }
}

pub fn gpu_rt_entry() -> RendererEntry {
    RendererEntry { info: raycast::rayquery::info, create: |p, d, w| create(raycast::rayquery::BACKEND, p, d, w) }
}

pub fn gpu_splat_entry() -> RendererEntry {
    RendererEntry { info: splat::info, create: |p, d, w| create(splat::BACKEND, p, d, w) }
}

pub fn gpu_voxelspace_entry() -> RendererEntry {
    RendererEntry { info: columns::info, create: |p, d, w| create(columns::BACKEND, p, d, w) }
}

/// The executor for `backend`'s graph: the faces lane when a drawer draws them, else the lane
/// of the visibility node's component.
fn create(backend: &str, p: InitParams, devices: &Devices, waker: Waker) -> anyhow::Result<Box<dyn Renderer>> {
    let g = graph::of_init(backend, &p)?;
    if g.find(Role::Draw).is_some() {
        return faces::create(backend, &p, devices, waker, &g);
    }
    let vis = g.find(Role::Visibility).ok_or_else(|| anyhow::anyhow!("{backend}: no visibility node"))?;
    match g.node(vis).component.as_str() {
        "splat" => run(splat::Splat::new(&p, devices, waker, &g)?),
        "columns" => run(columns::Columns::new(&p, devices, waker, &g)?),
        _ => raycast::create(backend, &p, devices, waker, &g),
    }
}

fn run<L: Lane + 'static>((core, lane): (Core, L)) -> anyhow::Result<Box<dyn Renderer>> {
    Ok(Box::new(Executor { core, lane }))
}

/// The camera uniform every lane's root but the face drawer's reads (group 0, binding 0); the
/// last two words are the lane's.
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
    lane: [u32; 2],
}

/// Group 0: the camera (0), the output (1), the shading state (2-3), the hit records (4,
/// `placement = deferred`), the lane's bindings from `LANE_BINDING`.
const STATE_BINDING: u32 = 2;
const RECORDS_BINDING: u32 = 4;
const LANE_BINDING: u32 = 8;

/// What a lane needs of the core.
struct CoreSpec<'a> {
    gpu: GpuNeeds,
    /// The walks of the GPU's rays: the storages whose data the core keeps.
    walks: &'a Walks,
    /// The stages that read group 0, the world and the storages.
    stages: wgpu::ShaderStages,
    /// LOD levels streamed and walked.
    max_lod: u8,
    /// The lane reads the voxel pool itself (beyond its storages' needs).
    voxels: bool,
    /// One hit record per pixel (`placement = deferred`; fixed at creation).
    records: bool,
    shading: &'a str,
    bounces: u32,
    spp: u32,
    accumulate: bool,
    shadows: Shadows,
    max_steps: u32,
    pool_mb: u32,
    frames_in_flight: usize,
}

/// Everything but the lane.
pub(crate) struct Core {
    gpu: Arc<Gpu>,
    world: VoxelWorld,
    /// The storage instances the GPU's rays walk (PLAN step 9c).
    storages: StorageSet,
    /// Shading state (pathtrace's accumulation).
    accum: ShadeState,
    camera_buf: wgpu::Buffer,
    /// placement = deferred: the hit records (one `pack_hit` per pixel).
    records: Option<wgpu::Buffer>,
    output: wgpu::Texture,
    stages: wgpu::ShaderStages,
    ring: FrameRing,
    frames_in_flight: usize,
    /// The nodes of the timed stages (PLAN step 11): visibility with shading, visibility,
    /// shading.
    stage_nodes: [Vec<String>; 3],
    width: u32,
    height: u32,
    waker: Waker,
}

/// The records of `placement = deferred` (8 bytes per pixel).
fn records(gpu: &Gpu, width: u32, height: u32) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("hit records"), size: (width as u64 * height as u64 * 8).max(16), usage: wgpu::BufferUsages::STORAGE, mapped_at_creation: false })
}

/// A storage buffer's layout entry.
fn storage_entry(binding: u32, read_only: bool, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only }, has_dynamic_offset: false, min_binding_size: None }, count: None }
}

impl Core {
    fn new(p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked, s: CoreSpec) -> anyhow::Result<Self> {
        // The host hands the device in (GPU slot 0); the executor only states its needs.
        let gpu = mr_gpu::gpu(devices, 0, &s.gpu)?;
        let world = VoxelWorld::new(
            &gpu,
            &p.blocks,
            p.environment,
            VoxelWorldConfig {
                max_lod: s.max_lod,
                // Region storages (tree64, ...) hold their own data instead of the voxel pool.
                upload_voxels: s.voxels || StorageSet::needs_pool(s.walks),
                pool_mb: s.pool_mb,
                view_radius: p.view_radius_chunks,
                shadows: s.shadows,
                max_steps: s.max_steps,
                visibility: s.stages,
            },
        )?;
        let storages = StorageSet::new(&gpu, s.walks, s.stages, gpu_data::Limits { mb: s.pool_mb, view_radius: p.view_radius_chunks, wide_ids: p.blocks.blocks.len() > 256 });
        if storages.groups_end() > gpu.device.limits().max_bind_groups {
            anyhow::bail!("{} storages need {} bind groups; this device has {}", s.walks.storages.len(), storages.groups_end(), gpu.device.limits().max_bind_groups);
        }
        let fif = s.frames_in_flight.max(1);
        Ok(Self {
            accum: ShadeState::new(&gpu, p.width, p.height, s.shading, s.bounces, s.spp, s.accumulate),
            camera_buf: gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("camera"), size: size_of::<CameraUniform>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }),
            records: s.records.then(|| records(&gpu, p.width, p.height)),
            output: mr_gpu::output_texture(&gpu, p.width, p.height),
            stages: s.stages,
            ring: FrameRing::new(&gpu, p.width, p.height, fif, waker.clone()).with_stages(p.stage_times),
            frames_in_flight: fif,
            stage_nodes: [g.ids(&[Role::Visibility, Role::Shading]), g.ids(&[Role::Visibility]), g.ids(&[Role::Shading])],
            width: p.width,
            height: p.height,
            waker,
            storages,
            world,
            gpu,
        })
    }

    /// Group 0's entries before the lane's.
    fn layout_entries(&self) -> Vec<wgpu::BindGroupLayoutEntry> {
        let mut e = vec![
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: self.stages,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly, format: wgpu::TextureFormat::Rgba8Unorm, view_dimension: wgpu::TextureViewDimension::D2 },
                count: None,
            },
        ];
        e.extend(ShadeState::layout_entries(STATE_BINDING, self.stages));
        e.extend(self.records.as_ref().map(|_| storage_entry(RECORDS_BINDING, false, self.stages)));
        e
    }

    /// Their resources, the output's `view`.
    fn bind_entries<'a>(&'a self, view: &'a wgpu::TextureView) -> Vec<wgpu::BindGroupEntry<'a>> {
        let mut e = vec![wgpu::BindGroupEntry { binding: 0, resource: self.camera_buf.as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) }];
        e.extend(self.accum.bind_entries(STATE_BINDING));
        e.extend(self.records.as_ref().map(|r| wgpu::BindGroupEntry { binding: RECORDS_BINDING, resource: r.as_entire_binding() }));
        e
    }

    /// Writes the camera uniform for `c` (`lane`: the lane's two words).
    fn write_camera(&self, c: &Camera, mode: RenderMode, lane: [u32; 2]) {
        let cam = CameraUniform {
            forward: c.forward().to_array(),
            tan_half_fov: (c.fov_y * 0.5).tan(),
            right: c.right().to_array(),
            aspect: self.width as f32 / self.height as f32,
            up: c.up().to_array(),
            mode: (mode == RenderMode::Lit) as u32,
            origin: c.split_position().1.to_array(),
            sky: u32::from_le_bytes(encode_srgb(self.world.env.sky)),
            width: self.width,
            height: self.height,
            lane,
        };
        self.gpu.queue.write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&cam));
    }

    /// The knobs every lane shares; `None` if `name` is not one.
    fn set_param(&mut self, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        Some(match name {
            "shadows" => self.world.cfg.shadows.set(value).map(|_| ()),
            "max_steps" => {
                self.world.cfg.max_steps = value.as_i64().unwrap_or(2048) as u32;
                Ok(())
            }
            _ => return self.accum.set_param(name, value).map(|_| Ok(())),
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.output = mr_gpu::output_texture(&self.gpu, width, height);
        self.accum.resize(&self.gpu, width, height);
        if self.records.is_some() {
            self.records = Some(records(&self.gpu, width, height));
        }
        self.ring = FrameRing::new(&self.gpu, width, height, self.frames_in_flight, self.waker.clone()).with_stages(self.ring.stages());
    }

    fn memory_bytes(&self) -> u64 {
        self.world.memory_bytes() + self.storages.memory_bytes() + self.accum.memory_bytes() + self.records.as_ref().map_or(0, |r| r.size()) + self.width as u64 * self.height as u64 * 4
    }
}

/// A lane: the visibility node's implementation and what it keeps.
trait Lane {
    /// World changes before the GPU world applies them.
    fn updating(&mut self, _updates: &[WorldUpdate]) {}
    /// Chunks the GPU world (and the storages) changed.
    fn updated(&mut self, core: &Core, changed: &[IVec3]) -> anyhow::Result<()>;
    fn update_meshes(&mut self, _update: MeshUpdate) {}
    fn render(&mut self, core: &mut Core, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()>;
    /// Its knobs; `None` if `name` is not one.
    fn set_param(&mut self, core: &mut Core, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>>;
    /// After the core's resize.
    fn resize(&mut self, core: &Core);
    /// (CPU, GPU) bytes beyond the core's.
    fn memory(&self, core: &Core) -> (u64, u64);
}

/// The core and one lane (static dispatch).
struct Executor<L> {
    core: Core,
    lane: L,
}

impl<L: Lane> Renderer for Executor<L> {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        if !updates.is_empty() {
            self.core.accum.invalidate();
        }
        self.lane.updating(&updates);
        let c = &mut self.core;
        let changed = c.world.apply(&c.gpu, updates);
        c.storages.update(&c.gpu, &c.world, &changed)?;
        self.lane.updated(&self.core, &changed)
    }

    fn update_meshes(&mut self, update: MeshUpdate) -> anyhow::Result<()> {
        self.lane.update_meshes(update);
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.lane.render(&mut self.core, req, sink)
    }

    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.core.ring.poll(&self.core.gpu, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        if let Some(r) = self.lane.set_param(&mut self.core, name, value) {
            return r;
        }
        self.core.set_param(name, value).unwrap_or_else(|| anyhow::bail!("`{name}` cannot change while running"))
    }

    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.core.ring.poll(&self.core.gpu, sink, true)
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.core.resize(width, height);
        self.lane.resize(&self.core);
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        let (cpu, gpu) = self.lane.memory(&self.core);
        MemoryReport { cpu_bytes: cpu, gpu_bytes: self.core.memory_bytes() + gpu }
    }
}
