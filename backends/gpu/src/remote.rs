//! The shading node on a second GPU (PLAN step 13e4c): one pipeline split over two GPUs at
//! the hit records. The visibility node casts the primary rays on its GPU and writes one
//! record per pixel (`placement = deferred`); the schedule's transfer adapter for that edge
//! (`visibility` -> `shading`.hits, `now`) carries them to the shading's GPU, through the
//! host (read back from the one, written to the other: no peer copy in wgpu); there the
//! common deferred pass shades them, its shadow and bounce rays walking (or tracing) the
//! storages compiled into the shading, kept on that GPU from the world's change log like any
//! storage. The image is the shading's GPU's.
//!
//! [`Far`] is everything on the second GPU: a core of its own (the world, the shading's
//! storages and acceleration structures, the shading state, the camera, the records that
//! arrived, the output, its frame ring) and the shading pass. A frame: the rays' pass on the
//! first GPU, its records copied out and submitted ([`Far::send`]: waits for them, so the
//! edge is `now`), written to the second GPU, shaded there ([`Far::render`]). The frame's GPU
//! time is both GPUs' passes (sequential here); its stages are the first's (`cast`, `copy`:
//! the records into the readback buffer), the transfer (CPU: the upload; the wait for the
//! first GPU is the counter `transfer_wait_ms`) and the second's (`shade`).

use std::time::Instant;

use glam::IVec3;
use mr_components::graph::{self, Device, Role};
use mr_components::shaders::{self, Walks};
use mr_components::storage::gpu_data::Bound;
use mr_core::*;
use mr_gpu::{GpuNeeds, Ticket, wgpu};

use crate::{Core, CoreSpec};

/// The storage groups of the shading's walks on the second GPU.
const STORAGE_GROUPS: &[u32] = &[2, 3, 4, 5];

/// The GPU slot the shading runs on, if it is another than the visibility node's.
pub(crate) fn far_slot(g: &graph::Checked) -> Option<usize> {
    let at = |r| g.find(r).map(|id| g.devices[id]);
    match (at(Role::Visibility), at(Role::Shading)) {
        (Some(near), Some(Device::Gpu(far))) if near != Device::Gpu(far) => Some(far as usize),
        _ => None,
    }
}

/// Frames' stats captured from the first GPU's ring (its frames are no frames of their own).
#[derive(Default)]
struct Captured(Option<FrameStats>);

impl FrameSink for Captured {
    fn complete(&mut self, frame: CompletedFrame<'_>) {
        self.0 = Some(frame.stats);
    }
}

/// The second GPU's part of the pipeline.
pub(crate) struct Far {
    core: Core,
    walks: Walks,
    shadow: usize,
    bounce: usize,
    layout0: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    shade: wgpu::ComputePipeline,
    bind0: Option<wgpu::BindGroup>,
    bound: Bound,
    built_shading: String,
    /// On the first GPU: the records read back.
    staging: wgpu::Buffer,
    near: Captured,
    /// The transfer's nodes (the edge's ends); its last CPU times: waiting for the first GPU's
    /// frame (its passes and the records' copy) and the upload.
    transfer_nodes: Vec<String>,
    wait_ms: f32,
    transfer_ms: f32,
}

fn staging(near: &Core) -> wgpu::Buffer {
    near.gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("hit records readback"),
        size: crate::records_size(near.width, near.height),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

impl Far {
    /// The second GPU's part of graph `g` if its shading runs on another GPU than its
    /// visibility node (`spec`: the first GPU's core, whose knobs it shares).
    pub(crate) fn of(p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked, near: &Core, spec: &CoreSpec) -> anyhow::Result<Option<Self>> {
        let Some(slot) = far_slot(g) else { return Ok(None) };
        anyhow::ensure!(near.records.is_some(), "the shading on gpu{slot} shades the hit records the rays write on another GPU: placement = deferred");
        let mut walks = Walks::default();
        let site = |port| g.site(Role::Shading, port);
        let shadow = walks.site(g, site("shadow").ok_or_else(|| anyhow::anyhow!("the shading's shadow rays are not bound"))?, STORAGE_GROUPS, false);
        let bounce = site("bounce").map_or(shadow, |s| walks.site(g, s, STORAGE_GROUPS, false));
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
                records: true,
                ..*spec
            },
        )?;
        let dev = &core.gpu.device;
        let layout0 = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("remote shading"), entries: &core.layout_entries() });
        let pipeline_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("remote shading"), bind_group_layouts: &core.storages.layouts(&[Some(&layout0), Some(core.world.layout())]), immediate_size: 0 });
        let shade = compile(&core, &pipeline_layout, &walks, shadow, bounce, spec.shading)?;
        let vis = g.find(Role::Visibility).unwrap_or("visibility").to_string();
        let sh = g.find(Role::Shading).unwrap_or("shading").to_string();
        tracing::info!("remote shading on gpu{slot} ({}): shadow {}, bounce {}", core.gpu.info.name, walks.label(shadow), walks.label(bounce));
        Ok(Some(Self {
            staging: staging(near),
            near: Captured::default(),
            transfer_nodes: vec![vis, sh],
            wait_ms: 0.0,
            transfer_ms: 0.0,
            built_shading: spec.shading.to_string(),
            bind0: None,
            bound: Bound(Vec::new()),
            layout0,
            pipeline_layout,
            shade,
            walks,
            shadow,
            bounce,
            core,
        }))
    }

    /// The first GPU's frame sink: its frames' stats, kept for the frame the second shades.
    pub(crate) fn near_sink(&mut self) -> &mut dyn FrameSink {
        &mut self.near
    }

    /// World changes: the second GPU's world and storages apply them too.
    pub(crate) fn update(&mut self, updates: &[WorldUpdate]) -> anyhow::Result<()> {
        let c = &mut self.core;
        if !updates.is_empty() {
            c.accum.invalidate();
        }
        let changed: Vec<IVec3> = c.world.apply(&c.gpu, updates.to_vec());
        c.storages.update(&c.gpu, &c.world, &changed)?;
        if let Some(a) = &mut c.accel {
            a.updated(&c.world, &changed);
        }
        Ok(())
    }

    /// The transfer: submits the first GPU's frame `enc` with its records copied out, waits for
    /// them and writes them to the second GPU (with the frame's stats from the first).
    pub(crate) fn send(&mut self, near: &mut Core, mut enc: wgpu::CommandEncoder, ticket: Ticket, req: &FrameRequest, stats: FrameStats) -> anyhow::Result<()> {
        let records = near.records.as_ref().expect("placement = deferred");
        near.ring.stage(&mut enc, ticket, "copy", &self.transfer_nodes);
        enc.copy_buffer_to_buffer(records, 0, &self.staging, 0, records.size());
        near.ring.submit(&near.gpu, enc, ticket, &near.output, req.frame_id, false, stats);
        let start = Instant::now();
        let slice = self.staging.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| {
            if let Err(e) = r {
                tracing::error!("hit records readback failed: {e}");
            }
        });
        near.gpu.device.poll(wgpu::PollType::wait_indefinitely())?;
        let arrived = Instant::now();
        self.core.gpu.queue.write_buffer(self.core.records.as_ref().expect("records"), 0, &slice.get_mapped_range()?);
        self.staging.unmap();
        self.wait_ms = (arrived - start).as_secs_f32() * 1e3;
        self.transfer_ms = arrived.elapsed().as_secs_f32() * 1e3;
        near.ring.poll(&near.gpu, &mut self.near, true)
    }

    /// Shades the records that arrived with `shading` and hands the frame to `sink`.
    pub(crate) fn render(&mut self, req: &FrameRequest, shading: &str, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let c = &mut self.core;
        if shading != self.built_shading {
            self.shade = compile(c, &self.pipeline_layout, &self.walks, self.shadow, self.bounce, shading)?;
            if c.accum.set_shading(&c.gpu, shading) {
                self.bind0 = None;
            }
            self.built_shading = shading.to_string();
        }
        let ticket = c.ring.begin(&c.gpu, sink)?;
        let start = Instant::now();
        let mut enc = c.gpu.device.create_command_encoder(&Default::default());
        if c.build_accel(&mut enc, ticket, req) {
            self.bind0 = None;
        }
        c.accum.begin(&c.gpu, &req.camera, req.mode);
        c.write_camera(&req.camera, req.mode, [0, 0]);
        let world = c.world.prepare(&c.gpu, &req.camera).clone();
        self.bound = c.storages.prepare(&c.gpu, &c.world);
        if self.bind0.is_none() {
            let view = c.output.create_view(&Default::default());
            self.bind0 = Some(c.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("remote shading"), layout: &self.layout0, entries: &c.bind_entries(&view) }));
        }
        c.ring.stage(&mut enc, ticket, "shade", &c.stage_nodes[2]);
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("remote shade"), timestamp_writes: c.ring.compute_timestamps(ticket, true, true) });
            pass.set_pipeline(&self.shade);
            pass.set_bind_group(0, self.bind0.as_ref().unwrap(), &[]);
            pass.set_bind_group(1, &world, &[]);
            self.bound.compute(&mut pass);
            pass.dispatch_workgroups(c.width.div_ceil(8), c.height.div_ceil(8), 1);
        }
        // The first GPU's frame: its counters, stages and GPU time; the ring adds this one's.
        let mut stats = self.near.0.take().unwrap_or_default();
        stats.stages.push(StageTime::cpu("transfer", &self.transfer_nodes, self.transfer_ms));
        let (accel, accel_stages) = c.accel_stats();
        stats.counters.extend(accel);
        stats.stages.extend(accel_stages);
        stats.counters.extend(c.accum.counter(req.mode));
        stats.counters.push(("transfer_wait_ms".into(), self.wait_ms as f64));
        stats.cpu_ms += start.elapsed().as_secs_f32() * 1e3 + self.wait_ms + self.transfer_ms;
        c.ring.submit(&c.gpu, enc, ticket, &c.output, req.frame_id, req.readback, stats);
        c.ring.poll(&c.gpu, sink, false)
    }

    pub(crate) fn poll(&mut self, sink: &mut dyn FrameSink, wait: bool) -> anyhow::Result<()> {
        self.core.ring.poll(&self.core.gpu, sink, wait)
    }

    /// The core's knobs the shading's rays and state read (`None` if `name` is not one).
    pub(crate) fn set_param(&mut self, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        self.core.set_param(name, value)
    }

    pub(crate) fn resize(&mut self, near: &Core) {
        self.core.resize(near.width, near.height);
        self.staging = staging(near);
        self.bind0 = None;
    }

    pub(crate) fn memory_bytes(&self) -> u64 {
        self.core.memory_bytes() + self.staging.size()
    }
}

/// The deferred shading pass's shader: the shading's walks (or ray queries' services), the
/// view and the records that arrived, the common pass (`shade`). Returns the program, its root
/// and the root's dependencies.
fn program(walks: &Walks, shadow: usize, bounce: usize, shading: &str) -> (mr_gpu::program::Program, String, Vec<String>) {
    let (mut program, rays) = shaders::sites_program(walks, None, shadow, bounce);
    let services: Vec<&str> = rays.iter().map(String::as_str).filter(|r| r.starts_with("rays::")).collect();
    shaders::shading(&mut program, shading, &services, 0, crate::STATE_BINDING);
    let root = format!("// Remote shading: shading = {shading}\n{}\n{}\n{}", include_str!("raycast/view.wgsl"), include_str!("raycast/records.wgsl"), mr_components::shading::DEFERRED_PACKED_WGSL);
    let mut deps = vec![shaders::SHADING.to_string(), shaders::STATE.to_string()];
    deps.extend(rays);
    (program, root, deps)
}

fn compile(core: &Core, layout: &wgpu::PipelineLayout, walks: &Walks, shadow: usize, bounce: usize, shading: &str) -> anyhow::Result<wgpu::ComputePipeline> {
    let (mut program, root, deps) = program(walks, shadow, bounce, shading);
    core.trace_root(&mut program, "0u");
    let deps: Vec<&str> = deps.iter().map(String::as_str).collect();
    let dev = &core.gpu.device;
    let scope = dev.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = mr_gpu::program::compile(dev, "remote shading", &program, &root, &deps);
    let pipeline = dev.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some("remote shade"), layout: Some(layout), module: &module, entry_point: Some("shade"), compilation_options: Default::default(), cache: None });
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
        let mut g = Graph { executor: "gpu-compute".into(), nodes: Default::default() };
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
            for shading in mr_components::names_on("gpu-compute", mr_components::Slot::Shading) {
                let g = split(st, k, shading);
                let ck = g.check().unwrap_or_else(|e| panic!("{st} {k} {shading}: {e}"));
                g.accept().unwrap_or_else(|e| panic!("{st} {k} {shading}: {e}"));
                assert_eq!(far_slot(&ck), Some(1));
                assert_eq!((ck.devices["s.kernel"], ck.devices["s.storage"], ck.devices["kernel"]), (Device::Gpu(1), Device::Gpu(1), Device::Gpu(0)));
                let mut walks = Walks::default();
                let shadow = walks.site(&ck, ck.site(Role::Shading, "shadow").unwrap(), STORAGE_GROUPS, false);
                let (program, root, deps) = program(&walks, shadow, shadow, shading);
                let deps: Vec<&str> = deps.iter().map(String::as_str).collect();
                let m = program.naga(&root, &deps).unwrap_or_else(|e| panic!("{st} {k} {shading}: {e}"));
                Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("{st} {k} {shading}: {e:?}"));
            }
        }
    }
}
