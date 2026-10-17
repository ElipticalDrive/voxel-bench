//! The ray-casting executor (PLAN step 13e2): every pipeline whose image is one primary ray
//! per pixel, shaded inline or (`placement = deferred`) through hit records and the common
//! shading pass. One executor for the two ways the visibility node's primary rays are cast,
//! its kernel's API:
//! - `gpu-compute` (`compute`, [`walk`]): a walk, the traversal kernel over a storage component
//!   (any storage, any kernel, missing capabilities from adapters), with an optional prepass,
//!   meshes' records (mesh mode), LOD and the steps view; pixels in compute threads or a
//!   fragment shader;
//! - `gpu-rt` (`rayquery`, [`rayquery`]): hardware ray queries over acceleration structures
//!   built from the geometry component (triangles or brick boxes).
//!
//! Which lane runs is the graph's: the primary rays' kernel node's API. The rest is shared
//! ([`Core`]): the GPU device, the voxel world, the shading state, the camera uniform, the
//! output, the hit records, the frame ring; and the passes ([`Passes`]): the shader (the lane's
//! root and modules, then `cast.wgsl`, and when deferred `deferred.wgsl` with the common pass),
//! the rays' pass and the deferred shading pass.

use std::sync::Arc;
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use glam::IVec3;
use mr_components::Api;
use mr_components::graph::{self, Role};
use mr_components::shading::state::ShadeState;
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::voxels::{Shadows, VoxelWorld, VoxelWorldConfig};
use mr_gpu::{FrameRing, Gpu, GpuNeeds, Ticket, wgpu};

mod rayquery;
mod walk;

pub fn gpu_compute_entry() -> RendererEntry {
    RendererEntry { info: walk::info, create: |p, d, w| Raycast::create(walk::BACKEND, p, d, w) }
}

pub fn gpu_rt_entry() -> RendererEntry {
    RendererEntry { info: rayquery::info, create: |p, d, w| Raycast::create(rayquery::BACKEND, p, d, w) }
}

/// The camera uniform of `cast.wgsl`, shared by both lanes.
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
    view_max: u32,
    ray_flags: u32,
}

/// Group 0 of the shading state (`sp`, `accum`) and the hit records; the lane's bindings start
/// at `LANE_BINDING`.
const STATE_BINDING: u32 = 2;
const RECORDS_BINDING: u32 = 4;
const LANE_BINDING: u32 = 8;

/// What the core needs of a lane's options.
struct CoreOpts<'a> {
    shading: &'a str,
    placement: &'a str,
    bounces: u32,
    spp: u32,
    accumulate: bool,
    shadows: Shadows,
    max_steps: u32,
    pool_mb: u32,
    frames_in_flight: usize,
}

/// What a lane needs of the GPU and the voxel world.
struct LaneNeeds {
    gpu: GpuNeeds,
    /// LOD levels streamed and walked.
    max_lod: u8,
    /// The voxel pool is uploaded (walks over pool storages, boxes' DDA).
    upload_voxels: bool,
    /// The stages that read group 0 and the world (fragment too with `dispatch = fragment`).
    stages: wgpu::ShaderStages,
}

/// Everything but the lane: the GPU ray-casting lane of every configuration.
pub(crate) struct Core {
    gpu: Arc<Gpu>,
    world: VoxelWorld,
    /// Shading state (pathtrace's accumulation).
    accum: ShadeState,
    shading: String,
    /// Shading the passes were compiled with (recompiled at the next frame on a change).
    built_shading: String,
    /// placement = deferred (fixed at creation: only then does group 0 carry the records):
    /// the hit records the common pass reads (one `pack_hit` per pixel).
    records: Option<wgpu::Buffer>,
    camera_buf: wgpu::Buffer,
    output: wgpu::Texture,
    stages: wgpu::ShaderStages,
    ring: FrameRing,
    frames_in_flight: usize,
    /// The nodes of the timed stages: rays and shading, and when deferred, apart (PLAN step 11).
    stage_nodes: [Vec<String>; 3],
    width: u32,
    height: u32,
    waker: Waker,
}

/// The records of `placement = deferred` (8 bytes per pixel).
fn records(gpu: &Gpu, width: u32, height: u32) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("hit records"), size: (width as u64 * height as u64 * 8).max(16), usage: wgpu::BufferUsages::STORAGE, mapped_at_creation: false })
}

impl Core {
    fn new(p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked, o: &CoreOpts, needs: LaneNeeds) -> anyhow::Result<Self> {
        // The host hands the device in (GPU slot 0); the executor only states its needs.
        let gpu = mr_gpu::gpu(devices, 0, &needs.gpu)?;
        let world = VoxelWorld::new(
            &gpu,
            &p.blocks,
            p.environment,
            VoxelWorldConfig {
                max_lod: needs.max_lod,
                upload_voxels: needs.upload_voxels,
                pool_mb: o.pool_mb,
                view_radius: p.view_radius_chunks,
                shadows: o.shadows,
                max_steps: o.max_steps,
                visibility: needs.stages,
            },
        )?;
        let deferred = mr_components::parse_placement(o.placement)?;
        let fif = o.frames_in_flight.max(1);
        Ok(Self {
            accum: ShadeState::new(&gpu, p.width, p.height, o.shading, o.bounces, o.spp, o.accumulate),
            shading: o.shading.to_string(),
            built_shading: o.shading.to_string(),
            records: deferred.then(|| records(&gpu, p.width, p.height)),
            camera_buf: gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("camera"), size: size_of::<CameraUniform>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }),
            output: mr_gpu::output_texture(&gpu, p.width, p.height),
            stages: needs.stages,
            ring: FrameRing::new(&gpu, p.width, p.height, fif, waker.clone()).with_stages(p.stage_times),
            frames_in_flight: fif,
            stage_nodes: [g.ids(&[Role::Visibility, Role::Shading]), g.ids(&[Role::Visibility]), g.ids(&[Role::Shading])],
            width: p.width,
            height: p.height,
            waker,
            world,
            gpu,
        })
    }

    /// Group 0's entries before the lane's: the camera, the output, the shading state, the
    /// records.
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
        if self.records.is_some() {
            e.push(wgpu::BindGroupLayoutEntry {
                binding: RECORDS_BINDING,
                visibility: self.stages,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            });
        }
        e
    }

    /// Writes the camera uniform for `c` (`view_max`, `ray_flags`: the lane's).
    fn write_camera(&self, c: &Camera, mode: RenderMode, view_max: u32, ray_flags: u32) {
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
            view_max,
            ray_flags,
        };
        self.gpu.queue.write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&cam));
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
        self.world.memory_bytes() + self.accum.memory_bytes() + self.records.as_ref().map_or(0, |r| r.size()) + self.width as u64 * self.height as u64 * 4
    }
}

/// A lane's shader: its modules, its root (bindings from `LANE_BINDING`, `pixel_hit` and what
/// it needs) and the root's dependencies.
struct LaneProgram {
    program: mr_gpu::program::Program,
    root: String,
    deps: Vec<String>,
}

/// What the lane's work before the rays left for the rays' pass.
struct Before {
    /// The rays' pass starts the frame's clock (no earlier pass or encoder timestamp did).
    first: bool,
    /// This frame has mesh records (the meshed passes run).
    meshed: bool,
}

/// How the primary rays are cast: a walk over storages or ray queries over acceleration
/// structures, with what each keeps.
trait Lane: Sized {
    /// The registry's name of this configuration.
    const BACKEND: &'static str;
    /// Group 0's entries from `LANE_BINDING`.
    fn layout_entries(&self, stages: wgpu::ShaderStages) -> Vec<wgpu::BindGroupLayoutEntry>;
    /// Their resources (stale after `Core`'s bind group is dropped: see `Passes::unbind`).
    fn bind_entries(&self) -> Vec<wgpu::BindGroupEntry<'_>>;
    /// The bind group layouts after group 0 and the world's (group 1).
    fn layouts<'a>(&'a self, base: [&'a wgpu::BindGroupLayout; 2]) -> Vec<Option<&'a wgpu::BindGroupLayout>>;
    /// The shader with `shading`; `meshed`: the variant for frames with mesh records.
    fn program(&self, shading: &str, meshed: bool) -> LaneProgram;
    /// The lane's knobs changed what its shader compiles (cleared by `compiled`).
    fn stale(&self) -> bool;
    fn compiled(&mut self);
    /// The variant with mesh records exists (a meshes stage).
    fn meshes(&self) -> bool {
        false
    }
    /// Pixels run in a fragment shader (`dispatch = fragment`).
    fn fragment(&self) -> bool {
        false
    }
    /// Shading runs inline whatever the placement (the steps view shows the walk itself).
    fn inline_only(&self) -> bool {
        false
    }
    /// The lane's own compute entry points in the module (the prepass's).
    fn entries(&self) -> &'static [&'static str] {
        &[]
    }
    /// The camera's `view_max` and `ray_flags`.
    fn camera(&self) -> (u32, u32);
    /// World changes the GPU world applied (`changed`: its chunks).
    fn updated(&mut self, core: &Core, changed: &[IVec3]) -> anyhow::Result<()>;
    fn update_meshes(&mut self, _update: MeshUpdate) {}
    /// Work before the camera is written (acceleration structures); may drop group 0's
    /// bind group (`passes.unbind()`).
    fn prepare(&mut self, core: &mut Core, passes: &mut Passes, enc: &mut wgpu::CommandEncoder, ticket: Ticket, req: &FrameRequest);
    /// Passes before the rays (meshes, the prepass), with group 0 and the world bound.
    fn before(&mut self, core: &mut Core, passes: &Passes, enc: &mut wgpu::CommandEncoder, ticket: Ticket, req: &FrameRequest, world: &wgpu::BindGroup) -> Before;
    /// The lane's bind groups after group 1.
    fn groups(&self) -> impl Iterator<Item = (u32, &wgpu::BindGroup)>;
    /// The frame's counters before the shading state's, its legend and CPU stage times.
    fn stats(&self, core: &Core) -> (Vec<(String, f64)>, Option<Legend>, Vec<StageTime>);
    /// Before and after the frame's submission (readbacks).
    fn finish(&mut self, _enc: &mut wgpu::CommandEncoder) {}
    fn submitted(&mut self) {}
    /// Its knobs; `None` if `name` is not one.
    fn set_param(&mut self, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>>;
    fn resize(&mut self, core: &Core);
    /// GPU bytes beyond the core's.
    fn memory_bytes(&self) -> u64;
}

/// One variant of the rays' pass and the deferred shading pass after it.
struct Variant {
    main: Main,
    shade: Option<wgpu::ComputePipeline>,
}

/// The rays' pipeline: compute, or a fragment shader over a full-screen triangle.
enum Main {
    Compute(wgpu::ComputePipeline),
    Fragment(wgpu::RenderPipeline),
}

/// The compiled passes: group 0's layout and bind group, the variant for frames without mesh
/// records and (with a meshes stage) the one with them (the mesh capability is compiled in
/// only there: its code cost frames without meshes 2-2.5% with `dispatch = fragment`), and the
/// lane's own entry points.
pub(crate) struct Passes {
    layout0: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    plain: Variant,
    meshed: Option<Variant>,
    extra: Vec<wgpu::ComputePipeline>,
    bind0: Option<wgpu::BindGroup>,
    /// Group 0 for `dispatch = "fragment"`: a 1x1 stand-in instead of the output texture.
    bind0_frag: Option<wgpu::BindGroup>,
    stand_in: Option<wgpu::Texture>,
}

impl Passes {
    fn new<L: Lane>(core: &Core, lane: &L) -> anyhow::Result<Self> {
        let dev = &core.gpu.device;
        let mut entries = core.layout_entries();
        entries.extend(lane.layout_entries(core.stages));
        let layout0 = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("raycast"), entries: &entries });
        let pipeline_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("raycast"), bind_group_layouts: &lane.layouts([&layout0, core.world.layout()]), immediate_size: 0 });
        let (plain, meshed, extra) = Self::compile(core, &pipeline_layout, lane)?;
        Ok(Self { layout0, pipeline_layout, plain, meshed, extra, bind0: None, bind0_frag: None, stand_in: None })
    }

    /// The variants for `lane` and its own entry points.
    fn compile<L: Lane>(core: &Core, layout: &wgpu::PipelineLayout, lane: &L) -> anyhow::Result<(Variant, Option<Variant>, Vec<wgpu::ComputePipeline>)> {
        let (plain, extra) = Self::variant(core, layout, lane, false)?;
        let meshed = if lane.meshes() { Some(Self::variant(core, layout, lane, true)?.0) } else { None };
        Ok((plain, meshed, extra))
    }

    fn variant<L: Lane>(core: &Core, layout: &wgpu::PipelineLayout, lane: &L, meshed: bool) -> anyhow::Result<(Variant, Vec<wgpu::ComputePipeline>)> {
        let dev = &core.gpu.device;
        let deferred = core.records.is_some() && !lane.inline_only();
        let lp = shader(lane.program(&core.shading, meshed), deferred, lane.inline_only(), lane.fragment());
        let deps: Vec<&str> = lp.deps.iter().map(String::as_str).collect();
        let scope = dev.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = mr_gpu::program::compile(dev, "raycast", &lp.program, &lp.root, &deps);
        let entry = |name| {
            dev.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some(name), layout: Some(layout), module: &module, entry_point: Some(name), compilation_options: Default::default(), cache: None })
        };
        let main = if lane.fragment() {
            // Deferred: the fragments write records, not colour (the output is attached
            // untouched).
            let write_mask = if deferred { wgpu::ColorWrites::empty() } else { wgpu::ColorWrites::ALL };
            let targets = [Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba8Unorm, blend: None, write_mask })];
            Main::Fragment(dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("raycast fragment"),
                layout: Some(layout),
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_full"), compilation_options: Default::default(), buffers: &[] },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some(if deferred { "fs_cast" } else { "fs_full" }), compilation_options: Default::default(), targets: &targets }),
                multiview_mask: None,
                cache: None,
            }))
        } else {
            Main::Compute(entry(if deferred { "walk_records" } else { "main" }))
        };
        let shade = deferred.then(|| entry("shade"));
        let extra = if meshed { vec![] } else { lane.entries().iter().map(|e| entry(e)).collect() };
        if let Some(e) = pollster::block_on(scope.pop()) {
            anyhow::bail!("compiling the ray-casting shader: {e}");
        }
        Ok((Variant { main, shade }, extra))
    }

    /// Drops group 0's bind group (its resources changed); made anew before the next passes.
    fn unbind(&mut self) {
        self.bind0 = None;
        self.bind0_frag = None;
    }

    fn bind<L: Lane>(&mut self, core: &Core, lane: &L) {
        let make = |out: &wgpu::Texture| {
            let view = out.create_view(&Default::default());
            let mut entries = vec![wgpu::BindGroupEntry { binding: 0, resource: core.camera_buf.as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) }];
            entries.extend(core.accum.bind_entries(STATE_BINDING));
            entries.extend(core.records.as_ref().map(|r| wgpu::BindGroupEntry { binding: RECORDS_BINDING, resource: r.as_entire_binding() }));
            entries.extend(lane.bind_entries());
            core.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("raycast"), layout: &self.layout0, entries: &entries })
        };
        if self.bind0.is_none() {
            self.bind0 = Some(make(&core.output));
        }
        // `dispatch` is live: the stand-in's bind group when first needed.
        if lane.fragment() && self.bind0_frag.is_none() {
            let stand_in = self.stand_in.get_or_insert_with(|| mr_gpu::output_texture(&core.gpu, 1, 1));
            self.bind0_frag = Some(make(stand_in));
        }
    }

    /// Group 0's bind group (after `bind`).
    fn bind0(&self) -> &wgpu::BindGroup {
        self.bind0.as_ref().expect("bound")
    }
}

/// The whole shader of a lane's program: its root, then `cast.wgsl` (`steps`: the steps view,
/// `fragment`: the fragment entry points), and when shading is `deferred` `deferred.wgsl` with
/// the common pass.
fn shader(mut lp: LaneProgram, deferred: bool, steps: bool, fragment: bool) -> LaneProgram {
    if steps {
        lp.program.define("VIEW_STEPS");
    }
    if fragment {
        lp.program.define("FRAGMENT");
    }
    lp.root = format!("{}\n{}", lp.root, include_str!("cast.wgsl"));
    if deferred {
        lp.root = format!("{}\n{}\n{}", lp.root, include_str!("deferred.wgsl"), mr_components::shading::DEFERRED_PACKED_WGSL);
    }
    lp
}

struct Raycast<L> {
    core: Core,
    passes: Passes,
    lane: L,
}

impl Raycast<()> {
    /// The executor for `backend`'s graph: the lane by the API of the primary rays' kernel
    /// (ray queries, else a walk).
    fn create(backend: &str, p: InitParams, devices: &Devices, waker: Waker) -> anyhow::Result<Box<dyn Renderer>> {
        let g = graph::of_init(backend, &p)?;
        let kernel = g.input(Role::Visibility, "primary").ok_or_else(|| anyhow::anyhow!("{backend}: the primary rays are not bound"))?;
        Ok(match g.api(kernel) {
            Some(Api::RayQuery) => Box::new(Raycast::new(rayquery::RayQuery::new(&p, devices, waker, &g)?)?),
            // A walk: compute threads or a fragment shader (`dispatch`), the kernel compiled in.
            _ => Box::new(Raycast::new(walk::Walk::new(&p, devices, waker, &g)?)?),
        })
    }
}

impl<L: Lane> Raycast<L> {
    fn new((core, lane): (Core, L)) -> anyhow::Result<Self> {
        let passes = Passes::new(&core, &lane)?;
        Ok(Self { core, passes, lane })
    }
}

impl<L: Lane> Renderer for Raycast<L> {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        if !updates.is_empty() {
            self.core.accum.invalidate();
        }
        let changed = self.core.world.apply(&self.core.gpu, updates);
        self.lane.updated(&self.core, &changed)
    }

    fn update_meshes(&mut self, update: MeshUpdate) -> anyhow::Result<()> {
        self.lane.update_meshes(update);
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let (core, passes, lane) = (&mut self.core, &mut self.passes, &mut self.lane);
        if lane.stale() || core.shading != core.built_shading {
            (passes.plain, passes.meshed, passes.extra) = Passes::compile(core, &passes.pipeline_layout, lane)?;
            if core.accum.set_shading(&core.gpu, &core.shading) {
                passes.unbind();
            }
            core.built_shading = core.shading.clone();
            lane.compiled();
        }
        let ticket = core.ring.begin(&core.gpu, sink)?;
        let start = Instant::now();
        let c = &req.camera;
        let mut enc = core.gpu.device.create_command_encoder(&Default::default());
        lane.prepare(core, passes, &mut enc, ticket, req);
        core.accum.begin(&core.gpu, c, req.mode);
        let (view_max, ray_flags) = lane.camera();
        core.write_camera(c, req.mode, view_max, ray_flags);
        let world_bg = core.world.prepare(&core.gpu, c).clone();
        passes.bind(core, lane);
        let before = lane.before(core, passes, &mut enc, ticket, req, &world_bg);
        let variant = if before.meshed { passes.meshed.as_ref().unwrap_or(&passes.plain) } else { &passes.plain };
        // The rays' pass ends the frame unless the deferred shading pass follows.
        let last = variant.shade.is_none();
        if last {
            core.ring.stage(&mut enc, ticket, "cast+shade", &core.stage_nodes[0]);
        } else {
            core.ring.stage(&mut enc, ticket, "cast", &core.stage_nodes[1]);
        }
        let (w, h) = (core.width, core.height);
        match &variant.main {
            Main::Compute(pipeline) => {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("raycast"), timestamp_writes: core.ring.compute_timestamps(ticket, before.first, last) });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, passes.bind0(), &[]);
                pass.set_bind_group(1, &world_bg, &[]);
                for (i, b) in lane.groups() {
                    pass.set_bind_group(i, b, &[]);
                }
                pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
            }
            Main::Fragment(pipeline) => {
                let view = core.output.create_view(&Default::default());
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("raycast fragment"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                    depth_stencil_attachment: None,
                    timestamp_writes: core.ring.render_timestamps(ticket, before.first, last),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(pipeline);
                // The output is the colour attachment here, so group 0 binds a stand-in for
                // the storage-texture output.
                pass.set_bind_group(0, passes.bind0_frag.as_ref().expect("bound"), &[]);
                pass.set_bind_group(1, &world_bg, &[]);
                for (i, b) in lane.groups() {
                    pass.set_bind_group(i, b, &[]);
                }
                pass.draw(0..3, 0..1);
            }
        }
        if let Some(shade) = &variant.shade {
            core.ring.stage(&mut enc, ticket, "shade", &core.stage_nodes[2]);
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("deferred shade"), timestamp_writes: core.ring.compute_timestamps(ticket, false, true) });
            pass.set_pipeline(shade);
            pass.set_bind_group(0, passes.bind0(), &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            for (i, b) in lane.groups() {
                pass.set_bind_group(i, b, &[]);
            }
            pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        }
        let (mut counters, legend, stages) = lane.stats(core);
        counters.extend(core.accum.counter(req.mode));
        let stats = FrameStats { cpu_ms: start.elapsed().as_secs_f32() * 1e3, gpu_ms: None, counters, legend, stages };
        lane.finish(&mut enc);
        core.ring.submit(&core.gpu, enc, ticket, &core.output, req.frame_id, req.readback, stats);
        lane.submitted();
        core.ring.poll(&core.gpu, sink, false)
    }

    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.core.ring.poll(&self.core.gpu, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        let core = &mut self.core;
        match name {
            "shadows" => core.world.cfg.shadows.set(value)?,
            "max_steps" => core.world.cfg.max_steps = value.as_i64().unwrap_or(2048) as u32,
            "shading" => core.shading = mr_components::require(L::BACKEND, mr_components::Slot::Shading, value.as_str().unwrap_or(""))?,
            _ => match self.lane.set_param(name, value) {
                Some(r) => r?,
                None if core.accum.set_param(name, value).is_some() => {}
                None => anyhow::bail!("`{name}` cannot change while running"),
            },
        }
        Ok(())
    }

    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.core.ring.poll(&self.core.gpu, sink, true)
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.core.resize(width, height);
        self.lane.resize(&self.core);
        self.passes.unbind();
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        MemoryReport { cpu_bytes: 0, gpu_bytes: self.core.memory_bytes() + self.lane.memory_bytes() }
    }
}

#[cfg(test)]
mod tests {
    /// A lane's program as the passes compile it (`shader`), composed and validated (no GPU
    /// needed).
    pub(crate) fn validate(lp: crate::LaneProgram, deferred: bool, steps: bool, fragment: bool, what: &str) {
        use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        let lp = crate::shader(lp, deferred, steps, fragment);
        let deps: Vec<&str> = lp.deps.iter().map(String::as_str).collect();
        let m = lp.program.naga(&lp.root, &deps).unwrap_or_else(|e| panic!("{what}: {e}"));
        Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("{what}: {e:?}"));
    }
}
