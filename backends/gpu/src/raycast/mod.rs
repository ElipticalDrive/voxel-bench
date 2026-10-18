//! The ray-casting lane (PLAN steps 13e2, 13e3): every pipeline whose image is one primary
//! ray per pixel, shaded inline or (`placement = deferred`) through hit records and the common
//! shading pass. One lane for the two ways the visibility node's primary rays are cast, its
//! kernel's API:
//! - `gpu-compute` (`compute`, [`walk`]): a walk, the traversal kernel over a storage component
//!   (any storage, any kernel, missing capabilities from adapters), with an optional prepass,
//!   meshes' records (mesh mode), LOD and the steps view; pixels in compute threads or a
//!   fragment shader;
//! - `gpu-rt` (`rayquery`, [`rayquery`]): hardware ray queries over acceleration structures
//!   built from the geometry component (triangles or brick boxes).
//!
//! Which caster runs is the graph's: the primary rays' kernel node's API. The rest is the
//! lane's ([`Raycast`], [`Passes`]) over the executor's core: the shader (the caster's root
//! and modules, then `view.wgsl` and `cast.wgsl`, and when deferred `records.wgsl` and
//! `deferred.wgsl` with the common pass), the rays' pass and the deferred shading pass, or
//! (the shading on another GPU, PLAN step 13e4c) the records sent to it ([`crate::remote`]).

use std::time::Instant;

use glam::IVec3;
use mr_components::Api;
use mr_components::graph::{self, Role};
use mr_core::*;
use mr_gpu::{Ticket, wgpu};

use crate::remote::Far;
use crate::{Core, Lane};

pub(crate) mod rayquery;
pub(crate) mod walk;

/// The lane for graph `g`: the caster by the API of the primary rays' kernel (ray queries,
/// else a walk).
pub(crate) fn create(backend: &str, p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked) -> anyhow::Result<Box<dyn Renderer>> {
    let kernel = g.input(Role::Visibility, "primary").ok_or_else(|| anyhow::anyhow!("{backend}: the primary rays are not bound"))?;
    match g.api(kernel) {
        Some(Api::RayQuery) => crate::run(Raycast::new(rayquery::RayQuery::new(p, devices, waker, g)?)?),
        // A walk: compute threads or a fragment shader (`dispatch`), the kernel compiled in.
        _ => crate::run(Raycast::new(walk::Walk::new(p, devices, waker, g)?)?),
    }
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

/// A caster's shader: its modules, its root (bindings from `LANE_BINDING`, `pixel_hit` and
/// what it needs) and the root's dependencies.
struct CastProgram {
    program: mr_gpu::program::Program,
    root: String,
    deps: Vec<String>,
}

/// What the caster's work before the rays left for the rays' pass.
struct Before {
    /// The rays' pass starts the frame's clock (no earlier pass or encoder timestamp did).
    first: bool,
    /// This frame has mesh records (the meshed passes run).
    meshed: bool,
}

/// How the primary rays are cast: a walk over storages or ray queries over acceleration
/// structures, with what each keeps.
trait Cast: Sized {
    /// The registry's name of this configuration.
    const BACKEND: &'static str;
    /// The ray flags of ray queries that are not shadow rays (a WGSL expression; see
    /// `Core::trace_root`).
    const RT_FLAGS: &'static str = "0u";
    /// Group 0's entries from `LANE_BINDING`.
    fn layout_entries(&self, stages: wgpu::ShaderStages) -> Vec<wgpu::BindGroupLayoutEntry>;
    /// Their resources (stale after the lane's group 0 is dropped: see `Passes::unbind`).
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
    /// Shading runs inline whatever the placement (the steps view shows the walk itself).
    fn inline_only(&self) -> bool {
        false
    }
    /// The caster's own compute entry points in the module (the prepass's).
    fn entries(&self) -> &'static [&'static str] {
        &[]
    }
    /// The camera's lane words: `view_max` and `ray_flags`.
    fn camera(&self) -> [u32; 2];
    /// World changes the GPU world applied (`changed`: its chunks).
    fn updated(&mut self, core: &Core, changed: &[IVec3]) -> anyhow::Result<()>;
    fn update_meshes(&mut self, _update: MeshUpdate) {}
    /// Work before the camera is written (acceleration structures); may drop group 0's
    /// bind group (`passes.unbind()`).
    fn prepare(&mut self, core: &mut Core, passes: &mut Passes, enc: &mut wgpu::CommandEncoder, ticket: Ticket, req: &FrameRequest);
    /// Passes before the rays (meshes, the prepass), with group 0 and the world bound.
    fn before(&mut self, core: &mut Core, passes: &Passes, enc: &mut wgpu::CommandEncoder, ticket: Ticket, req: &FrameRequest, world: &wgpu::BindGroup) -> Before;
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
        let deferred = core.records.is_some() && !cast.inline_only();
        let mut lp = cast.program(shading, meshed);
        core.trace_root(&mut lp.program, C::RT_FLAGS);
        let lp = shader(lp, deferred, cast.inline_only(), cast.fragment());
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
            let stand_in = self.stand_in.get_or_insert_with(|| mr_gpu::output_texture(&core.gpu, 1, 1));
            self.bind0_frag = Some(make(stand_in));
        }
    }

    /// Group 0's bind group (after `bind`).
    fn bind0(&self) -> &wgpu::BindGroup {
        self.bind0.as_ref().expect("bound")
    }
}

/// The whole shader of a caster's program: its root, then `view.wgsl` and `cast.wgsl`
/// (`steps`: the steps view, `fragment`: the fragment entry points), and when shading is
/// `deferred` `records.wgsl` and `deferred.wgsl` with the common pass.
fn shader(mut lp: CastProgram, deferred: bool, steps: bool, fragment: bool) -> CastProgram {
    if steps {
        lp.program.define("VIEW_STEPS");
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

/// The lane: the caster, its passes and the shading they were compiled with; the shading's
/// GPU when it is another.
struct Raycast<C> {
    passes: Passes,
    cast: C,
    far: Option<Far>,
    shading: String,
    /// Shading the passes were compiled with (recompiled at the next frame on a change).
    built_shading: String,
}

impl<C: Cast> Raycast<C> {
    fn new((core, cast, shading, far): (Core, C, String, Option<Far>)) -> anyhow::Result<(Core, Self)> {
        anyhow::ensure!(far.is_none() || !cast.inline_only(), "{}: view = steps shades inline, not on the shading's GPU", C::BACKEND);
        let passes = Passes::new(&core, &cast, &shading, far.is_some())?;
        Ok((core, Self { passes, cast, far, built_shading: shading.clone(), shading }))
    }
}

impl<C: Cast> Lane for Raycast<C> {
    fn updating(&mut self, updates: &[WorldUpdate]) {
        if let Some(f) = &mut self.far {
            if let Err(e) = f.update(updates) {
                tracing::error!("the shading's GPU: {e}");
            }
        }
    }

    fn updated(&mut self, core: &Core, changed: &[IVec3]) -> anyhow::Result<()> {
        self.cast.updated(core, changed)
    }

    fn update_meshes(&mut self, update: MeshUpdate) {
        self.cast.update_meshes(update);
    }

    fn render(&mut self, core: &mut Core, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let (passes, cast) = (&mut self.passes, &mut self.cast);
        if cast.stale() || self.shading != self.built_shading {
            (passes.plain, passes.meshed, passes.extra) = Passes::compile(core, &passes.pipeline_layout, cast, &self.shading, passes.remote)?;
            if core.accum.set_shading(&core.gpu, &self.shading) {
                passes.unbind();
            }
            self.built_shading = self.shading.clone();
            cast.compiled();
        }
        // With the shading on another GPU, this GPU's frames are the first half of one (their
        // stats go with the records).
        let ticket = match &mut self.far {
            Some(f) => core.ring.begin(&core.gpu, f.near_sink())?,
            None => core.ring.begin(&core.gpu, sink)?,
        };
        let start = Instant::now();
        let c = &req.camera;
        let mut enc = core.gpu.device.create_command_encoder(&Default::default());
        if core.build_accel(&mut enc, ticket, req) {
            passes.unbind();
        }
        cast.prepare(core, passes, &mut enc, ticket, req);
        core.accum.begin(&core.gpu, c, req.mode);
        core.write_camera(c, req.mode, cast.camera());
        let world_bg = core.world.prepare(&core.gpu, c).clone();
        passes.bind(core, cast);
        let before = cast.before(core, passes, &mut enc, ticket, req, &world_bg);
        let variant = if before.meshed { passes.meshed.as_ref().unwrap_or(&passes.plain) } else { &passes.plain };
        // The rays' pass ends the frame unless the deferred shading pass follows.
        let last = variant.shade.is_none();
        if last && !passes.remote {
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
                    timestamp_writes: core.ring.render_timestamps(ticket, before.first, last),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(pipeline);
                // The output is the colour attachment here, so group 0 binds a stand-in for
                // the storage-texture output.
                pass.set_bind_group(0, passes.bind0_frag.as_ref().expect("bound"), &[]);
                pass.set_bind_group(1, &world_bg, &[]);
                for (i, b) in cast.groups() {
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
            for (i, b) in cast.groups() {
                pass.set_bind_group(i, b, &[]);
            }
            pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        }
        let (mut counters, legend, mut stages) = cast.stats(core);
        let (accel, accel_stages) = core.accel_stats();
        counters.extend(accel);
        stages.extend(accel_stages);
        counters.extend(core.accum.counter(req.mode));
        let stats = FrameStats { cpu_ms: start.elapsed().as_secs_f32() * 1e3, gpu_ms: None, counters, legend, stages };
        cast.finish(&mut enc);
        if let Some(f) = &mut self.far {
            f.send(core, enc, ticket, req, stats);
            cast.submitted();
            return f.render(core, req, &self.shading, sink);
        }
        core.ring.submit(&core.gpu, enc, ticket, &core.output, req.frame_id, req.readback, stats);
        cast.submitted();
        core.ring.poll(&core.gpu, sink, false)
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

#[cfg(test)]
mod tests {
    /// A caster's program as the passes compile it (`shader`), composed and validated (no GPU
    /// needed).
    pub(crate) fn validate(lp: super::CastProgram, deferred: bool, steps: bool, fragment: bool, what: &str) {
        use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        let lp = super::shader(lp, deferred, steps, fragment);
        let deps: Vec<&str> = lp.deps.iter().map(String::as_str).collect();
        let m = lp.program.naga(&lp.root, &deps).unwrap_or_else(|e| panic!("{what}: {e}"));
        Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("{what}: {e:?}"));
    }
}
