//! Point splatting on the node executor (components):
//! geometry `points`, visibility `splat`, shading `direct`, and the **storage** lit-mode
//! shadow rays walk (refloor; group 2).
//!
//! GPU point splatting: every face-exposed block becomes one point (geometry `points` on
//! the CPU, `mr_components::geometry::points`), uploaded as the schedule's transfer of the
//! points to the GPU; the splat passes and their resolve, the common deferred shading pass
//! over the ids, are visibility `splat` (`mr_components::visibility::splat`).
//! The splat passes and the resolve are nodes sharing the [`Splat`] region, which keeps the
//! points and their buffers. With the shading on another GPU ([`crate::remote`]) the region
//! splats only: its ids go to a buffer of its own and a decode pass after the splats (the
//! ids -> records adapter, `mr_components::draw::DECODE_WGSL`) writes `pack_hit` records into
//! the hits edge, what crosses.

use glam::IVec3;
use mr_components::geometry::points::Points;
use mr_components::graph::{self, Role};
use mr_components::shaders::Walks;
use mr_components::shading::state::{self as shade_state, ShadeState};
use mr_components::storage::gpu_data::Bound;
use mr_components::visibility::splat;
use mr_core::*;
use mr_gpu::voxels::{Shadows, shadows_param};
use mr_gpu::{Gpu, GpuNeeds, wgpu};
use serde::Deserialize;

use crate::exec::{self, Cx, Node, Region, State};
use crate::{Core, CoreSpec, REGION_BINDING, storage_entry};

pub(crate) fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu".into(),
        description: "GPU point splatting (compute): one point per exposed block, atomicMin depth + id buffers, ray-box resolve; lit-mode shadow rays walk the composed storage".into(),
        tags: vec!["gpu".into(), "compute".into(), "points".into(), "splat".into(), "modular".into(), "wgpu".into()],
        features: Features::of(&[Feature::Lit, Feature::Volume]),
        params: REGION.lighting_knobs(vec![
            ParamSpec::int("max_splat", "Largest splat side (px) one thread draws; larger points take a workgroup each", 1, 64, 16, true).of("visibility", &["splat"]),
            shadows_param(),
            ParamSpec::int("pool_mb", "Voxel pool limit (MiB)", 64, 6144, 2048, false),
            ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
        ]
        .into_iter()
        .chain(shade_state::params())
        .collect()),
    }
}

/// The region's own knobs (the nodes' are read from the graph).
#[derive(Deserialize)]
#[serde(default)]
struct Options {
    pool_mb: u32,
    frames_in_flight: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { pool_mb: 2048, frames_in_flight: 3 }
    }
}

/// The visibility node's knobs.
#[derive(Deserialize)]
#[serde(default)]
struct VisibilityKnobs {
    max_splat: u32,
}

impl Default for VisibilityKnobs {
    fn default() -> Self {
        Self { max_splat: 16 }
    }
}

/// The shading node's knobs.
#[derive(Deserialize)]
#[serde(default)]
struct ShadingKnobs {
    shadows: Shadows,
    bounces: u32,
    spp: u32,
    accumulate: bool,
}

impl Default for ShadingKnobs {
    fn default() -> Self {
        Self { shadows: Shadows::All, bounces: 1, spp: 1, accumulate: true }
    }
}

pub(crate) const REGION: &mr_components::graph::Builds = &mr_components::graph::GPU_SPLAT;

/// The region the splat's nodes share: the points and their buffers, the program, group 0,
/// the frame's world and storages. The ids the splat passes write and the resolve shades are
/// the hits edge's resource (the executor's), attached in group 0; drawing only (the shading
/// on another GPU), they are the region's own (`ids`) and the edge holds the records decoded
/// from them.
pub(crate) struct Splat {
    pipes: [wgpu::ComputePipeline; 6],
    layout0: wgpu::BindGroupLayout,
    zbuf: wgpu::Buffer,
    points_buf: wgpu::Buffer,
    /// The large points' list (a count, then up to one id per point).
    big_buf: wgpu::Buffer,
    /// Drawing only: the ids (the edge holds the records).
    ids: Option<wgpu::Buffer>,
    bind0: wgpu::BindGroup,
    /// The geometry node's state: every chunk's points, in slots as the buffer holds them.
    points: Points,
    /// Entries in the buffer (points and holes).
    count: u32,
    max_splat: u32,
    /// This frame's world bind group and storages' bind groups.
    world: Option<wgpu::BindGroup>,
    bound: Bound,
}

fn pixel_buffer(gpu: &Gpu, w: u32, h: u32) -> wgpu::Buffer {
    gpu.storage("splat fb", w as u64 * h as u64 * 4, wgpu::BufferUsages::STORAGE, || format!("the resolution ({w}x{h})"))
}

fn points_buffer(gpu: &Gpu, n: u64) -> wgpu::Buffer {
    gpu.storage("points", n.max(1) * 16, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, || format!("the world's points ({n} in view)"))
}

/// The large points' list for a points buffer of `points_bytes`: a count and one id per point.
fn big_buffer(gpu: &Gpu, points_bytes: u64) -> wgpu::Buffer {
    gpu.storage("big points", 4 + points_bytes / 4, wgpu::BufferUsages::STORAGE, || "the world's points (a quarter of the points' buffer)".into())
}

impl Splat {
    fn new(p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked) -> anyhow::Result<(Core, Self, Option<crate::remote::Far>)> {
        let opts: Options = p.options()?;
        let visibility = g.find(Role::Visibility).ok_or_else(|| anyhow::anyhow!("no visibility node"))?;
        let v: VisibilityKnobs = g.knobs(visibility).map_err(anyhow::Error::msg)?;
        let shading = &g.node(g.shading()).component;
        let k: ShadingKnobs = g.knobs(g.shading()).map_err(anyhow::Error::msg)?;
        // The points cross from the CPU (geometry) to the GPU (visibility): the schedule's
        // transfer, its adapter `upload`.
        let schedule = g.schedule().map_err(|e| anyhow::anyhow!(e))?;
        if !schedule.transfers.iter().any(|e| e.port == "points" && g.node(&e.to).role == Role::Visibility) {
            anyhow::bail!("the points do not cross from the CPU to the GPU");
        }
        // The sites' walks: shadow rays (the storage and traversal stages),
        // bounces their own or the same.
        // The shading on another GPU: splats only, their ids decoded into the records that cross.
        let draw_only = crate::remote::far_slot(g).is_some();
        let (walks, mut program, own, deps) = if draw_only { draw_program(g) } else { program(g, shading)? };
        let spec = CoreSpec {
                gpu: GpuNeeds { experimental: false, required_features: wgpu::Features::empty(), optional_features: wgpu::Features::TIMESTAMP_QUERY },
                walks: &walks,
                accel: crate::accel::AccelSpec::of(g, &walks)?,
                stages: wgpu::ShaderStages::COMPUTE,
                max_lod: 0,
                voxels: false,
                hits: Some(if draw_only { exec::Form::Records } else { exec::Form::Ids }),
                shading,
                bounces: k.bounces,
                spp: k.spp,
                accumulate: k.accumulate,
                shadows: k.shadows,
                max_steps: 2048,
                pool_mb: opts.pool_mb,
                frames_in_flight: opts.frames_in_flight,
            };
        let core = Core::new(p, devices, waker.clone(), g, spec.clone())?;
        // The shading on another GPU: that GPU's executor, shading the records decoded here.
        let far = crate::remote::create(p, devices, waker, g, &core, &spec)?;
        core.bind_program(&mut program, "0u");
        let gpu = &core.gpu;
        let module = mr_gpu::program::compile(&gpu.device, "splat", &program, &own, &deps.iter().map(String::as_str).collect::<Vec<_>>());
        let mut entries = core.layout_entries();
        entries.extend([storage_entry(REGION_BINDING, true, core.stages), storage_entry(REGION_BINDING + 1, false, core.stages), storage_entry(REGION_BINDING + 2, false, core.stages), storage_entry(REGION_BINDING + 3, false, core.stages)]);
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("splat"), entries: &entries });
        let pl = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("splat"), bind_group_layouts: &core.storages.layouts(&[Some(&layout0), Some(core.world.layout())]), immediate_size: 0 });
        let pipe = |e: &str| gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some(e), layout: Some(&pl), module: &module, entry_point: Some(e), compilation_options: Default::default(), cache: gpu.cache() });
        let pipes = splat::ENTRIES.map(|e| pipe(if draw_only && e == "shade" { "decode" } else { e }));
        let (points_buf, zbuf) = (points_buffer(gpu, 1024), pixel_buffer(gpu, p.width, p.height));
        let big_buf = big_buffer(gpu, points_buf.size());
        let ids = draw_only.then(|| pixel_buffer(gpu, p.width, p.height));
        let bind0 = bind0(&core, &layout0, &points_buf, &zbuf, &big_buf, ids.as_ref());
        let splat = Self { pipes, layout0, zbuf, points_buf, big_buf, ids, bind0, points: Points::default(), count: 0, max_splat: v.max_splat.clamp(1, 64), world: None, bound: Bound(Vec::new()) };
        Ok((core, splat, far))
    }

    /// The points' transfer: the slots that changed (all of them into a larger buffer when
    /// the array outgrew it).
    fn upload(&mut self, core: &Core) {
        let Some(ch) = self.points.changes() else { return };
        self.count = ch.all.len() as u32;
        let queue = &core.gpu.queue;
        if (ch.all.len() as u64) * 16 > self.points_buf.size() {
            self.points_buf = points_buffer(&core.gpu, ch.all.len() as u64 * 3 / 2);
            self.big_buf = big_buffer(&core.gpu, self.points_buf.size());
            self.bind0 = bind0(core, &self.layout0, &self.points_buf, &self.zbuf, &self.big_buf, self.ids.as_ref());
            queue.write_buffer(&self.points_buf, 0, bytemuck::cast_slice(ch.all));
            let rest = ch.all.len() as u64 - ch.ranges.iter().map(|r| r.len() as u64).sum::<u64>();
            self.points.uploaded += rest;
            return;
        }
        for r in ch.ranges {
            queue.write_buffer(&self.points_buf, r.start as u64 * 16, bytemuck::cast_slice(&ch.all[r]));
        }
    }
}

/// Group 0: the core's (with the hits edge's records when drawing only), the points, the
/// depths, the ids (the hits edge, the executor's; drawing only, `own`) and the large points'
/// list.
fn bind0(core: &Core, layout: &wgpu::BindGroupLayout, pts: &wgpu::Buffer, z: &wgpu::Buffer, big: &wgpu::Buffer, own: Option<&wgpu::Buffer>) -> wgpu::BindGroup {
    let view = core.output.create_view(&Default::default());
    let mut entries = core.bind_entries(&view);
    entries.extend([
        wgpu::BindGroupEntry { binding: REGION_BINDING, resource: pts.as_entire_binding() },
        wgpu::BindGroupEntry { binding: REGION_BINDING + 1, resource: z.as_entire_binding() },
        match own {
            Some(ids) => wgpu::BindGroupEntry { binding: REGION_BINDING + 2, resource: ids.as_entire_binding() },
            None => core.edges.hits().expect("the splats' ids cross to the resolve").bind_entry(REGION_BINDING + 2),
        },
        wgpu::BindGroupEntry { binding: REGION_BINDING + 3, resource: big.as_entire_binding() },
    ]);
    core.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("splat"), layout, entries: &entries })
}

/// The shader: the shadow and bounce rays' walks, the splat passes, the
/// shading component and the common deferred pass (the resolve) as the root.
fn program(g: &graph::Checked, shading: &str) -> anyhow::Result<(Walks, mr_gpu::program::Program, String, Vec<String>)> {
    let mut walks = Walks::of(g);
    let shadow = walks.site(g, g.site(Role::Shading, "shadow").ok_or_else(|| anyhow::anyhow!("splat-gpu: shadow rays are not bound"))?, &[2, 3, 4], false);
    let bounce = g.site(Role::Shading, "bounce").map_or(shadow, |s| walks.site(g, s, &[2, 3, 4], false));
    let shade = mr_components::shading::gpu_wgsl(shading).expect("validated");
    let own = format!("{}\n{}\n{shade}\n{}", splat::gpu_wgsl(), ShadeState::wgsl(0, crate::STATE_BINDING), mr_components::shading::DEFERRED_WGSL);
    let (program, deps) = mr_components::shaders::sites_program(&walks, None, 0, bounce);
    Ok((walks, program, own, deps))
}

/// The shader drawing only (the shading on another GPU): the splat passes and the decode of
/// their ids into the hits edge's records (entry `decode`), no shading and no walks.
fn draw_program(g: &graph::Checked) -> (Walks, mr_gpu::program::Program, String, Vec<String>) {
    let program = mr_gpu::program::Program::world(None);
    let root = format!(
        "{}
fn view_origin() -> vec3<f32> {{ return cam.origin; }}
{}
{}",
        splat::gpu_wgsl(),
        crate::exec::Hits::store_wgsl(0, crate::RECORDS_BINDING),
        mr_components::draw::DECODE_WGSL
    );
    (Walks::of(g), program, root, [mr_gpu::program::VOXELS, mr_gpu::program::COMMON].map(String::from).to_vec())
}

impl State for Splat {
    fn updated(&mut self, core: &Core, changed: &[IVec3]) -> anyhow::Result<()> {
        let world = &core.world;
        self.points.update(changed.to_vec(), |c| world.chunk(c));
        Ok(())
    }

    fn set_param(&mut self, _core: &mut Core, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        (name == "max_splat").then(|| {
            self.max_splat = value.as_i64().unwrap_or(16).clamp(1, 64) as u32;
            Ok(())
        })
    }

    fn resize(&mut self, core: &Core) {
        self.zbuf = pixel_buffer(&core.gpu, core.width, core.height);
        if self.ids.is_some() {
            self.ids = Some(pixel_buffer(&core.gpu, core.width, core.height));
        }
        self.bind0 = bind0(core, &self.layout0, &self.points_buf, &self.zbuf, &self.big_buf, self.ids.as_ref());
    }

    fn memory(&self, core: &Core) -> (u64, u64) {
        (self.points.bytes(), self.points_buf.size() + self.big_buf.size() + core.width as u64 * core.height as u64 * if self.ids.is_some() { 8 } else { 4 })
    }
}

impl Region for Splat {
    /// The points' transfer when the world changed them.
    fn compile(&mut self, core: &mut Core, _req: &FrameRequest) -> anyhow::Result<()> {
        self.upload(core);
        Ok(())
    }

    fn setup(&mut self, core: &mut Core, req: &FrameRequest) {
        let c = &req.camera;
        core.write_camera(c, req.mode, [self.count, self.max_splat]);
        core.accum.begin(&core.gpu, c, req.mode);
        self.world = Some(core.world.prepare(&core.gpu, c).clone());
        self.bound = core.storages.prepare(&core.gpu, &core.world);
    }

    fn stats(&self, core: &Core, req: &FrameRequest) -> (Vec<(String, f64)>, Option<Legend>, Vec<StageTime>) {
        let (accel, stages) = core.accel_stats();
        let counters = [("points".into(), self.points.live() as f64), ("point_slots".into(), self.count as f64), ("points_uploaded".into(), self.points.uploaded as f64)].into_iter().chain(accel).chain(core.accum.counter(req.mode)).collect();
        (counters, None, stages)
    }

    fn finish(&mut self, _: &Core, _: &mut wgpu::CommandEncoder, _: mr_gpu::Ticket) {}
}

/// The splat passes (after the shadow rays' acceleration structures): clear, then the small
/// points one per thread and the large ones a workgroup each, depth then ids.
struct Visibility;
/// The resolve: the common deferred shading over the ids.
struct Shading;

impl Node<Splat> for Visibility {
    fn run(&mut self, cx: &mut Cx<Splat>) {
        let (core, r, enc, ticket) = (&mut *cx.core, &mut *cx.region, &mut *cx.enc, cx.ticket);
        if core.build_accel(enc, ticket, cx.req) {
            r.bind0 = bind0(core, &r.layout0, &r.points_buf, &r.zbuf, &r.big_buf, r.ids.as_ref());
        }
        let groups = r.count.div_ceil(64);
        let (gx, gy) = (groups.min(65535), groups.div_ceil(65535));
        core.ring.stage(enc, ticket, "splat", &core.stage_nodes[1]);
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("splat"), timestamp_writes: core.ring.compute_timestamps(ticket, true, false) });
        pass.set_bind_group(0, &r.bind0, &[]);
        pass.set_bind_group(1, r.world.as_ref().expect("set up"), &[]);
        r.bound.compute(&mut pass);
        pass.set_pipeline(&r.pipes[0]);
        pass.dispatch_workgroups(core.width.div_ceil(8), core.height.div_ceil(8), 1);
        if groups > 0 {
            for (per_point, big) in [(1, 2), (3, 4)] {
                pass.set_pipeline(&r.pipes[per_point]);
                pass.dispatch_workgroups(gx, gy, 1);
                pass.set_pipeline(&r.pipes[big]);
                pass.dispatch_workgroups(splat::BIG_GROUPS, 1, 1);
            }
        }
        drop(pass);
        // Drawing only: the ids decoded into the records that cross (the frame's end here).
        if r.ids.is_some() {
            core.ring.stage(enc, ticket, "decode", &core.stage_nodes[1]);
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("decode"), timestamp_writes: core.ring.compute_timestamps(ticket, false, true) });
            pass.set_bind_group(0, &r.bind0, &[]);
            pass.set_bind_group(1, r.world.as_ref().expect("set up"), &[]);
            r.bound.compute(&mut pass);
            pass.set_pipeline(&r.pipes[5]);
            pass.dispatch_workgroups(core.width.div_ceil(8), core.height.div_ceil(8), 1);
        }
    }
}

impl Node<Splat> for Shading {
    fn run(&mut self, cx: &mut Cx<Splat>) {
        let (core, r, enc, ticket) = (&mut *cx.core, &*cx.region, &mut *cx.enc, cx.ticket);
        core.ring.stage(enc, ticket, "shade", &core.stage_nodes[2]);
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("shade"), timestamp_writes: core.ring.compute_timestamps(ticket, false, true) });
        pass.set_bind_group(0, &r.bind0, &[]);
        pass.set_bind_group(1, r.world.as_ref().expect("set up"), &[]);
        r.bound.compute(&mut pass);
        pass.set_pipeline(&r.pipes[5]);
        pass.dispatch_workgroups(core.width.div_ceil(8), core.height.div_ceil(8), 1);
    }
}

/// The splat executor for `g`: the splat passes and the resolve as nodes (the points are
/// built on the CPU and uploaded before the frame: the geometry node and its transfer); with
/// the shading on another GPU, that GPU's executor too.
pub(crate) fn create(p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked) -> anyhow::Result<Box<dyn Renderer>> {
    let passes = exec::passes(g, crate::home_slot(g), |role| -> Option<Box<dyn Node<Splat>>> {
        match role {
            Role::Visibility => Some(Box::new(Visibility)),
            Role::Shading => Some(Box::new(Shading)),
            _ => None,
        }
    })?;
    exec::run_or_split(Splat::new(p, devices, waker, g)?, g, passes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every composition's shader composes from its modules and validates (no GPU needed).
    #[test]
    fn programs_compose() {
        use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        use mr_components::graph::Change;
        let base = mr_components::graph::shipped("splat-gpu").unwrap();
        let mut n = 0;
        for st in base.choices("storage") {
            for k in base.choices("kernel") {
                for shading in base.choices("shading") {
                    let bounce = (shading == "pathtrace").then(|| Change::bind("shading", "bounce", "kernel"));
                    let flat: Vec<Change> = [("storage", st), ("kernel", k), ("shading", shading)].into_iter().map(|(n, c)| Change::component(n, c)).chain(bounce).collect();
                    let Ok(gr) = base.with_edits(&flat) else { continue };
                    if gr.validate().is_err() {
                        continue;
                    }
                    let (_, p, root, deps) = program(&gr.check().unwrap(), shading).unwrap();
                    let m = p.naga(&root, &deps.iter().map(String::as_str).collect::<Vec<_>>()).unwrap_or_else(|e| panic!("{flat:?}: {e}"));
                    Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("{flat:?}: {e:?}"));
                    n += 1;
                }
            }
        }
        assert!(n > 20, "only {n} compositions composed");
    }

    /// Drawing only (the shading on `gpu1`), the splat passes and the decode compose and
    /// validate, with none of the shading's modules.
    #[test]
    fn draw_program_composes() {
        use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        let g = mr_components::graph::shipped("splat-gpu").unwrap().with_edits(&[mr_components::graph::Change::Device("shading".into(), Some(graph::Device::Gpu(1)))]).unwrap();
        let ck = g.check().unwrap();
        assert_eq!(crate::remote::far_slot(&ck), Some(1));
        let (_, p, root, deps) = draw_program(&ck);
        let m = p.naga(&root, &deps.iter().map(String::as_str).collect::<Vec<_>>()).unwrap();
        Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap();
        for e in ["clear", "splat_depth", "splat_big_depth", "splat_id", "splat_big_id", "decode"] {
            assert!(m.entry_points.iter().any(|ep| ep.name == e), "no entry `{e}`");
        }
        assert!(!m.entry_points.iter().any(|ep| ep.name == "shade"));
    }
}
