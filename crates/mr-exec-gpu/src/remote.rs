//! The shading node on a second GPU: one pipeline split over two GPUs at the hit records. Each
//! GPU slot has an executor of its own ([`crate::exec`]) running its device's passes of the
//! schedule: the first the rays' (the ray-casting region, its program without the shading),
//! the second the shading's, this region ([`Shade`]): the common deferred pass over the hit
//! records of its executor's hits edge, its shadow and bounce rays walking (or tracing) the
//! storages compiled into the shading, kept on that GPU from the world's change log like any
//! storage. The records reach its edge through the edge's transfer ([`crate::transfer`]), the
//! executors' own; the image is this GPU's.
//!
//! With a meshes node the records name mesh triangles too: this GPU keeps its own copy of the
//! meshes (fed the same updates), placed for the camera the records were cast with, so a
//! record's triangle is the one the first GPU rasterised (instances that moved since are
//! decoded where they are now), and lit frames' shading rays meet the triangles.

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

/// Group 0's binding of the meshes' triangles (`mr_gpu::meshes::surface_wgsl`).
const MESH_BINDING: u32 = crate::REGION_BINDING;

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
    /// With a meshes node: the copy of the meshes and the passes for frames with them.
    meshes: Option<Meshes>,
}

/// The meshes on the second GPU: their copy, the site of the rays lighting them, and the
/// pass's variants for frames with mesh records (unlit: their surfaces; lit: and the
/// shading's rays meeting the triangles), compiled when a frame first needs one.
struct Meshes {
    meshes: mr_gpu::meshes::GpuMeshes,
    shadow: usize,
    variants: [Option<wgpu::ComputePipeline>; 2],
    /// This frame's: it has mesh records (mesh mode and instances), lit.
    on: Option<bool>,
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
    // The meshes are lit through their own rays where this GPU walks them, else through the
    // shading's shadow rays (the same world, walked on this GPU's copy).
    let meshed = g.find(Role::Meshes).is_some_and(|id| g.node(id).component != "none");
    let mesh_shadow = g.site(Role::Meshes, "shadow").filter(|s| meshed && g.devices[s.kernel] == Device::Gpu(slot as u8)).map_or(shadow, |s| walks.site(g, s, STORAGE_GROUPS, false));
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
    let mut entries = core.layout_entries();
    if meshed {
        entries.push(wgpu::BindGroupLayoutEntry { binding: MESH_BINDING, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None });
    }
    let layout0 = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("remote shading"), entries: &entries });
    let pipeline_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("remote shading"), bind_group_layouts: &core.storages.layouts(&[Some(&layout0), Some(core.world.layout())]), immediate_size: 0 });
    let shade = compile(&core, &pipeline_layout, &walks, shadow, bounce, spec.shading, false, None)?;
    let meshes = meshed.then(|| Meshes { meshes: mr_gpu::meshes::GpuMeshes::new(&core.gpu, None), shadow: mesh_shadow, variants: [None, None], on: None });
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
        meshes,
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
            let mut entries = core.bind_entries(&view);
            entries.extend(self.meshes.as_ref().map(|m| wgpu::BindGroupEntry { binding: MESH_BINDING, resource: m.meshes.vertices().as_entire_binding() }));
            self.bind0 = Some(core.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("remote shading"), layout: &self.layout0, entries: &entries }));
        }
        let pipeline = match self.meshes.as_ref().and_then(|m| Some(m.variants[m.on? as usize].as_ref().expect("compiled for the frame"))) {
            Some(p) => p,
            None => &self.shade,
        };
        core.ring.stage(enc, ticket, "shade", &core.stage_nodes[2]);
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("remote shade"), timestamp_writes: core.ring.compute_timestamps(ticket, true, true) });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, self.bind0.as_ref().unwrap(), &[]);
        pass.set_bind_group(1, self.world.as_ref().expect("set up"), &[]);
        self.bound.compute(&mut pass);
        pass.dispatch_workgroups(core.width.div_ceil(8), core.height.div_ceil(8), 1);
    }
}

impl exec::Region for Shade {
    fn compile(&mut self, core: &mut Core, req: &FrameRequest) -> anyhow::Result<()> {
        let layers = core.edges.layers();
        if self.shading != self.built_shading || layers != self.built_layers {
            self.shade = compile(core, &self.pipeline_layout, &self.walks, self.shadow, self.bounce, &self.shading, layers > 1, None)?;
            if let Some(m) = &mut self.meshes {
                m.variants = [None, None];
            }
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
        // The variant with the meshes for this frame's mode, the first time one asks for it.
        if let Some(m) = &mut self.meshes {
            let lit = req.mode == RenderMode::Lit;
            m.on = (req.meshes && m.meshes.has_instances()).then_some(lit);
            if m.on.is_some() && m.variants[lit as usize].is_none() {
                m.variants[lit as usize] = Some(compile(core, &self.pipeline_layout, &self.walks, self.shadow, self.bounce, &self.shading, layers > 1, Some((m.shadow, lit)))?);
            }
        }
        Ok(())
    }

    /// `req`'s camera is the one the records were cast with (the meshes are placed for it).
    fn setup(&mut self, core: &mut Core, req: &FrameRequest) {
        (self.accel, self.stored) = (false, false);
        if let Some(m) = self.meshes.as_mut().filter(|m| m.on.is_some()) {
            let view_proj = req.camera.projection(core.width as f32 / core.height as f32) * req.camera.view_rotation();
            if m.meshes.prepare(&core.gpu, &req.camera, view_proj, req.mode) {
                self.bind0 = None;
            }
        }
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

    fn update_meshes(&mut self, update: MeshUpdate) {
        if let Some(m) = &mut self.meshes {
            m.meshes.apply(update);
        }
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

/// The deferred shading pass's shader: the shading's walks (or ray queries' services), the
/// view and the records that arrived, the common pass (`shade`; `layered`: `shade_layers`
/// over `LAYERS` records per pixel; `meshed`: with the meshes' triangles, lit through the
/// site given, and lit frames' shading rays meeting them). Returns the program, its root and
/// the root's dependencies.
fn program(walks: &Walks, shadow: usize, bounce: usize, shading: &str, layered: bool, meshed: Option<(usize, bool)>) -> (mr_gpu::program::Program, String, Vec<String>) {
    let (mut program, rays) = shaders::sites_program(walks, None, shadow, bounce);
    let services: Vec<&str> = rays.iter().map(String::as_str).filter(|r| r.starts_with("rays::")).collect();
    shaders::shading(&mut program, shading, &services, 0, crate::STATE_BINDING);
    let meshes = meshed.map(|(mesh_shadow, lit)| shaders::meshes(&mut program, walks, shading, [shadow, bounce], mesh_shadow, 0, MESH_BINDING, lit));
    let mut root = format!("// Remote shading: shading = {shading}\n{}\n{}\n{}", include_str!("raycast/view.wgsl"), crate::hits_wgsl() + include_str!("raycast/records.wgsl"), mr_components::shading::DEFERRED_PACKED_WGSL);
    if layered {
        program.define("LAYERS");
        program.define("DEFERRED_LAYERS");
        root = format!("{root}\n{}", include_str!("raycast/layer_records.wgsl"));
    }
    let mut deps = vec![shaders::SHADING.to_string(), shaders::STATE.to_string()];
    deps.extend(meshes.into_iter().flatten());
    deps.extend(rays);
    (program, root, deps)
}

#[allow(clippy::too_many_arguments)]
fn compile(core: &Core, layout: &wgpu::PipelineLayout, walks: &Walks, shadow: usize, bounce: usize, shading: &str, layered: bool, meshed: Option<(usize, bool)>) -> anyhow::Result<wgpu::ComputePipeline> {
    let (mut program, root, deps) = program(walks, shadow, bounce, shading, layered, meshed);
    core.bind_program(&mut program, "0u");
    let deps: Vec<&str> = deps.iter().map(String::as_str).collect();
    let dev = &core.gpu.device;
    let scope = dev.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = mr_gpu::program::compile(dev, "remote shading", &program, &root, &deps);
    let pipeline = dev.compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some("remote shade"), layout: Some(layout), module: &module, entry_point: Some(if layered { "shade_layers" } else { "shade" }), compilation_options: Default::default(), cache: core.gpu.cache() });
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
                // Layered records where the shading composites them.
                for layered in [false, true].into_iter().filter(|&l| !l || mr_components::caps::provides(mr_components::Kind::Gpu, Role::Shading, shading, mr_components::Cap::Layers)) {
                    let (program, root, deps) = program(&walks, shadow, shadow, shading, layered, None);
                    let deps: Vec<&str> = deps.iter().map(String::as_str).collect();
                    let m = program.naga(&root, &deps).unwrap_or_else(|e| panic!("{st} {k} {shading} {layered}: {e}"));
                    Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("{st} {k} {shading} {layered}: {e:?}"));
                }
            }
        }
    }
}
