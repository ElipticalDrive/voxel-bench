//! The columns lane (`gpu-voxelspace`; components, IDEAS §12; PLAN step 13e3): storage
//! `heightmap`, visibility `columns`, shading `direct` or `pathtrace` (heightmap-gradient
//! normals on column tops; shadow and bounce rays walk the heightmap, `hm_trace`). The march
//! is the columns component's root (`mr_components::visibility::columns::GPU_WGSL`) in one
//! compute pass; shading inline in the march or deferred (`placement`, PLAN step 12c: the
//! march writes hit records, the common pass `shading/deferred.wgsl` shades them). The
//! heightmap is kept on the CPU, patched from the change log, its changed rows uploaded (the
//! change log's transfer to the GPU).

use std::time::Instant;

use glam::IVec3;
use mr_components::graph;
use mr_components::shading::state::{self as shade_state, ShadeState};
use mr_components::storage::heightmap::{self, Heightmap};
use mr_components::visibility::columns;
use mr_core::*;
use mr_gpu::voxels::{Shadows, shadows_param};
use mr_gpu::{GpuNeeds, wgpu};
use serde::Deserialize;

use crate::{Core, CoreSpec, LANE_BINDING, Lane, storage_entry};

pub(crate) fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu-voxelspace".into(),
        description: "Comanche-style Voxel Space: per-column front-to-back heightmap march with a y-buffer (2.5D, no overhangs)".into(),
        tags: vec!["gpu".into(), "compute".into(), "heightmap".into(), "2.5d".into(), "wgpu".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: mr_components::lighting_knobs(BACKEND, vec![
            ParamSpec::choice("storage", "Storage component", &mr_components::names_on(BACKEND, mr_components::Slot::Storage), "heightmap", false),
            ParamSpec::choice("visibility", "Visibility component", &mr_components::names_on(BACKEND, mr_components::Slot::Visibility), "columns", false),
            ParamSpec::choice("shading", "Shading component", &mr_components::names_on(BACKEND, mr_components::Slot::Shading), "direct", false),
            mr_components::placement_param(BACKEND, "inline"),
            shadows_param(),
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
    visibility: String,
    shading: String,
    placement: String,
    shadows: Shadows,
    bounces: u32,
    spp: u32,
    accumulate: bool,
    frames_in_flight: usize,
}

pub(crate) const BACKEND: &str = "gpu-voxelspace";

impl Default for Options {
    fn default() -> Self {
        Self { storage: "heightmap".into(), visibility: "columns".into(), shading: "direct".into(), placement: "inline".into(), shadows: Shadows::All, bounces: 1, spp: 1, accumulate: true, frames_in_flight: 3 }
    }
}

pub(crate) struct Columns {
    /// The heightmap storage (on the CPU) and its GPU copy; `hm_info` = mask, columns a walk
    /// may cross.
    hm: Heightmap,
    hm_buf: wgpu::Buffer,
    hm_info: wgpu::Buffer,
    /// The march (shading inline, or writing records), and the deferred shading pass.
    pipeline: wgpu::ComputePipeline,
    shade: Option<wgpu::ComputePipeline>,
    layout0: wgpu::BindGroupLayout,
    bind0: wgpu::BindGroup,
}

impl Columns {
    pub(crate) fn new(p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked) -> anyhow::Result<(Core, Self)> {
        let opts: Options = p.options()?;
        use mr_components::{Slot, require};
        for (slot, name) in [(Slot::Storage, &opts.storage), (Slot::Visibility, &opts.visibility), (Slot::Shading, &opts.shading)] {
            require(BACKEND, slot, name)?;
        }
        let deferred = mr_components::parse_placement(&opts.placement)?;
        // The voxel world holds the chunks, the texture atlas and the lighting parameters only.
        let core = Core::new(
            p,
            devices,
            waker,
            g,
            CoreSpec {
                gpu: GpuNeeds { experimental: false, required_features: wgpu::Features::empty(), optional_features: wgpu::Features::TIMESTAMP_QUERY },
                walks: &mr_components::shaders::Walks::of(g),
                accel: None,
                stages: wgpu::ShaderStages::COMPUTE,
                max_lod: 0,
                voxels: false,
                records: deferred,
                shading: &opts.shading,
                bounces: opts.bounces,
                spp: opts.spp,
                accumulate: opts.accumulate,
                shadows: opts.shadows,
                max_steps: 1,
                pool_mb: 64,
                frames_in_flight: opts.frames_in_flight,
            },
        )?;
        let gpu = &core.gpu;
        let hm = Heightmap::new(p.view_radius_chunks);
        let hm_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("heightmap"), size: hm.hm.len() as u64 * 4, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let hm_info = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("heightmap info"), size: 16, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        gpu.queue.write_buffer(&hm_info, 0, bytemuck::cast_slice(&hm.info()));
        // Shadow and bounce rays walk the heightmap (its `ctrace`).
        let (program, root, deps) = program(&opts.shading, deferred, g.tone());
        let module = mr_gpu::program::compile(&gpu.device, "voxelspace", &program, &root, &deps);
        let mut entries = core.layout_entries();
        entries.push(storage_entry(LANE_BINDING, true, core.stages));
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: LANE_BINDING + 1,
            visibility: core.stages,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        });
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("voxelspace"), entries: &entries });
        let pl = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("voxelspace"), bind_group_layouts: &[Some(&layout0), Some(core.world.layout())], immediate_size: 0 });
        let pipe = |e: &str| gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some(e), layout: Some(&pl), module: &module, entry_point: Some(e), compilation_options: Default::default(), cache: gpu.cache() });
        let (pipeline, shade) = (pipe("main"), deferred.then(|| pipe("shade")));
        let bind0 = bind0(&core, &layout0, &hm_buf, &hm_info);
        Ok((core, Self { hm, hm_buf, hm_info, pipeline, shade, layout0, bind0 }))
    }
}

fn bind0(core: &Core, layout: &wgpu::BindGroupLayout, hm: &wgpu::Buffer, hm_info: &wgpu::Buffer) -> wgpu::BindGroup {
    let view = core.output.create_view(&Default::default());
    let mut entries = core.bind_entries(&view);
    entries.extend([wgpu::BindGroupEntry { binding: LANE_BINDING, resource: hm.as_entire_binding() }, wgpu::BindGroupEntry { binding: LANE_BINDING + 1, resource: hm_info.as_entire_binding() }]);
    core.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("voxelspace"), layout, entries: &entries })
}

impl Lane for Columns {
    /// The heightmap's patch from the change log and its transfer to the GPU: its changed
    /// rows.
    fn updated(&mut self, core: &Core, changed: &[IVec3]) -> anyhow::Result<()> {
        let world = &core.world;
        let tiles = self.hm.apply(changed.iter().copied(), |pos| world.chunk(pos));
        for &c in &tiles {
            for (start, len) in self.hm.dirty_rows(c) {
                core.gpu.queue.write_buffer(&self.hm_buf, start as u64 * 4, bytemuck::cast_slice(&self.hm.hm[start..start + len]));
            }
        }
        core.gpu.queue.write_buffer(&self.hm_info, 0, bytemuck::cast_slice(&self.hm.info()));
        Ok(())
    }

    fn render(&mut self, core: &mut Core, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let (width, height) = (core.width, core.height);
        let ticket = core.ring.begin(&core.gpu, sink)?;
        let start = Instant::now();
        let c = &req.camera;
        core.write_camera(c, req.mode, [0, self.hm.march_cells()]);
        core.accum.begin(&core.gpu, c, req.mode);
        let world_bg = core.world.prepare(&core.gpu, c).clone();
        let mut enc = core.gpu.device.create_command_encoder(&Default::default());
        let deferred = self.shade.is_some();
        if deferred {
            core.ring.stage(&mut enc, ticket, "columns", &core.stage_nodes[1]);
        } else {
            core.ring.stage(&mut enc, ticket, "columns+shade", &core.stage_nodes[0]);
        }
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("voxelspace"), timestamp_writes: core.ring.compute_timestamps(ticket, true, !deferred) });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.dispatch_workgroups(width.div_ceil(64), 1, 1);
        }
        if let Some(shade) = &self.shade {
            core.ring.stage(&mut enc, ticket, "shade", &core.stage_nodes[2]);
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("deferred shade"), timestamp_writes: core.ring.compute_timestamps(ticket, false, true) });
            pass.set_pipeline(shade);
            pass.set_bind_group(0, &self.bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            legend: None,
            counters: self.hm.counters().into_iter().chain(core.accum.counter(req.mode)).collect(),
            stages: Vec::new(),
        };
        core.ring.submit(&core.gpu, enc, ticket, &core.output, req.frame_id, req.readback, stats);
        core.ring.poll(&core.gpu, sink, false)
    }

    fn set_param(&mut self, _core: &mut Core, _name: &str, _value: &ParamValue) -> Option<anyhow::Result<()>> {
        None
    }

    fn resize(&mut self, core: &Core) {
        self.bind0 = bind0(core, &self.layout0, &self.hm_buf, &self.hm_info);
    }

    fn memory(&self, _core: &Core) -> (u64, u64) {
        let hm = self.hm.hm.len() as u64 * 4;
        (hm, hm)
    }
}

/// The shader: the heightmap storage (group 2) is its own kernel (`ctrace`; storage and walk
/// in one, IDEAS §18's exceptions), the ray services through it, the column march and the
/// shading component as the root, the post's `tone` in its output; with `deferred`, the march writes records and
/// the common deferred pass shades them.
fn program(shading: &str, deferred: bool, tone: mr_components::post::Tone) -> (mr_gpu::program::Program, String, [&'static str; 5]) {
    use mr_gpu::program::{COMMON, LIGHTING, Program, VOXELS};
    let mut p = Program::world(Some("heightmap::rays"));
    mr_components::post::add_to(tone, &mut p);
    p.add("heightmap", heightmap::gpu_wgsl(0, LANE_BINDING), &[VOXELS]);
    mr_components::shaders::rays(&mut p, "heightmap::rays", "heightmap");
    let shade = mr_components::shading::gpu_wgsl(shading).expect("validated");
    let place = if deferred { format!("{}\n{}", columns::DEFERRED_WGSL, mr_components::shading::DEFERRED_PACKED_WGSL) } else { String::new() };
    if deferred {
        p.define("DEFERRED");
    }
    let root = format!("{}\n{}\n{shade}\n{place}", ShadeState::wgsl(0, crate::STATE_BINDING), columns::GPU_WGSL);
    (p, root, ["heightmap", "heightmap::rays", LIGHTING, VOXELS, COMMON])
}

#[cfg(test)]
mod tests {
    /// The shader composes from its modules and validates for every shading and placement
    /// (no GPU needed).
    #[test]
    fn programs_compose() {
        use mr_gpu::wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        for shading in mr_components::names_on("gpu-voxelspace", mr_components::Slot::Shading) {
            for deferred in [false, true] {
                let (p, root, deps) = super::program(shading, deferred, mr_components::post::Tone::Aces);
                let m = p.naga(&root, &deps).unwrap_or_else(|e| panic!("{shading} {deferred}: {e}"));
                Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("{shading} {deferred}: {e:?}"));
            }
        }
    }
}
