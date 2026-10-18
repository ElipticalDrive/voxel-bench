//! The splat lane (components, IDEAS §12; PLAN step 13e3: was the gpu-splat back end):
//! geometry `points`, visibility `splat`, shading `direct`, and the **storage** lit-mode
//! shadow rays walk (refloor; group 2).
//!
//! GPU point splatting: every face-exposed block becomes one point (geometry `points` on
//! the CPU, `mr_components::geometry::points`), uploaded as the schedule's transfer of the
//! points to the GPU; the splat passes and their resolve, the common deferred shading pass
//! over the ids (PLAN step 12c), are visibility `splat` (`mr_components::visibility::splat`).
//! The lane keeps the points, their buffers and the passes.

use std::time::Instant;

use glam::IVec3;
use mr_components::geometry::points::{Point, Points};
use mr_components::graph::{self, Role};
use mr_components::shaders::Walks;
use mr_components::shading::state::{self as shade_state, ShadeState};
use mr_components::visibility::splat;
use mr_components::{Slot, names_on, require};
use mr_core::*;
use mr_gpu::voxels::{Shadows, shadows_param};
use mr_gpu::{Gpu, GpuNeeds, wgpu};
use serde::Deserialize;

use crate::{Core, CoreSpec, LANE_BINDING, Lane, storage_entry};

pub(crate) fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu-splat".into(),
        description: "GPU-splat back end (storage for shadow rays, points geometry, splat visibility, shading). Compute point splatting: one point per exposed block, atomicMin depth + id buffer, ray-box resolve".into(),
        tags: vec!["gpu".into(), "compute".into(), "points".into(), "splat".into(), "modular".into(), "wgpu".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: mr_components::lighting_knobs(BACKEND, vec![
            ParamSpec::choice("storage", "Storage component the lit-mode shadow rays walk (with the traversal kernel)", &names_on(BACKEND, Slot::Storage), "bricks", false),
            ParamSpec::choice("geometry", "Geometry component", &names_on(BACKEND, Slot::Geometry), "points", false),
            ParamSpec::choice("visibility", "Visibility component", &names_on(BACKEND, Slot::Visibility), "splat", false),
            ParamSpec::choice("traversal", "Traversal kernel of the lit-mode shadow rays", &names_on(BACKEND, Slot::Traversal), "refloor", false),
            ParamSpec::choice("shading", "Shading component", &names_on(BACKEND, Slot::Shading), "direct", false),
            ParamSpec::int("max_splat", "Largest splat side (px)", 1, 64, 8, true).of("visibility", &["splat"]),
            shadows_param(),
            ParamSpec::int("pool_mb", "Voxel pool limit (MiB)", 64, 6144, 2048, false),
            ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
        ]
        .into_iter()
        .chain(shade_state::params())
        .collect()),
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    storage: String,
    geometry: String,
    visibility: String,
    traversal: String,
    shading: String,
    max_splat: u32,
    bounces: u32,
    spp: u32,
    accumulate: bool,
    shadows: Shadows,
    pool_mb: u32,
    frames_in_flight: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { storage: "bricks".into(), geometry: "points".into(), visibility: "splat".into(), traversal: "refloor".into(), shading: "direct".into(), max_splat: 8, bounces: 1, spp: 1, accumulate: true, shadows: Shadows::All, pool_mb: 2048, frames_in_flight: 3 }
    }
}

pub(crate) const BACKEND: &str = "gpu-splat";

pub(crate) struct Splat {
    pipes: [wgpu::ComputePipeline; 4],
    layout0: wgpu::BindGroupLayout,
    zbuf: wgpu::Buffer,
    idbuf: wgpu::Buffer,
    points_buf: wgpu::Buffer,
    bind0: wgpu::BindGroup,
    /// The geometry node's state: every chunk's points.
    points: Points,
    count: u32,
    max_splat: u32,
}

fn pixel_buffer(gpu: &Gpu, w: u32, h: u32) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("splat fb"), size: w as u64 * h as u64 * 4, usage: wgpu::BufferUsages::STORAGE, mapped_at_creation: false })
}

fn points_buffer(gpu: &Gpu, n: u64) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("points"), size: n.max(1) * 16, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })
}

impl Splat {
    pub(crate) fn new(p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked) -> anyhow::Result<(Core, Self)> {
        let opts: Options = p.options()?;
        for (slot, name) in [(Slot::Geometry, &opts.geometry), (Slot::Visibility, &opts.visibility), (Slot::Shading, &opts.shading)] {
            require(BACKEND, slot, name)?;
        }
        // The points cross from the CPU (geometry) to the GPU (visibility): the schedule's
        // transfer, its adapter `upload`.
        let schedule = g.schedule().map_err(|e| anyhow::anyhow!(e))?;
        if !schedule.transfers.iter().any(|e| e.port == "points" && g.node(&e.to).role == Role::Visibility) {
            anyhow::bail!("{BACKEND}: the points do not cross from the CPU to the GPU");
        }
        // The sites' walks (PLAN step 9c): shadow rays (the storage and traversal stages),
        // bounces their own or the same.
        let (walks, mut program, own, deps) = program(g, &opts.shading)?;
        let core = Core::new(
            p,
            devices,
            waker,
            g,
            CoreSpec {
                gpu: GpuNeeds { experimental: false, required_features: wgpu::Features::empty(), optional_features: wgpu::Features::TIMESTAMP_QUERY },
                walks: &walks,
                accel: crate::accel::AccelSpec::of(g, &walks)?,
                stages: wgpu::ShaderStages::COMPUTE,
                max_lod: 0,
                voxels: false,
                records: false,
                shading: &opts.shading,
                bounces: opts.bounces,
                spp: opts.spp,
                accumulate: opts.accumulate,
                shadows: opts.shadows,
                max_steps: 2048,
                pool_mb: opts.pool_mb,
                frames_in_flight: opts.frames_in_flight,
            },
        )?;
        core.trace_root(&mut program, "0u");
        let gpu = &core.gpu;
        let module = mr_gpu::program::compile(&gpu.device, "splat", &program, &own, &deps.iter().map(String::as_str).collect::<Vec<_>>());
        let mut entries = core.layout_entries();
        entries.extend([storage_entry(LANE_BINDING, true, core.stages), storage_entry(LANE_BINDING + 1, false, core.stages), storage_entry(LANE_BINDING + 2, false, core.stages)]);
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("splat"), entries: &entries });
        let pl = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("splat"), bind_group_layouts: &core.storages.layouts(&[Some(&layout0), Some(core.world.layout())]), immediate_size: 0 });
        let pipe = |e: &str| gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some(e), layout: Some(&pl), module: &module, entry_point: Some(e), compilation_options: Default::default(), cache: gpu.cache() });
        let pipes = splat::ENTRIES.map(pipe);
        let (points_buf, zbuf, idbuf) = (points_buffer(gpu, 1024), pixel_buffer(gpu, p.width, p.height), pixel_buffer(gpu, p.width, p.height));
        let bind0 = bind0(&core, &layout0, &points_buf, &zbuf, &idbuf);
        let lane = Self { pipes, layout0, zbuf, idbuf, points_buf, bind0, points: Points::default(), count: 0, max_splat: opts.max_splat.clamp(1, 64) };
        Ok((core, lane))
    }

    /// The points' transfer: every chunk's points into one GPU buffer (rebuilt after any
    /// change).
    fn upload(&mut self, core: &Core, all: Vec<Point>) {
        self.count = all.len() as u32;
        if (all.len() as u64) * 16 > self.points_buf.size() {
            self.points_buf = points_buffer(&core.gpu, all.len() as u64 * 3 / 2);
            self.bind0 = bind0(core, &self.layout0, &self.points_buf, &self.zbuf, &self.idbuf);
        }
        if !all.is_empty() {
            core.gpu.queue.write_buffer(&self.points_buf, 0, bytemuck::cast_slice(&all));
        }
    }
}

fn bind0(core: &Core, layout: &wgpu::BindGroupLayout, pts: &wgpu::Buffer, z: &wgpu::Buffer, id: &wgpu::Buffer) -> wgpu::BindGroup {
    let view = core.output.create_view(&Default::default());
    let mut entries = core.bind_entries(&view);
    entries.extend([
        wgpu::BindGroupEntry { binding: LANE_BINDING, resource: pts.as_entire_binding() },
        wgpu::BindGroupEntry { binding: LANE_BINDING + 1, resource: z.as_entire_binding() },
        wgpu::BindGroupEntry { binding: LANE_BINDING + 2, resource: id.as_entire_binding() },
    ]);
    core.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("splat"), layout, entries: &entries })
}

/// The shader: the shadow and bounce rays' walks (PLAN step 9c), the splat passes, the
/// shading component and the common deferred pass (the resolve) as the root.
fn program(g: &graph::Checked, shading: &str) -> anyhow::Result<(Walks, mr_gpu::program::Program, String, Vec<String>)> {
    let mut walks = Walks::of(g);
    let shadow = walks.site(g, g.site(Role::Shading, "shadow").ok_or_else(|| anyhow::anyhow!("gpu-splat: shadow rays are not bound"))?, &[2, 3, 4], false);
    let bounce = g.site(Role::Shading, "bounce").map_or(shadow, |s| walks.site(g, s, &[2, 3, 4], false));
    let shade = mr_components::shading::gpu_wgsl(shading).expect("validated");
    let own = format!("{}\n{}\n{shade}\n{}", splat::GPU_WGSL, ShadeState::wgsl(0, crate::STATE_BINDING), mr_components::shading::DEFERRED_WGSL);
    let (program, deps) = mr_components::shaders::sites_program(&walks, None, 0, bounce);
    Ok((walks, program, own, deps))
}

impl Lane for Splat {
    fn updated(&mut self, core: &Core, changed: &[IVec3]) -> anyhow::Result<()> {
        let world = &core.world;
        self.points.update(changed.to_vec(), |c| world.chunk(c));
        Ok(())
    }

    fn render(&mut self, core: &mut Core, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        if let Some(all) = self.points.changed() {
            self.upload(core, all);
        }
        let ticket = core.ring.begin(&core.gpu, sink)?;
        let start = Instant::now();
        let c = &req.camera;
        core.write_camera(c, req.mode, [self.count, self.max_splat]);
        core.accum.begin(&core.gpu, c, req.mode);
        let world_bg = core.world.prepare(&core.gpu, c).clone();
        let bound = core.storages.prepare(&core.gpu, &core.world);
        let mut enc = core.gpu.device.create_command_encoder(&Default::default());
        if core.build_accel(&mut enc, ticket, req) {
            self.bind0 = bind0(core, &self.layout0, &self.points_buf, &self.zbuf, &self.idbuf);
        }
        let groups = self.count.div_ceil(64);
        let (gx, gy) = (groups.min(65535), groups.div_ceil(65535));
        let (w, h) = (core.width, core.height);
        core.ring.stage(&mut enc, ticket, "splat", &core.stage_nodes[1]);
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("splat"), timestamp_writes: core.ring.compute_timestamps(ticket, true, false) });
            pass.set_bind_group(0, &self.bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            bound.compute(&mut pass);
            pass.set_pipeline(&self.pipes[0]);
            pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
            if groups > 0 {
                pass.set_pipeline(&self.pipes[1]);
                pass.dispatch_workgroups(gx, gy, 1);
                pass.set_pipeline(&self.pipes[2]);
                pass.dispatch_workgroups(gx, gy, 1);
            }
        }
        core.ring.stage(&mut enc, ticket, "shade", &core.stage_nodes[2]);
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("shade"), timestamp_writes: core.ring.compute_timestamps(ticket, false, true) });
            pass.set_bind_group(0, &self.bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            bound.compute(&mut pass);
            pass.set_pipeline(&self.pipes[3]);
            pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        }
        let (accel, stages) = core.accel_stats();
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            legend: None,
            counters: [("points".into(), self.count as f64), ("dropped_chunks".into(), core.world.dropped_chunks() as f64)].into_iter().chain(accel).chain(core.accum.counter(req.mode)).collect(),
            stages,
        };
        core.ring.submit(&core.gpu, enc, ticket, &core.output, req.frame_id, req.readback, stats);
        core.ring.poll(&core.gpu, sink, false)
    }

    fn set_param(&mut self, _core: &mut Core, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        (name == "max_splat").then(|| {
            self.max_splat = value.as_i64().unwrap_or(8).clamp(1, 64) as u32;
            Ok(())
        })
    }

    fn resize(&mut self, core: &Core) {
        self.zbuf = pixel_buffer(&core.gpu, core.width, core.height);
        self.idbuf = pixel_buffer(&core.gpu, core.width, core.height);
        self.bind0 = bind0(core, &self.layout0, &self.points_buf, &self.zbuf, &self.idbuf);
    }

    fn memory(&self, core: &Core) -> (u64, u64) {
        (self.points.bytes(), self.points_buf.size() + core.width as u64 * core.height as u64 * 8)
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
                for shading in names_on(BACKEND, Slot::Shading) {
                    let flat = mr_components::Composition { storage: st.into(), traversal: k.into(), shading: shading.into(), ..mr_components::Composition::new(BACKEND) };
                    let Ok(gr) = mr_components::graph::Graph::from_composition(&flat) else { continue };
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
}
