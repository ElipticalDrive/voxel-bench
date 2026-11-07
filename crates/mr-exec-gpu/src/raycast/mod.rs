//! Ray casting: every pipeline whose image is one primary
//! ray per pixel, shaded inline or (`placement = deferred`) through hit records and the common
//! shading pass. One region for the two ways the visibility node's primary rays are cast, its
//! kernel's API:
//! - `raycast-gpu` (`compute`, [`walk`]): a walk, the traversal kernel over a storage component
//!   (any storage, any kernel, missing capabilities from adapters), with an optional prepass,
//!   meshes' records (mesh mode), LOD and the steps view; pixels in compute threads or a
//!   fragment shader;
//! - `rayquery-gpu` (`rayquery`, [`rayquery`]): hardware ray queries over acceleration structures
//!   built from the geometry component (triangles or brick boxes).
//!
//! Which caster runs is the graph's: the primary rays' kernel node's API. The nodes run on the
//! node executor ([`crate::exec`]); what they share is the region's ([`Raycast`], [`Passes`])
//! over the executor's core: the shader (the caster's root and modules, then `view.wgsl` and
//! `cast.wgsl`, and when deferred `records.wgsl` and `deferred.wgsl` with the common pass),
//! the rays' pass and the deferred shading pass, or (the shading on another GPU) the records
//! sent to it ([`crate::remote`]).

use glam::IVec3;
use mr_components::Api;
use mr_components::graph::{self, Role};
use mr_core::*;
use mr_gpu::{Ticket, wgpu};

use crate::remote::Far;
use crate::Core;
use crate::exec::State;

pub(crate) mod rayquery;
pub(crate) mod walk;

/// The ray-casting nodes of graph `g` on the node executor: the caster by the API of the
/// primary rays' kernel (ray queries, else a walk).
pub(crate) fn create(backend: &str, p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked) -> anyhow::Result<Box<dyn Renderer>> {
    let kernel = g.input(Role::Visibility, "primary").ok_or_else(|| anyhow::anyhow!("{backend}: the primary rays are not bound"))?;
    match g.api(kernel) {
        Some(Api::RayQuery) => run(Raycast::new(rayquery::RayQuery::new(p, devices, waker, g)?)?, g),
        // A walk: compute threads or a fragment shader (`dispatch`), the kernel compiled in.
        _ => run(Raycast::new(walk::Walk::new(p, devices, waker, g)?)?, g),
    }
}

fn run<C: Cast + 'static>((core, region): (Core, Raycast<C>), g: &graph::Checked) -> anyhow::Result<Box<dyn Renderer>> {
    let passes = nodes::of::<C>(g)?;
    tracing::debug!("{}: {} passes", C::BACKEND, passes.len());
    crate::exec::run(core, region, passes)
}

/// The site `port` of the shading's rays, unless the shading runs on another GPU (then its
/// rays are the remote part's, [`Far`], and this GPU's program walks the primary rays' kernel
/// there, never called).
fn shading_site<'a>(g: &'a graph::Checked, port: &str) -> Option<Option<graph::Site<'a>>> {
    match crate::remote::far_slot(g) {
        Some(_) => None,
        None => Some(g.site(Role::Shading, port)),
    }
}

/// A caster's shader: its modules, its root (bindings from `REGION_BINDING`, `pixel_hit` and
/// what it needs) and the root's dependencies.
struct CastProgram {
    program: mr_gpu::program::Program,
    root: String,
    deps: Vec<String>,
}

/// What of a frame's work before the rays has run: each part once, in this order (accel,
/// storages, meshes, prepass), from its node's pass or, when the graph has no pass for it (a
/// live knob added it), from the rays'.
#[derive(Default)]
struct Steps {
    accel: bool,
    stored: bool,
    meshed: Option<bool>,
    prepassed: bool,
    /// A pass began the frame's clock.
    started: bool,
}

/// What a caster's pixels show (`view`): the image, or a diagnostic on the shared colour
/// scale up to `view_max`, compiled in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum View {
    Image,
    /// The primary walk's loop iterations (log scale).
    Steps,
    /// The primary hit's distance in blocks (linear; sky above the top).
    Distance,
}

impl View {
    pub(crate) fn parse(s: &str) -> anyhow::Result<Self> {
        match s {
            "image" => Ok(View::Image),
            "steps" => Ok(View::Steps),
            "distance" => Ok(View::Distance),
            _ => anyhow::bail!("view must be `image`, `steps` or `distance`, got `{s}`"),
        }
    }

    fn define(self) -> Option<&'static str> {
        match self {
            View::Image => None,
            View::Steps => Some("VIEW_STEPS"),
            View::Distance => Some("VIEW_DISTANCE"),
        }
    }
}

/// How the primary rays are cast: a walk over storages or ray queries over acceleration
/// structures, with what each keeps.
trait Cast: Sized {
    /// The registry's name of this configuration.
    const BACKEND: &'static str;
    /// The ray flags of ray queries that are not shadow rays (a WGSL expression; see
    /// `Core::trace_root`).
    const RT_FLAGS: &'static str = "0u";
    /// Group 0's entries from `REGION_BINDING`.
    fn layout_entries(&self, stages: wgpu::ShaderStages) -> Vec<wgpu::BindGroupLayoutEntry>;
    /// Their resources (stale after the region's group 0 is dropped: see `Passes::unbind`).
    fn bind_entries(&self) -> Vec<wgpu::BindGroupEntry<'_>>;
    /// The bind group layouts after group 0 and the world's (group 1).
    fn layouts<'a>(&'a self, core: &'a Core, base: [&'a wgpu::BindGroupLayout; 2]) -> Vec<Option<&'a wgpu::BindGroupLayout>>;
    /// The shader with `shading`; `meshed`: the variant for frames with mesh records.
    fn program(&self, shading: &str, meshed: bool) -> CastProgram;
    /// The caster's knobs changed what its shader compiles (cleared by `compiled`).
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
    /// What the pixels show; a diagnostic view shades inline whatever the placement.
    fn view(&self) -> View {
        View::Image
    }
    /// The caster's own compute entry points in the module (the prepass's).
    fn entries(&self) -> &'static [&'static str] {
        &[]
    }
    /// The camera's region words: `view_max` and `ray_flags`.
    fn camera(&self) -> [u32; 2];
    /// World changes the GPU world applied (`changed`: its chunks).
    fn updated(&mut self, core: &Core, changed: &[IVec3]) -> anyhow::Result<()>;
    fn update_meshes(&mut self, _update: MeshUpdate) {}
    /// This frame's storage bind groups (before any pass that walks them).
    fn storages(&mut self, core: &mut Core);
    /// Rasterises the meshes' records (mesh mode) into `enc`, the frame's first pass if
    /// `first`; whether they ran (frames with them run the meshed variant).
    fn encode_meshes(&mut self, _core: &mut Core, _enc: &mut wgpu::CommandEncoder, _ticket: Ticket, _req: &FrameRequest, _first: bool) -> bool {
        false
    }
    /// The prepass, with group 0 and the world bound; the frame's first pass if `first`.
    /// Whether it ran.
    fn prepass(&mut self, _core: &mut Core, _passes: &Passes, _enc: &mut wgpu::CommandEncoder, _ticket: Ticket, _world: &wgpu::BindGroup, _first: bool) -> bool {
        false
    }
    /// The caster's bind groups after group 1.
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
/// caster's own entry points.
pub(crate) struct Passes {
    layout0: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    /// The records are shaded on another GPU (no shading pass here).
    remote: bool,
    plain: Variant,
    meshed: Option<Variant>,
    extra: Vec<wgpu::ComputePipeline>,
    bind0: Option<wgpu::BindGroup>,
    /// Group 0 for `dispatch = "fragment"`: a 1x1 stand-in instead of the output texture.
    bind0_frag: Option<wgpu::BindGroup>,
    stand_in: Option<wgpu::Texture>,
}

impl Passes {
    fn new<C: Cast>(core: &Core, cast: &C, shading: &str, remote: bool) -> anyhow::Result<Self> {
        let dev = &core.gpu.device;
        let mut entries = core.layout_entries();
        entries.extend(cast.layout_entries(core.stages));
        let layout0 = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("raycast"), entries: &entries });
        let pipeline_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("raycast"), bind_group_layouts: &cast.layouts(core, [&layout0, core.world.layout()]), immediate_size: 0 });
        let (plain, meshed, extra) = Self::compile(core, &pipeline_layout, cast, shading, remote)?;
        Ok(Self { layout0, pipeline_layout, remote, plain, meshed, extra, bind0: None, bind0_frag: None, stand_in: None })
    }

    /// The variants for `cast` and its own entry points.
    fn compile<C: Cast>(core: &Core, layout: &wgpu::PipelineLayout, cast: &C, shading: &str, remote: bool) -> anyhow::Result<(Variant, Option<Variant>, Vec<wgpu::ComputePipeline>)> {
        let (plain, extra) = Self::variant(core, layout, cast, shading, false, remote)?;
        let meshed = if cast.meshes() { Some(Self::variant(core, layout, cast, shading, true, remote)?.0) } else { None };
        Ok((plain, meshed, extra))
    }

    fn variant<C: Cast>(core: &Core, layout: &wgpu::PipelineLayout, cast: &C, shading: &str, meshed: bool, remote: bool) -> anyhow::Result<(Variant, Vec<wgpu::ComputePipeline>)> {
        let dev = &core.gpu.device;
        let deferred = core.records.is_some() && cast.view() == View::Image;
        let mut lp = cast.program(shading, meshed);
        core.trace_root(&mut lp.program, C::RT_FLAGS);
        let lp = shader(lp, deferred, cast.view(), cast.fragment());
        let deps: Vec<&str> = lp.deps.iter().map(String::as_str).collect();
        let scope = dev.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = mr_gpu::program::compile(dev, "raycast", &lp.program, &lp.root, &deps);
        let entry = |name| {
            dev.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some(name), layout: Some(layout), module: &module, entry_point: Some(name), compilation_options: Default::default(), cache: core.gpu.cache() })
        };
        let main = if cast.fragment() {
            // Deferred: the fragments write records, not colour (the output is attached
            // untouched).
            let write_mask = if deferred { wgpu::ColorWrites::empty() } else { wgpu::ColorWrites::ALL };
            let targets = [Some(wgpu::ColorTargetState { format: core.output_format(), blend: None, write_mask })];
            Main::Fragment(dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("raycast fragment"),
                layout: Some(layout),
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_full"), compilation_options: Default::default(), buffers: &[] },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some(if deferred { "fs_cast" } else { "fs_full" }), compilation_options: Default::default(), targets: &targets }),
                multiview_mask: None,
                cache: core.gpu.cache(),
            }))
        } else {
            Main::Compute(entry(if deferred { "walk_records" } else { "main" }))
        };
        let shade = (deferred && !remote).then(|| entry("shade"));
        let extra = if meshed { vec![] } else { cast.entries().iter().map(|e| entry(e)).collect() };
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

    fn bind<C: Cast>(&mut self, core: &Core, cast: &C) {
        let make = |out: &wgpu::Texture| {
            let view = out.create_view(&Default::default());
            let mut entries = core.bind_entries(&view);
            entries.extend(cast.bind_entries());
            core.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("raycast"), layout: &self.layout0, entries: &entries })
        };
        if self.bind0.is_none() {
            self.bind0 = Some(make(&core.output));
        }
        // `dispatch` is live: the stand-in's bind group when first needed.
        if cast.fragment() && self.bind0_frag.is_none() {
            let stand_in = self.stand_in.get_or_insert_with(|| mr_gpu::output_texture(&core.gpu, 1, 1, core.tone.format()));
            self.bind0_frag = Some(make(stand_in));
        }
    }

    /// Group 0's bind group (after `bind`).
    fn bind0(&self) -> &wgpu::BindGroup {
        self.bind0.as_ref().expect("bound")
    }
}

/// The whole shader of a caster's program: its root, then `view.wgsl` and `cast.wgsl`
/// (`view`: a diagnostic view, `fragment`: the fragment entry points), and when shading is
/// `deferred` `records.wgsl` and `deferred.wgsl` with the common pass.
fn shader(mut lp: CastProgram, deferred: bool, view: View, fragment: bool) -> CastProgram {
    if let Some(d) = view.define() {
        lp.program.define(d);
    }
    if fragment {
        lp.program.define("FRAGMENT");
    }
    lp.root = format!("{}\n{}\n{}", lp.root, include_str!("view.wgsl"), include_str!("cast.wgsl"));
    if deferred {
        lp.root = format!("{}\n{}\n{}\n{}", lp.root, include_str!("records.wgsl"), include_str!("deferred.wgsl"), mr_components::shading::DEFERRED_PACKED_WGSL);
    }
    lp
}

/// The region the ray-casting nodes share ([`nodes`]): the caster, its passes (one program,
/// group 0) and the shading they were compiled with, the frame's state; the shading's GPU
/// when it is another.
struct Raycast<C> {
    passes: Passes,
    cast: C,
    far: Option<Far>,
    shading: String,
    /// Shading the passes were compiled with (recompiled at the next frame on a change).
    built_shading: String,
    steps: Steps,
    /// This frame's world bind group.
    world: Option<wgpu::BindGroup>,
}

impl<C: Cast> Raycast<C> {
    fn new((core, cast, shading, far): (Core, C, String, Option<Far>)) -> anyhow::Result<(Core, Self)> {
        anyhow::ensure!(far.is_none() || cast.view() == View::Image, "{}: a diagnostic view shades inline, not on the shading's GPU", C::BACKEND);
        let passes = Passes::new(&core, &cast, &shading, far.is_some())?;
        Ok((core, Self { passes, cast, far, built_shading: shading.clone(), shading, steps: Steps::default(), world: None }))
    }

    /// The variant this frame runs: the meshed one when the meshes' records ran.
    fn variant(&self) -> &Variant {
        match (self.steps.meshed, &self.passes.meshed) {
            (Some(true), Some(m)) => m,
            _ => &self.passes.plain,
        }
    }

    /// The frame's acceleration structures (group 0 is bound anew if they changed it).
    fn accel(&mut self, core: &mut Core, enc: &mut wgpu::CommandEncoder, ticket: Ticket, req: &FrameRequest) {
        if !std::mem::replace(&mut self.steps.accel, true) && core.build_accel(enc, ticket, req) {
            self.passes.unbind();
        }
    }

    /// The storages' bind groups.
    fn store(&mut self, core: &mut Core, enc: &mut wgpu::CommandEncoder, ticket: Ticket, req: &FrameRequest) {
        self.accel(core, enc, ticket, req);
        if !std::mem::replace(&mut self.steps.stored, true) {
            self.cast.storages(core);
        }
    }

    /// The meshes' records.
    fn mesh(&mut self, core: &mut Core, enc: &mut wgpu::CommandEncoder, ticket: Ticket, req: &FrameRequest) {
        self.store(core, enc, ticket, req);
        if self.steps.meshed.is_none() {
            let ran = self.cast.encode_meshes(core, enc, ticket, req, !self.steps.started);
            self.steps.started |= ran;
            self.steps.meshed = Some(ran);
        }
    }

    fn prepass(&mut self, core: &mut Core, enc: &mut wgpu::CommandEncoder, ticket: Ticket, req: &FrameRequest) {
        self.mesh(core, enc, ticket, req);
        if !std::mem::replace(&mut self.steps.prepassed, true) {
            self.passes.bind(core, &self.cast);
            let ran = self.cast.prepass(core, &self.passes, enc, ticket, self.world.as_ref().expect("set up"), !self.steps.started);
            self.steps.started |= ran;
        }
    }

    /// The rays' pass (after the work before it, if its nodes had no pass).
    fn rays(&mut self, core: &mut Core, enc: &mut wgpu::CommandEncoder, ticket: Ticket, req: &FrameRequest) {
        self.prepass(core, enc, ticket, req);
        let (passes, cast, world) = (&self.passes, &self.cast, self.world.as_ref().expect("set up"));
        let variant = self.variant();
        let first = !self.steps.started;
        // The rays' pass ends the frame unless the deferred shading pass follows.
        let last = variant.shade.is_none();
        if last && !passes.remote {
            core.ring.stage(enc, ticket, "cast+shade", &core.stage_nodes[0]);
        } else {
            core.ring.stage(enc, ticket, "cast", &core.stage_nodes[1]);
        }
        let (w, h) = (core.width, core.height);
        match &variant.main {
            Main::Compute(pipeline) => {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("raycast"), timestamp_writes: core.ring.compute_timestamps(ticket, first, last) });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, passes.bind0(), &[]);
                pass.set_bind_group(1, world, &[]);
                for (i, b) in cast.groups() {
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
                    timestamp_writes: core.ring.render_timestamps(ticket, first, last),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(pipeline);
                // The output is the colour attachment here, so group 0 binds a stand-in for
                // the storage-texture output.
                pass.set_bind_group(0, passes.bind0_frag.as_ref().expect("bound"), &[]);
                pass.set_bind_group(1, world, &[]);
                for (i, b) in cast.groups() {
                    pass.set_bind_group(i, b, &[]);
                }
                pass.draw(0..3, 0..1);
            }
        }
        self.steps.started = true;
    }

    /// The deferred shading pass, if this frame's variant has one.
    fn shade(&self, core: &mut Core, enc: &mut wgpu::CommandEncoder, ticket: Ticket) {
        let Some(shade) = &self.variant().shade else { return };
        core.ring.stage(enc, ticket, "shade", &core.stage_nodes[2]);
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("deferred shade"), timestamp_writes: core.ring.compute_timestamps(ticket, false, true) });
        pass.set_pipeline(shade);
        pass.set_bind_group(0, self.passes.bind0(), &[]);
        pass.set_bind_group(1, self.world.as_ref().expect("set up"), &[]);
        for (i, b) in self.cast.groups() {
            pass.set_bind_group(i, b, &[]);
        }
        pass.dispatch_workgroups(core.width.div_ceil(8), core.height.div_ceil(8), 1);
    }
}

impl<C: Cast> crate::exec::Region for Raycast<C> {
    fn compile(&mut self, core: &mut Core, _req: &FrameRequest) -> anyhow::Result<()> {
        let (passes, cast) = (&mut self.passes, &mut self.cast);
        if cast.stale() || self.shading != self.built_shading {
            (passes.plain, passes.meshed, passes.extra) = Passes::compile(core, &passes.pipeline_layout, cast, &self.shading, passes.remote)?;
            if core.accum.set_shading(&core.gpu, &self.shading) {
                passes.unbind();
            }
            self.built_shading = self.shading.clone();
            cast.compiled();
        }
        Ok(())
    }

    fn setup(&mut self, core: &mut Core, req: &FrameRequest) {
        self.steps = Steps::default();
        core.accum.begin(&core.gpu, &req.camera, req.mode);
        core.write_camera(&req.camera, req.mode, self.cast.camera());
        self.world = Some(core.world.prepare(&core.gpu, &req.camera).clone());
    }

    fn stats(&self, core: &Core, req: &FrameRequest) -> (Vec<(String, f64)>, Option<Legend>, Vec<StageTime>) {
        let (mut counters, legend, mut stages) = self.cast.stats(core);
        let (accel, accel_stages) = core.accel_stats();
        counters.extend(accel);
        stages.extend(accel_stages);
        counters.extend(core.accum.counter(req.mode));
        (counters, legend, stages)
    }

    fn finish(&mut self, _: &Core, enc: &mut wgpu::CommandEncoder, _: Ticket) {
        self.cast.finish(enc);
    }

    /// With the shading on another GPU, this GPU's frames are the first half of one (their
    /// stats go with the records).
    fn begin(&mut self, core: &mut Core, sink: &mut dyn FrameSink) -> anyhow::Result<Ticket> {
        match &mut self.far {
            Some(f) => core.ring.begin(&core.gpu, f.near_sink()),
            None => core.ring.begin(&core.gpu, sink),
        }
    }

    /// The records go to the shading's GPU, which shades them and completes the frame.
    fn submit(&mut self, core: &mut Core, enc: wgpu::CommandEncoder, ticket: Ticket, req: &FrameRequest, stats: FrameStats, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        match &mut self.far {
            Some(f) => {
                f.send(core, enc, ticket, req, stats);
                self.cast.submitted();
                f.render(core, req, &self.shading, sink)
            }
            None => {
                core.ring.submit(&core.gpu, enc, ticket, &core.output, req.frame_id, req.readback, stats);
                self.cast.submitted();
                core.ring.poll(&core.gpu, sink, false)
            }
        }
    }
}

impl<C: Cast> State for Raycast<C> {
    fn updating(&mut self, updates: &[WorldUpdate]) {
        if let Some(f) = &mut self.far
            && let Err(e) = f.update(updates) {
            tracing::error!("the shading's GPU: {e}");
        }
    }

    fn updated(&mut self, core: &Core, changed: &[IVec3]) -> anyhow::Result<()> {
        self.cast.updated(core, changed)
    }

    fn update_meshes(&mut self, update: MeshUpdate) {
        self.cast.update_meshes(update);
    }

    fn set_param(&mut self, _core: &mut Core, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        if name == "shading" {
            return Some(mr_components::require(C::BACKEND, mr_components::Slot::Shading, value.as_str().unwrap_or("")).map(|s| self.shading = s));
        }
        // The core's knobs (shadows, the shading state's) reach both GPUs' cores.
        if let Some(Err(e)) = self.far.as_mut().and_then(|f| f.set_param(name, value)) {
            return Some(Err(e));
        }
        self.cast.set_param(name, value)
    }

    fn resize(&mut self, core: &Core) {
        self.cast.resize(core);
        self.passes.unbind();
        if let Some(Err(e)) = self.far.as_mut().map(|f| f.resize(core)) {
            tracing::error!("the shading's GPU: {e}");
        }
    }

    fn memory(&self, _core: &Core) -> (u64, u64) {
        (0, self.cast.memory_bytes() + self.far.as_ref().map_or(0, |f| f.memory_bytes()))
    }

    fn poll(&mut self, core: &mut Core, sink: &mut dyn FrameSink, wait: bool) -> anyhow::Result<()> {
        match &mut self.far {
            Some(f) => f.poll(core, sink, wait),
            None => core.ring.poll(&core.gpu, sink, wait),
        }
    }
}

/// The ray-casting nodes on the node executor ([`crate::exec`]), one per pass of the graph's
/// schedule on a GPU (the shading's on another GPU: its pass here records nothing, the
/// region's submission sends the records there); they share [`Raycast`] as their region.
mod nodes {
    use mr_components::graph::{self, Role};

    use super::{Cast, Raycast};
    use crate::exec::{Cx, Node};

    /// A ray-query kernel: the acceleration structures, built from its geometry.
    struct Accel;
    struct Storage;
    struct Meshes;
    struct Prepass;
    struct Visibility;
    /// The deferred shading pass (with `placement = inline` fused into the rays').
    struct Shading;

    impl<C: Cast> Node<Raycast<C>> for Accel {
        fn run(&mut self, cx: &mut Cx<Raycast<C>>) {
            cx.region.accel(cx.core, cx.enc, cx.ticket, cx.req);
        }
    }

    impl<C: Cast> Node<Raycast<C>> for Storage {
        fn run(&mut self, cx: &mut Cx<Raycast<C>>) {
            cx.region.store(cx.core, cx.enc, cx.ticket, cx.req);
        }
    }

    impl<C: Cast> Node<Raycast<C>> for Meshes {
        fn run(&mut self, cx: &mut Cx<Raycast<C>>) {
            cx.region.mesh(cx.core, cx.enc, cx.ticket, cx.req);
        }
    }

    impl<C: Cast> Node<Raycast<C>> for Prepass {
        fn run(&mut self, cx: &mut Cx<Raycast<C>>) {
            cx.region.prepass(cx.core, cx.enc, cx.ticket, cx.req);
        }
    }

    impl<C: Cast> Node<Raycast<C>> for Visibility {
        fn run(&mut self, cx: &mut Cx<Raycast<C>>) {
            cx.region.rays(cx.core, cx.enc, cx.ticket, cx.req);
        }
    }

    impl<C: Cast> Node<Raycast<C>> for Shading {
        fn run(&mut self, cx: &mut Cx<Raycast<C>>) {
            cx.region.shade(cx.core, cx.enc, cx.ticket);
        }
    }

    /// The nodes of `g`'s passes on the GPU, in frame order.
    pub(super) fn of<C: Cast + 'static>(g: &graph::Checked) -> anyhow::Result<Vec<Box<dyn Node<Raycast<C>>>>> {
        crate::exec::passes(g, |role| -> Option<Box<dyn Node<Raycast<C>>>> {
            Some(match role {
                Role::Traversal => Box::new(Accel),
                Role::Storage => Box::new(Storage),
                Role::Meshes => Box::new(Meshes),
                Role::Prepass => Box::new(Prepass),
                Role::Visibility => Box::new(Visibility),
                Role::Shading => Box::new(Shading),
                _ => return None,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use mr_components::graph;

    /// The valid graphs one or two edits from ray casting's own: `accept` takes exactly those
    /// the casters build; what they have no code for is refused with the reason (a draw node,
    /// meshes drawn into the depth buffer, a second geometry, a prepass over another storage
    /// than the primary rays', a visibility node feeding a ray-query kernel's geometry).
    #[test]
    fn casters_build_what_they_declare() {
        let (mut built, mut refused) = (0, 0);
        for ck in graph::near(&[super::walk::BACKEND, super::rayquery::BACKEND]) {
            let e = ck.preset.unwrap();
            let key = ck.graph.canonical().unwrap();
            match (ck.built(), ck.graph.accept()) {
                (Ok(()), Ok(_)) => built += 1,
                (Err(_), Err(_)) => refused += 1,
                (Ok(()), Err(why)) => panic!("`{e}` builds a graph `accept` refuses ({why}): {key}"),
                (Err(why), Ok(_)) => panic!("`{e}` refuses a graph `accept` takes ({why}): {key}"),
            }
        }
        assert!(built > 0 && refused > 0, "{built} built, {refused} refused");
    }

    /// A caster's program as the passes compile it (`shader`), composed and validated (no GPU
    /// needed).
    pub(crate) fn validate(lp: super::CastProgram, deferred: bool, view: super::View, fragment: bool, what: &str) {
        use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        let lp = super::shader(lp, deferred, view, fragment);
        let deps: Vec<&str> = lp.deps.iter().map(String::as_str).collect();
        let m = lp.program.naga(&lp.root, &deps).unwrap_or_else(|e| panic!("{what}: {e}"));
        Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("{what}: {e:?}"));
    }
}
