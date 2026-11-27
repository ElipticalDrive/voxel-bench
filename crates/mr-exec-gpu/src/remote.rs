//! The shading node on a second GPU: one pipeline split over two GPUs at the hit records. Each
//! GPU slot has an executor of its own ([`crate::exec`]) running its device's passes of the
//! schedule: the first the rays' (the ray-casting region, its program without the shading),
//! the second the shading's, this region ([`Shade`]): the common deferred pass over the hit
//! records of its executor's hits edge, its shadow and bounce rays walking (or tracing) the
//! storages compiled into the shading, kept on that GPU from the world's change log like any
//! storage. The records reach its edge through the edge's transfer ([`crate::transfer`]), the
//! executors' own; the image is this GPU's.

use mr_gpu::Pipelines as _;
use mr_components::graph::{self, Device, Role};
use mr_components::shaders::{self, Walks};
use mr_components::storage::gpu_data::Bound;
use mr_core::*;
use mr_gpu::{GpuNeeds, Ticket, wgpu};

use crate::exec::{self, Cx, Executor, Node};
use crate::{Core, CoreSpec};

/// The storage groups of the shading's walks on the second GPU.
const STORAGE_GROUPS: &[u32] = &[2, 3, 4, 5];

/// The GPU slot the shading runs on, if it is another than that of the node its hits come
/// from (ray casting's visibility, the face drawer), or the shading's own when gates sit on
/// its hits: past a gate the shading is a domain of its own, with its own copy of what it
/// walks, on the same GPU or another.
pub(crate) fn far_slot(g: &graph::Checked) -> Option<usize> {
    let near = g.input(Role::Shading, "hits").map(|id| g.devices[id]);
    // Regions that cannot split hold gated hits between their own passes ([`crate::cross`]).
    let gated = g.gates_on(g.shading(), "hits").is_some() && g.region.is_some_and(|r| r.split);
    match (near, g.find(Role::Shading).map(|id| g.devices[id])) {
        (Some(near), Some(Device::Gpu(far))) if near != Device::Gpu(far) || gated => Some(far as usize),
        _ => None,
    }
}

/// The second GPU's executor.
pub(crate) type Far = Executor<Shade>;

/// The region of the shading's nodes on a GPU of their own: the deferred pass's program and
/// group 0, the shading it was compiled with, the frame's state.
pub(crate) struct Shade {
    walks: Walks,
    shadow: usize,
    bounce: usize,
    layout0: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    shade: wgpu::ComputePipeline,
    bind0: Option<wgpu::BindGroup>,
    bound: Bound,
    shading: String,
    /// Shading the pass was compiled with (recompiled at the next frame on a change).
    built_shading: String,
    /// Records per pixel the pass was compiled for (`LAYERS` once layered records arrive:
    /// `shade_layers` composites them).
    built_layers: u32,
    /// This frame's world bind group, and what ran of the work before the pass (each once,
    /// from its node's pass or the shading's).
    world: Option<wgpu::BindGroup>,
    accel: bool,
    stored: bool,
}

/// The executor of graph `g`'s shading on its own GPU, if that is another than the visibility
/// node's (`near`, `spec`: the first GPU's core, whose knobs it shares).
pub(crate) fn create(p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked, near: &Core, spec: &CoreSpec) -> anyhow::Result<Option<Far>> {
    let Some(slot) = far_slot(g) else { return Ok(None) };
    anyhow::ensure!(near.edges.hits().is_some_and(|h| h.holds_records()), "the shading on gpu{slot} shades the hit records written on another GPU: placement = deferred (rays), draw = gbuffer or visbuffer (faces), or splats");
    let mut walks = Walks::of(g);
    let site = |port| g.site(Role::Shading, port);
    let shadow = walks.site(g, site("shadow").ok_or_else(|| anyhow::anyhow!("the shading's shadow rays are not bound"))?, STORAGE_GROUPS, false);
    let bounce = site("bounce").map_or(shadow, |s| walks.site(g, s, STORAGE_GROUPS, false));
    walks.set_max_steps(spec.max_steps);
    let core = Core::on(
        slot,
        p,
        devices,
        waker,
        g,
        CoreSpec {
            gpu: GpuNeeds { experimental: false, required_features: wgpu::Features::empty(), optional_features: wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS },
            walks: &walks,
            accel: crate::accel::AccelSpec::of(g, &walks)?,
            stages: wgpu::ShaderStages::COMPUTE,
            max_lod: 0,
            voxels: false,
            hits: Some(exec::Form::Records),
            ..*spec
        },
    )?;
    let dev = &core.gpu.device;
    let layout0 = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("remote shading"), entries: &core.layout_entries() });
    let pipeline_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("remote shading"), bind_group_layouts: &core.storages.layouts(&[Some(&layout0), Some(core.world.layout())]), immediate_size: 0 });
    let shade = compile(&core, &pipeline_layout, &walks, shadow, bounce, spec.shading, Records::One)?;
    core.gpu.save_cache();
    tracing::info!("shading on gpu{slot} ({}): shadow {}, bounce {}", core.gpu.info.name, walks.label(shadow), walks.label(bounce));
    let passes = exec::far_passes(g, slot, |role| -> Option<Box<dyn Node<Shade>>> {
        Some(match role {
            Role::Traversal => Box::new(Accel),
            Role::Storage => Box::new(Storage),
            Role::Shading => Box::new(Shading),
            _ => return None,
        })
    })?;
    let region = Shade {
        shading: spec.shading.to_string(),
        built_shading: spec.shading.to_string(),
        built_layers: 1,
        bind0: None,
        bound: Bound(Vec::new()),
        world: None,
        accel: false,
        stored: false,
        layout0,
        pipeline_layout,
        shade,
        walks,
        shadow,
        bounce,
    };
    // The hits' gates are the transfer's.
    Ok(Some(Executor::new(core, region, passes, true)))
}

impl Shade {
    /// The frame's acceleration structures (group 0 is bound anew if they changed it).
    fn accel(&mut self, core: &mut Core, enc: &mut wgpu::CommandEncoder, ticket: Ticket, req: &FrameRequest) {
        if !std::mem::replace(&mut self.accel, true) && core.build_accel(enc, ticket, req) {
            self.bind0 = None;
        }
    }

    /// The storages' bind groups.
    fn store(&mut self, core: &mut Core, enc: &mut wgpu::CommandEncoder, ticket: Ticket, req: &FrameRequest) {
        self.accel(core, enc, ticket, req);
        if !std::mem::replace(&mut self.stored, true) {
            self.bound = core.storages.prepare(&core.gpu, &core.world);
        }
    }

    /// The deferred pass over the records of the hits edge (after the work before it, if its
    /// nodes had no pass).
    fn shade(&mut self, core: &mut Core, enc: &mut wgpu::CommandEncoder, ticket: Ticket, req: &FrameRequest) {
        self.store(core, enc, ticket, req);
        if self.bind0.is_none() {
            let view = core.output.create_view(&Default::default());
            self.bind0 = Some(core.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("remote shading"), layout: &self.layout0, entries: &core.bind_entries(&view) }));
        }
        core.ring.stage(enc, ticket, "shade", &core.stage_nodes[2]);
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("remote shade"), timestamp_writes: core.ring.compute_timestamps(ticket, true, true) });
        pass.set_pipeline(&self.shade);
        pass.set_bind_group(0, self.bind0.as_ref().unwrap(), &[]);
        pass.set_bind_group(1, self.world.as_ref().expect("set up"), &[]);
        self.bound.compute(&mut pass);
        pass.dispatch_workgroups(core.width.div_ceil(8), core.height.div_ceil(8), 1);
    }
}

impl exec::Region for Shade {
    fn compile(&mut self, core: &mut Core, _req: &FrameRequest) -> anyhow::Result<()> {
        let layers = core.edges.layers();
        if self.shading != self.built_shading || layers != self.built_layers {
            self.shade = compile(core, &self.pipeline_layout, &self.walks, self.shadow, self.bounce, &self.shading, Records::of(layers > 1, &self.shading))?;
            // A new records buffer (layers): group 0 anew.
            if layers != self.built_layers {
                self.bind0 = None;
                self.built_layers = layers;
            }
            if core.accum.set_shading(&core.gpu, &self.shading) {
                self.bind0 = None;
            }
            self.built_shading = self.shading.clone();
        }
        Ok(())
    }

    /// `req`'s camera is the one the records were cast with.
    fn setup(&mut self, core: &mut Core, req: &FrameRequest) {
        (self.accel, self.stored) = (false, false);
        core.accum.begin(&core.gpu, &req.camera, req.mode);
        core.write_camera(&req.camera, req.mode, [0, 0]);
        self.world = Some(core.world.prepare(&core.gpu, &req.camera).clone());
    }

    fn stats(&self, core: &Core, req: &FrameRequest) -> (Vec<(String, f64)>, Option<Legend>, Vec<StageTime>) {
        let (mut counters, stages) = core.accel_stats();
        counters.extend(core.accum.counter(req.mode));
        (counters, None, stages)
    }

    fn finish(&mut self, _: &Core, _: &mut wgpu::CommandEncoder, _: Ticket) {}
}

impl exec::State for Shade {
    fn updated(&mut self, _core: &Core, _changed: &[glam::IVec3]) -> anyhow::Result<()> {
        Ok(())
    }

    fn set_param(&mut self, _core: &mut Core, _name: &str, _value: &ParamValue) -> Option<anyhow::Result<()>> {
        None
    }

    fn switch(&mut self, _core: &mut Core, s: &mr_components::graph::Switch) -> Option<anyhow::Result<()>> {
        let c = s.component.as_ref().filter(|_| s.role == Role::Shading)?;
        self.shading = c.clone();
        Some(Ok(()))
    }

    fn resize(&mut self, _core: &Core) {
        self.bind0 = None;
    }

    fn memory(&self, _core: &Core) -> (u64, u64) {
        (0, 0)
    }
}

/// A ray-query kernel of the shading's rays: the acceleration structures.
struct Accel;
struct Storage;
/// The deferred shading pass.
struct Shading;

impl Node<Shade> for Accel {
    fn run(&mut self, cx: &mut Cx<Shade>) {
        cx.region.accel(cx.core, cx.enc, cx.ticket, cx.req);
    }
}

impl Node<Shade> for Storage {
    fn run(&mut self, cx: &mut Cx<Shade>) {
        cx.region.store(cx.core, cx.enc, cx.ticket, cx.req);
    }
}

impl Node<Shade> for Shading {
    fn run(&mut self, cx: &mut Cx<Shade>) {
        cx.region.shade(cx.core, cx.enc, cx.ticket, cx.req);
    }
}

/// The records the far shading reads, and how it shades them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Records {
    /// One per pixel: `shade`.
    One,
    /// `LAYERS` per pixel, composited front to back: `shade_layers`.
    Composited,
    /// `LAYERS` per pixel, the first shaded (`shade`): a shading that does not composite
    /// layers (`Cap::Layers`), switched to while they arrive (the caster drops them once
    /// the switch reaches it, but what is on the way still has them).
    First,
}

impl Records {
    fn of(layered: bool, shading: &str) -> Self {
        match (layered, mr_components::caps::provides(mr_components::Kind::Gpu, Role::Shading, shading, mr_components::Cap::Layers)) {
            (false, _) => Self::One,
            (true, true) => Self::Composited,
            (true, false) => Self::First,
        }
    }
}

/// The deferred shading pass's shader: the shading's walks (or ray queries' services), the
/// view and the records that arrived, the common pass (`shade`; composited layers:
/// `shade_layers` over `LAYERS` records per pixel). Returns the program, its root and the
/// root's dependencies.
fn program(walks: &Walks, shadow: usize, bounce: usize, shading: &str, records: Records) -> (mr_gpu::program::Program, String, Vec<String>) {
    let (mut program, rays) = shaders::sites_program(walks, None, shadow, bounce);
    let services: Vec<&str> = rays.iter().map(String::as_str).filter(|r| r.starts_with("rays::")).collect();
    shaders::shading(&mut program, shading, &services, 0, crate::STATE_BINDING);
    let mut root = format!("// Remote shading: shading = {shading}\n{}\n{}\n{}", include_str!("raycast/view.wgsl"), crate::hits_wgsl() + include_str!("raycast/records.wgsl"), mr_components::shading::DEFERRED_PACKED_WGSL);
    match records {
        Records::One => {}
        Records::Composited => {
            program.define("LAYERS");
            program.define("DEFERRED_LAYERS");
            root = format!("{root}\n{}", include_str!("raycast/layer_records.wgsl"));
        }
        Records::First => drop(program.define("RECORDS_LAYERED")),
    }
    let mut deps = vec![shaders::SHADING.to_string(), shaders::STATE.to_string()];
    deps.extend(rays);
    (program, root, deps)
}

fn compile(core: &Core, layout: &wgpu::PipelineLayout, walks: &Walks, shadow: usize, bounce: usize, shading: &str, records: Records) -> anyhow::Result<wgpu::ComputePipeline> {
    let (mut program, root, deps) = program(walks, shadow, bounce, shading, records);
    core.bind_program(&mut program, "0u");
    let deps: Vec<&str> = deps.iter().map(String::as_str).collect();
    let dev = &core.gpu.device;
    let scope = dev.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = mr_gpu::program::compile(dev, "remote shading", &program, &root, &deps);
    let pipeline = dev.compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some("remote shade"), layout: Some(layout), module: &module, entry_point: Some(if records == Records::Composited { "shade_layers" } else { "shade" }), compilation_options: Default::default(), cache: core.gpu.cache() });
    if let Some(e) = pollster::block_on(scope.pop()) {
        anyhow::bail!("compiling the remote shading pass: {e}");
    }
    Ok(pipeline)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mr_components::graph::{Graph, Node};

    /// The split graph: the walk on `gpu`, the shading (deferred) with its own walk on `gpu1`.
    pub(crate) fn split(storage: &str, kernel: &str, shading: &str) -> Graph {
        let mut g = Graph { executor: "gpu".into(), nodes: Default::default() };
        let mut add = |id: &str, mut n: Node, inputs: &[(&str, &str)]| {
            n.inputs.extend(inputs.iter().map(|&(p, s)| (p.to_string(), s.to_string())));
            g.nodes.insert(id.into(), n);
        };
        add("world", Node::new(Role::World, "world"), &[]);
        add("storage", Node::new(Role::Storage, "bricks"), &[("changes", "world")]);
        add("kernel", Node::new(Role::Traversal, "refloor"), &[("storage", "storage")]);
        add("visibility", Node::new(Role::Visibility, "raycast"), &[("primary", "kernel")]);
        add("s.storage", Node::new(Role::Storage, storage), &[("changes", "world")]);
        add("s.kernel", Node::new(Role::Traversal, kernel), &[("storage", "s.storage")]);
        let mut sh = Node::new(Role::Shading, shading);
        sh.device = Some(Device::Gpu(1));
        sh.knobs.insert("placement".into(), toml::Value::String("deferred".into()));
        add("shading", sh, &[("hits", "visibility"), ("shadow", "s.kernel")]);
        g
    }

    /// The remote shading pass composes and validates for every shading over walks of any
    /// storage (no GPU needed); the split graph places it on `gpu1`.
    #[test]
    fn program_composes() {
        use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        for (st, k) in [("bricks", "refloor"), ("tree64", "sphere"), ("distance", "incremental")] {
            let shadings = mr_components::graph::shipped("raycast-gpu").unwrap().choices("shading");
            assert!(shadings.len() > 1, "{shadings:?}");
            for shading in shadings {
                let g = split(st, k, shading);
                let ck = g.check().unwrap_or_else(|e| panic!("{st} {k} {shading}: {e}"));
                g.accept().unwrap_or_else(|e| panic!("{st} {k} {shading}: {e}"));
                assert_eq!(far_slot(&ck), Some(1));
                assert_eq!((ck.devices["s.kernel"], ck.devices["s.storage"], ck.devices["kernel"]), (Device::Gpu(1), Device::Gpu(1), Device::Gpu(0)));
                let mut walks = Walks::of(&ck);
                let shadow = walks.site(&ck, ck.site(Role::Shading, "shadow").unwrap(), STORAGE_GROUPS, false);
                // Layered records: composited where the shading can, else the first shaded.
                for layered in [false, true] {
                    let records = Records::of(layered, shading);
                    let (program, root, deps) = program(&walks, shadow, shadow, shading, records);
                    let deps: Vec<&str> = deps.iter().map(String::as_str).collect();
                    let m = program.naga(&root, &deps).unwrap_or_else(|e| panic!("{st} {k} {shading} {records:?}: {e}"));
                    Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("{st} {k} {shading} {records:?}: {e:?}"));
                }
            }
        }
    }
}
