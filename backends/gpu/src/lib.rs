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
//! storages the GPU's rays walk, the acceleration structures its ray queries trace
//! ([`accel`]), the shading state, the camera uniform and group 0 (camera, output, shading
//! state, hit records, acceleration structures, then the lane's), the output, the frame ring
//! and the timed stages' nodes.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::IVec3;
use mr_components::graph::{self, Role};
use mr_components::shaders::Walks;
use mr_components::shading::state::ShadeState;
use mr_components::storage::gpu_data::{self, StorageSet};
use mr_core::*;
use mr_gpu::voxels::{Shadows, VoxelWorld, VoxelWorldConfig};
use mr_gpu::{FrameRing, Gpu, GpuNeeds, wgpu};

mod accel;
mod columns;
mod faces;
mod raycast;
mod remote;
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

/// The camera uniform every lane's root reads (group 0, binding 0); `lane` holds the lane's
/// two words. The rasterising lanes also read the view-projection and the anchor (the faces
/// lane: the CPU sampler's face records are relative to it; PLAN step 13e4d), which the
/// others' shaders leave undeclared.
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
    view_proj: [[f32; 4]; 4],
    anchor: [i32; 4],
}

/// Group 0: the camera (0), the output (1), the shading state (2-3), the hit records (4,
/// `placement = deferred`), the acceleration structures (5-7, `accel::ACCEL_BINDING`), the
/// lane's bindings from `LANE_BINDING`.
const STATE_BINDING: u32 = 2;
const RECORDS_BINDING: u32 = 4;
const LANE_BINDING: u32 = 8;

/// What a lane needs of the core.
#[derive(Clone)]
struct CoreSpec<'a> {
    gpu: GpuNeeds,
    /// The walks of the GPU's rays: the storages whose data the core keeps (and the geometry
    /// node of its ray queries' acceleration structures, built as `accel` says).
    walks: &'a Walks,
    accel: Option<accel::AccelSpec>,
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
    /// The acceleration structures the GPU's ray queries trace (PLAN step 13e4b), and the
    /// geometry node they are built from (their timed stage).
    accel: Option<accel::Accel>,
    accel_nodes: Vec<String>,
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
    /// The post's tone mapper (the walks'; PLAN step 15b): in the programs' `display`, and
    /// here for the sky.
    tone: mr_core::lighting::Tone,
}

/// The records of `placement = deferred` (8 bytes per pixel; copied between GPUs when the
/// shading runs on another, [`remote`]).
fn records(gpu: &Gpu, width: u32, height: u32) -> wgpu::Buffer {
    let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST;
    gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("hit records"), size: records_size(width, height), usage, mapped_at_creation: false })
}

fn records_size(width: u32, height: u32) -> u64 {
    (width as u64 * height as u64 * 8).max(16)
}

/// The GPU slot a pipeline's nodes run on (the device configuration's, PLAN step 13e4c): its
/// drawer's, else its visibility node's (the shading may run on another: [`remote`]).
fn home_slot(g: &graph::Checked) -> usize {
    let at = |r| g.find(r).map(|id| g.devices[id]);
    match at(Role::Draw).or(at(Role::Visibility)) {
        Some(graph::Device::Gpu(i)) => i as usize,
        _ => 0,
    }
}

/// A storage buffer's layout entry.
fn storage_entry(binding: u32, read_only: bool, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only }, has_dynamic_offset: false, min_binding_size: None }, count: None }
}

impl Core {
    fn new(p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked, s: CoreSpec) -> anyhow::Result<Self> {
        Self::on(home_slot(g), p, devices, waker, g, s)
    }

    /// The core on GPU slot `slot`.
    fn on(slot: usize, p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked, s: CoreSpec) -> anyhow::Result<Self> {
        // The host hands the device in (the slot's adapter); the executor only states its
        // needs (ray queries' with acceleration structures).
        let mut needs = s.gpu;
        if s.accel.is_some() {
            needs.experimental = true;
            needs.required_features |= wgpu::Features::EXPERIMENTAL_RAY_QUERY;
        }
        let gpu = mr_gpu::gpu(devices, slot, &needs)?;
        let accel = s.accel.as_ref().map(|a| accel::Accel::new(&gpu, a)).transpose()?;
        let world = VoxelWorld::new(
            &gpu,
            &p.blocks,
            p.environment,
            VoxelWorldConfig {
                max_lod: s.max_lod,
                // Region storages (tree64, ...) hold their own data instead of the voxel pool.
                upload_voxels: s.voxels || StorageSet::needs_pool(s.walks) || accel.as_ref().is_some_and(|a| a.reads_voxels()),
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
            tone: s.walks.post,
            storages,
            accel_nodes: s.walks.geometry.iter().map(|g| g.node.clone()).collect(),
            accel,
            world,
            gpu,
        })
    }

    /// The sky's pixel (a linear colour, through the post).
    fn sky(&self) -> [u8; 4] {
        self.tone.encode(self.world.env.sky)
    }

    /// Adds the ray queries' root code to `p` (the bindings and the trace; `flags`: the ray
    /// flags of rays that are not shadow rays, a WGSL expression), if the pipeline has them.
    fn trace_root(&self, p: &mut mr_gpu::program::Program, flags: &str) {
        if let Some(a) = &self.accel {
            p.root(format!("enable wgpu_ray_query;\n{}", a.wgsl(flags)));
        }
    }

    /// This frame's acceleration structures, built into `enc` before the lane's passes (its
    /// clock starts first, so the frame's GPU time holds the builds; a timed stage of the
    /// geometry node). Returns whether group 0's bindings changed.
    fn build_accel(&mut self, enc: &mut wgpu::CommandEncoder, ticket: mr_gpu::Ticket, req: &FrameRequest) -> bool {
        let Some(a) = &mut self.accel else { return false };
        self.ring.start_clock(&self.gpu, enc, ticket);
        self.ring.stage(enc, ticket, "geometry", &self.accel_nodes);
        a.build(&self.world, enc, req.camera.split_position().0)
    }

    /// The acceleration structures' counters and CPU stage time.
    fn accel_stats(&self) -> (Vec<(String, f64)>, Vec<StageTime>) {
        self.accel.as_ref().map_or_else(Default::default, |a| (a.counters(), vec![StageTime::cpu("geometry", &self.accel_nodes, a.build_ms as f32)]))
    }

    /// Group 0's entries before the lane's.
    fn layout_entries(&self) -> Vec<wgpu::BindGroupLayoutEntry> {
        self.layout_entries_with(true)
    }

    /// Group 0's entries before the lane's; without the output where the lane's output is a
    /// render attachment (the faces lane).
    fn layout_entries_with(&self, output: bool) -> Vec<wgpu::BindGroupLayoutEntry> {
        let mut e = vec![wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: self.stages,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        }];
        if output {
            e.push(wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly, format: wgpu::TextureFormat::Rgba8Unorm, view_dimension: wgpu::TextureViewDimension::D2 },
                count: None,
            });
        }
        // Writable buffers are not visible to vertex shaders (the faces lane's).
        let writable = self.stages - wgpu::ShaderStages::VERTEX;
        e.extend(ShadeState::layout_entries(STATE_BINDING, writable));
        e.extend(self.records.as_ref().map(|_| storage_entry(RECORDS_BINDING, false, writable)));
        e.extend(self.accel.iter().flat_map(|a| a.layout_entries(self.stages)));
        e
    }

    /// Their resources, the output's `view`.
    fn bind_entries<'a>(&'a self, view: &'a wgpu::TextureView) -> Vec<wgpu::BindGroupEntry<'a>> {
        self.bind_entries_with(Some(view))
    }

    fn bind_entries_with<'a>(&'a self, view: Option<&'a wgpu::TextureView>) -> Vec<wgpu::BindGroupEntry<'a>> {
        let mut e = vec![wgpu::BindGroupEntry { binding: 0, resource: self.camera_buf.as_entire_binding() }];
        e.extend(view.map(|v| wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(v) }));
        e.extend(self.accum.bind_entries(STATE_BINDING));
        e.extend(self.records.as_ref().map(|r| wgpu::BindGroupEntry { binding: RECORDS_BINDING, resource: r.as_entire_binding() }));
        e.extend(self.accel.iter().flat_map(|a| a.bind_entries()));
        e
    }

    /// Writes the camera uniform for `c` (`lane`: the lane's two words).
    fn write_camera(&self, c: &Camera, mode: RenderMode, lane: [u32; 2]) {
        self.write_camera_at(c, mode, lane, IVec3::ZERO);
    }

    /// The same with `anchor` (the faces lane's CPU records' origin).
    fn write_camera_at(&self, c: &Camera, mode: RenderMode, lane: [u32; 2], anchor: IVec3) {
        let aspect = self.width as f32 / self.height as f32;
        let cam = CameraUniform {
            forward: c.forward().to_array(),
            tan_half_fov: (c.fov_y * 0.5).tan(),
            right: c.right().to_array(),
            aspect,
            up: c.up().to_array(),
            mode: (mode == RenderMode::Lit) as u32,
            origin: c.split_position().1.to_array(),
            sky: u32::from_le_bytes(self.sky()),
            width: self.width,
            height: self.height,
            lane,
            view_proj: (c.projection(aspect) * c.view_rotation()).to_cols_array_2d(),
            anchor: [anchor.x, anchor.y, anchor.z, 0],
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
            _ => return self.accel.as_mut().and_then(|a| a.set_param(name, value)).or_else(|| self.accum.set_param(name, value).map(|_| Ok(()))),
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
        self.world.memory_bytes() + self.storages.memory_bytes() + self.accel.as_ref().map_or(0, |a| a.memory_bytes()) + self.accum.memory_bytes() + self.records.as_ref().map_or(0, |r| r.size()) + self.width as u64 * self.height as u64 * 4
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
    /// Completes finished frames (`wait`: all of them).
    fn poll(&mut self, core: &mut Core, sink: &mut dyn FrameSink, wait: bool) -> anyhow::Result<()> {
        core.ring.poll(&core.gpu, sink, wait)
    }
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
        if let Some(a) = &mut c.accel {
            a.updated(&c.world, &changed);
        }
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
        self.lane.poll(&mut self.core, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        if let Some(r) = self.lane.set_param(&mut self.core, name, value) {
            return r;
        }
        self.core.set_param(name, value).unwrap_or_else(|| anyhow::bail!("`{name}` cannot change while running"))
    }

    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.lane.poll(&mut self.core, sink, true)
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
