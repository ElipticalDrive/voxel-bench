//! Columns on the node executor (`columns-gpu`; components): storage
//! `heightmap`, visibility `columns`, shading `direct` or `pathtrace` (heightmap-gradient
//! normals on column tops; shadow and bounce rays walk the heightmap, `hm_trace`: the
//! `heightfield` kernel the graph binds to the shading's ray ports). The march
//! is the columns component's root (`mr_components::visibility::columns::GPU_WGSL`) in one
//! compute pass; shading inline in the march or deferred (`placement`: the
//! march writes hit records, the common pass `shading/deferred.wgsl` shades them). The
//! heightmap is kept on the CPU, patched from the change log, its changed rows uploaded (the
//! change log's transfer to the GPU). The march and the deferred shading are nodes sharing
//! the [`Columns`] region.

use mr_gpu::Pipelines as _;
use glam::IVec3;
use mr_components::graph::{self, Role};
use mr_components::shading::state::{self as shade_state, ShadeState};
use mr_components::storage::heightmap::{self, Heightmap};
use mr_components::visibility::columns;
use mr_core::*;
use mr_gpu::voxels::{Shadows, shadows_param};
use mr_gpu::{GpuNeeds, wgpu};
use serde::Deserialize;

use crate::exec::{self, Cx, Node, Region, State};
use crate::{Core, CoreSpec, REGION_BINDING, storage_entry};

pub(crate) fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu".into(),
        description: "Comanche-style Voxel Space: per-column front-to-back heightmap march with a y-buffer (2.5D, no overhangs)".into(),
        tags: vec!["gpu".into(), "compute".into(), "heightmap".into(), "2.5d".into(), "wgpu".into()],
        features: Features::of(&[Feature::Lit]).lacking(Feature::Volume, "storage `heightmap` is 2.5D: a column is solid below its top block"),
        params: REGION.lighting_knobs(vec![
            REGION.placement_param("inline"),
            shadows_param(),
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
    frames_in_flight: usize,
}

pub(crate) const REGION: &mr_components::graph::Builds = &mr_components::graph::GPU_COLUMNS;

impl Default for Options {
    fn default() -> Self {
        Self { frames_in_flight: 3 }
    }
}

/// The shading node's knobs.
#[derive(Deserialize)]
#[serde(default)]
struct ShadingKnobs {
    placement: String,
    shadows: Shadows,
}

impl Default for ShadingKnobs {
    fn default() -> Self {
        Self { placement: "inline".into(), shadows: Shadows::All }
    }
}

/// The region the columns' nodes share: the heightmap, the program, group 0, the frame's world.
pub(crate) struct Columns {
    /// The heightmap storage (on the CPU) and its GPU copy; `hm_info` = mask, columns a walk
    /// may cross.
    hm: Heightmap,
    hm_buf: wgpu::Buffer,
    hm_info: wgpu::Buffer,
    /// The programs per render mode (unlit, lit), each compiled when a frame first asks
    /// for its mode; `mode` = this frame's.
    variants: [Option<Variant>; 2],
    mode: usize,
    /// What the programs are made of: the shading component, `placement = deferred`, the tone.
    shading: String,
    deferred: bool,
    tone: mr_components::post::Tone,
    pl: wgpu::PipelineLayout,
    layout0: wgpu::BindGroupLayout,
    bind0: wgpu::BindGroup,
    /// This frame's world bind group.
    world: Option<wgpu::BindGroup>,
}

/// One render mode's programs: the march (shading inline, or writing records) and the
/// deferred shading pass.
struct Variant {
    march: wgpu::ComputePipeline,
    shade: Option<wgpu::ComputePipeline>,
}

impl Columns {
    fn variant(&self, core: &Core, lit: bool) -> Variant {
        let gpu = &core.gpu;
        let (mut program, root, deps) = program(&self.shading, self.deferred, lit, self.tone);
        core.bind_program(&mut program, "0u");
        let module = mr_gpu::program::compile(&gpu.device, if lit { "voxelspace lit" } else { "voxelspace" }, &program, &root, &deps);
        let pipe = |e: &str| gpu.device.compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some(e), layout: Some(&self.pl), module: &module, entry_point: Some(e), compilation_options: Default::default(), cache: gpu.cache() });
        Variant { march: pipe("main"), shade: self.deferred.then(|| pipe("shade")) }
    }
}

impl Columns {
    fn new(p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked) -> anyhow::Result<(Core, Self)> {
        let opts: Options = p.options()?;
        let shading = &g.node(g.shading()).component;
        let k: ShadingKnobs = g.knobs(g.shading()).map_err(anyhow::Error::msg)?;
        let deferred = mr_components::parse_placement(&k.placement)?;
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
                hits: deferred.then_some(exec::Form::Records),
                shading,
                shadows: k.shadows,
                max_steps: 1,
                pool_mb: 64,
                frames_in_flight: opts.frames_in_flight,
            },
        )?;
        let gpu = &core.gpu;
        let hm = Heightmap::new(p.view_radius_chunks);
        let hm_buf = gpu.storage("heightmap", hm.hm.len() as u64 * 4, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, || format!("the world's columns ({} of them, 4 bytes each)", hm.hm.len()));
        let hm_info = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("heightmap info"), size: 16, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        gpu.queue.write_buffer(&hm_info, 0, bytemuck::cast_slice(&hm.info()));
        let mut entries = core.layout_entries();
        entries.push(storage_entry(REGION_BINDING, true, core.stages));
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: REGION_BINDING + 1,
            visibility: core.stages,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        });
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("voxelspace"), entries: &entries });
        let pl = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("voxelspace"), bind_group_layouts: &[Some(&layout0), Some(core.world.layout())], immediate_size: 0 });
        let bind0 = bind0(&core, &layout0, &hm_buf, &hm_info);
        Ok((core, Self { hm, hm_buf, hm_info, variants: [None, None], mode: 0, shading: shading.clone(), deferred, tone: g.tone(), pl, layout0, bind0, world: None }))
    }
}

fn bind0(core: &Core, layout: &wgpu::BindGroupLayout, hm: &wgpu::Buffer, hm_info: &wgpu::Buffer) -> wgpu::BindGroup {
    let view = core.output.create_view(&Default::default());
    let mut entries = core.bind_entries(&view);
    entries.extend([wgpu::BindGroupEntry { binding: REGION_BINDING, resource: hm.as_entire_binding() }, wgpu::BindGroupEntry { binding: REGION_BINDING + 1, resource: hm_info.as_entire_binding() }]);
    core.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("voxelspace"), layout, entries: &entries })
}

impl State for Columns {
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

impl Region for Columns {
    fn compile(&mut self, core: &mut Core, req: &FrameRequest) -> anyhow::Result<()> {
        let lit = req.mode == RenderMode::Lit;
        self.mode = lit as usize;
        if self.variants[self.mode].is_none() {
            self.variants[self.mode] = Some(self.variant(core, lit));
        }
        Ok(())
    }

    fn setup(&mut self, core: &mut Core, req: &FrameRequest) {
        let c = &req.camera;
        core.write_camera(c, req.mode, [0, self.hm.march_cells()]);
        core.accum.begin(&core.gpu, c, req.mode);
        self.world = Some(core.world.prepare(&core.gpu, c).clone());
    }

    fn stats(&self, core: &Core, req: &FrameRequest) -> (Vec<(String, f64)>, Option<Legend>, Vec<StageTime>) {
        (self.hm.counters().into_iter().chain(core.accum.counter(req.mode)).collect(), None, Vec::new())
    }

    fn finish(&mut self, _: &Core, _: &mut wgpu::CommandEncoder, _: mr_gpu::Ticket) {}
}

/// The march (with `placement = inline` the shading fused into it).
struct Visibility;
/// The deferred shading pass.
struct Shading;

impl Node<Columns> for Visibility {
    fn run(&mut self, cx: &mut Cx<Columns>) {
        let (core, r, enc, ticket) = (&mut *cx.core, &*cx.region, &mut *cx.enc, cx.ticket);
        let deferred = r.deferred;
        if deferred {
            core.ring.stage(enc, ticket, "columns", &core.stage_nodes[1]);
        } else {
            core.ring.stage(enc, ticket, "columns+shade", &core.stage_nodes[0]);
        }
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("voxelspace"), timestamp_writes: core.ring.compute_timestamps(ticket, true, !deferred) });
        pass.set_pipeline(&r.variants[r.mode].as_ref().expect("compiled").march);
        pass.set_bind_group(0, &r.bind0, &[]);
        pass.set_bind_group(1, r.world.as_ref().expect("set up"), &[]);
        pass.dispatch_workgroups(core.width.div_ceil(64), 1, 1);
    }
}

impl Node<Columns> for Shading {
    fn run(&mut self, cx: &mut Cx<Columns>) {
        let (core, r, enc, ticket) = (&mut *cx.core, &*cx.region, &mut *cx.enc, cx.ticket);
        let Some(shade) = r.variants[r.mode].as_ref().and_then(|v| v.shade.as_ref()) else { return };
        core.ring.stage(enc, ticket, "shade", &core.stage_nodes[2]);
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("deferred shade"), timestamp_writes: core.ring.compute_timestamps(ticket, false, true) });
        pass.set_pipeline(shade);
        pass.set_bind_group(0, &r.bind0, &[]);
        pass.set_bind_group(1, r.world.as_ref().expect("set up"), &[]);
        pass.dispatch_workgroups(core.width.div_ceil(8), core.height.div_ceil(8), 1);
    }
}

/// The columns executor for `g`: the march and the deferred shading as nodes (the storage
/// has no pass: the heightmap is patched and uploaded with the world's changes).
pub(crate) fn create(p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked) -> anyhow::Result<Box<dyn Renderer>> {
    let passes = exec::passes(g, crate::home_slot(g), |role| -> Option<Box<dyn Node<Columns>>> {
        match role {
            Role::Visibility => Some(Box::new(Visibility)),
            Role::Shading => Some(Box::new(Shading)),
            _ => None,
        }
    })?;
    let (core, region) = Columns::new(p, devices, waker, g)?;
    exec::run(core, region, passes)
}

/// The shader: the heightmap storage (group 2) is its own kernel (`ctrace`; storage and walk
/// in one), the ray services through it, the column march and the
/// shading component as the root, the post's `tone` in its output; with `deferred`, the march writes records and
/// the common deferred pass shades them; `lit` (shader def `LIT`) the lit frames' program.
fn program(shading: &str, deferred: bool, lit: bool, tone: mr_components::post::Tone) -> (mr_gpu::program::Program, String, [&'static str; 5]) {
    use mr_gpu::program::{COMMON, LIGHTING, Program, VOXELS};
    let mut p = Program::world(Some("heightmap::rays"));
    mr_components::post::add_to(tone, &mut p);
    p.add("heightmap", heightmap::gpu_wgsl(0, REGION_BINDING), &[VOXELS]);
    mr_components::shaders::rays(&mut p, "heightmap::rays", "heightmap");
    let shade = mr_components::shading::gpu_wgsl(shading).expect("validated");
    let place = if deferred { format!("{}{}\n{}", crate::hits_wgsl(), columns::DEFERRED_WGSL, mr_components::shading::DEFERRED_PACKED_WGSL) } else { String::new() };
    if deferred {
        p.define("DEFERRED");
    }
    if lit {
        p.define("LIT");
    }
    let root = format!("{}\n{}\n{shade}\n{place}", ShadeState::wgsl(0, crate::STATE_BINDING), columns::GPU_WGSL);
    (p, root, ["heightmap", "heightmap::rays", LIGHTING, VOXELS, COMMON])
}

#[cfg(test)]
mod tests {
    /// The shader composes from its modules and validates for every shading, placement and mode
    /// (no GPU needed).
    #[test]
    fn programs_compose() {
        use mr_gpu::wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        let shadings = mr_components::graph::shipped("columns-gpu").unwrap().choices("shading");
        assert!(shadings.len() > 1, "{shadings:?}");
        for shading in shadings {
            for (deferred, lit) in [(false, false), (false, true), (true, false), (true, true)] {
                let (p, root, deps) = super::program(shading, deferred, lit, mr_components::post::Tone::Aces);
                let m = p.naga(&root, &deps).unwrap_or_else(|e| panic!("{shading} {deferred} {lit}: {e}"));
                Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("{shading} {deferred} {lit}: {e:?}"));
            }
        }
    }
}
