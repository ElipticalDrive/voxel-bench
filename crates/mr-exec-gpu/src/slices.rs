//! Plane sweeping on the node executor (`slices-gpu`; components): storage (any GPU
//! storage with voxels), visibility `slices`, shading in the common deferred pass, its shadow
//! (and bounce) rays walked.
//!
//! The visibility node is one raster pass with no vertex buffer: every integer plane of the
//! loaded region along each axis is an instance of a four-vertex strip spanning the region
//! (`mr_components::visibility::slices`), drawn nearest first; the fragment shader looks up
//! the two cells on either side of its plane in the storage and keeps the pixel where the far
//! one is solid and the near one air, writing that face's `pack_hit` record into the hits
//! edge (a render target of records, as `gbuffer`'s) behind a reverse-Z depth test. The
//! shading node is the common deferred pass over that target. The region keeps the depth
//! target and the plane count; the storages are the core's.

use mr_gpu::Pipelines as _;
use glam::IVec3;
use mr_components::graph::{self, Role};
use mr_components::shaders::Walks;
use mr_components::shading::state::{self as shade_state, ShadeState};
use mr_components::storage::gpu_data::Bound;
use mr_components::visibility::slices;
use mr_core::*;
use mr_gpu::voxels::{Shadows, shadows_param};
use mr_gpu::{Gpu, GpuNeeds, wgpu};
use serde::Deserialize;

use crate::exec::{self, Cx, Node, Region, State};
use crate::{Core, CoreSpec, REGION_BINDING};

pub(crate) fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu".into(),
        description: "GPU plane sweeping (raster): every integer plane of the loaded region drawn as a quad, nearest first; a fragment is kept where its plane holds a block face looking at the camera (two storage lookups) and writes a hit record; deferred shading".into(),
        tags: vec!["gpu".into(), "raster".into(), "slices".into(), "planes".into(), "modular".into(), "wgpu".into()],
        features: Features::of(&[Feature::Lit, Feature::Volume]),
        params: REGION.lighting_knobs(vec![
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

/// The shading node's knobs.
#[derive(Deserialize)]
#[serde(default)]
struct ShadingKnobs {
    shadows: Shadows,
}

impl Default for ShadingKnobs {
    fn default() -> Self {
        Self { shadows: Shadows::All }
    }
}

pub(crate) const REGION: &mr_components::graph::Builds = &mr_components::graph::GPU_SLICES;

/// The region the planes' pass and the deferred pass share: their pipelines, group 0 for each
/// (the draw's without the output, the shading's with it and the records as a texture), the
/// depth target, this frame's plane count, world and storages' bind groups.
pub(crate) struct Slices {
    draw: wgpu::RenderPipeline,
    shade: wgpu::ComputePipeline,
    layout_draw: wgpu::BindGroupLayout,
    layout_shade: wgpu::BindGroupLayout,
    bind_draw: wgpu::BindGroup,
    bind_shade: wgpu::BindGroup,
    depth: wgpu::TextureView,
    planes: u32,
    world: Option<wgpu::BindGroup>,
    bound: Bound,
}

fn depth_target(gpu: &Gpu, w: u32, h: u32) -> wgpu::TextureView {
    gpu.device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("slices depth"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

/// Group 0 of the planes' pass (no output: the records are its render target) and of the
/// deferred pass (the output, and the records as a texture at the region's first binding).
fn binds(core: &Core, draw: &wgpu::BindGroupLayout, shade: &wgpu::BindGroupLayout) -> (wgpu::BindGroup, wgpu::BindGroup) {
    let dev = &core.gpu.device;
    let bind_draw = dev.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("slices draw"), layout: draw, entries: &core.bind_entries_with(None) });
    let view = core.output.create_view(&Default::default());
    let mut entries = core.bind_entries(&view);
    entries.push(core.edges.hits().expect("the planes' records cross to the deferred pass").bind_entry(REGION_BINDING));
    let bind_shade = dev.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("slices shade"), layout: shade, entries: &entries });
    (bind_draw, bind_shade)
}

impl Slices {
    fn new(p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked) -> anyhow::Result<(Core, Self)> {
        let opts: Options = p.options()?;
        let shading = &g.node(g.shading()).component;
        let k: ShadingKnobs = g.knobs(g.shading()).map_err(anyhow::Error::msg)?;
        let (walks, mut program, root, deps) = program(g, shading)?;
        let core = Core::new(
            p,
            devices,
            waker,
            g,
            CoreSpec {
                gpu: GpuNeeds { experimental: false, required_features: wgpu::Features::empty(), optional_features: wgpu::Features::TIMESTAMP_QUERY },
                walks: &walks,
                accel: None,
                stages: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
                max_lod: 0,
                voxels: false,
                hits: Some(exec::Form::Image(wgpu::TextureFormat::Rg32Uint)),
                shading,
                shadows: k.shadows,
                max_steps: 2048,
                pool_mb: opts.pool_mb,
                frames_in_flight: opts.frames_in_flight,
            },
        )?;
        core.bind_program(&mut program, "0u");
        let gpu = &core.gpu;
        let dev = &gpu.device;
        let module = mr_gpu::program::compile(dev, "slices", &program, &root, &deps.iter().map(String::as_str).collect::<Vec<_>>());
        let layout_draw = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("slices draw"), entries: &core.layout_entries_with(false) });
        let mut entries = core.layout_entries();
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: REGION_BINDING,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Uint, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        });
        let layout_shade = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("slices shade"), entries: &entries });
        let world = core.world.layout();
        let pl_draw = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("slices draw"), bind_group_layouts: &core.storages.layouts(&[Some(&layout_draw), Some(world)]), immediate_size: 0 });
        let pl_shade = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("slices shade"), bind_group_layouts: &core.storages.layouts(&[Some(&layout_shade), Some(world)]), immediate_size: 0 });
        let targets = [Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rg32Uint, blend: None, write_mask: wgpu::ColorWrites::ALL })];
        let draw = dev.render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("slices"),
            layout: Some(&pl_draw),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            // A plane is seen from both sides.
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { format: wgpu::TextureFormat::Depth32Float, depth_write_enabled: Some(true), depth_compare: Some(wgpu::CompareFunction::Greater), stencil: Default::default(), bias: Default::default() }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs"), compilation_options: Default::default(), targets: &targets }),
            multiview_mask: None,
            cache: gpu.cache(),
        });
        let shade = dev.compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some("slices shade"), layout: Some(&pl_shade), module: &module, entry_point: Some("shade"), compilation_options: Default::default(), cache: gpu.cache() });
        let (bind_draw, bind_shade) = binds(&core, &layout_draw, &layout_shade);
        let depth = depth_target(gpu, p.width, p.height);
        let slices = Self { draw, shade, layout_draw, layout_shade, bind_draw, bind_shade, depth, planes: 0, world: None, bound: Bound(Vec::new()) };
        Ok((core, slices))
    }
}

/// The shader: the storage the planes look up (its module the root's nearest), the shadow
/// and bounce rays' walks, the planes' pass, the shading component and the common deferred
/// pass over the packed records.
fn program(g: &graph::Checked, shading: &str) -> anyhow::Result<(Walks, mr_gpu::program::Program, String, Vec<String>)> {
    let mut walks = Walks::of(g);
    let looked_up = g.input(Role::Visibility, "storage").ok_or_else(|| anyhow::anyhow!("slices-gpu: the visibility node's storage is not bound"))?;
    let st = walks.storage(g, looked_up, &[2, 3]);
    // No shadow site: no shadow rays.
    let shadow = g.site(Role::Shading, "shadow").map_or(Walks::NO_RAYS, |s| walks.site(g, s, &[2, 3], false));
    let bounce = g.site(Role::Shading, "bounce").map_or(shadow, |s| walks.site(g, s, &[2, 3], false));
    let shade = mr_components::shading::gpu_wgsl(shading).expect("validated");
    let root = format!("{}\n{}\n{shade}\n{}", slices::GPU_WGSL, ShadeState::wgsl(0, crate::STATE_BINDING), mr_components::shading::DEFERRED_PACKED_WGSL);
    let (program, rest) = mr_components::shaders::sites_program(&walks, None, shadow, bounce);
    let deps = std::iter::once(walks.storage_path(st)).chain(rest).collect();
    Ok((walks, program, root, deps))
}

impl State for Slices {
    fn updated(&mut self, _core: &Core, _changed: &[IVec3]) -> anyhow::Result<()> {
        Ok(())
    }

    fn set_param(&mut self, _core: &mut Core, _name: &str, _value: &ParamValue) -> Option<anyhow::Result<()>> {
        None
    }

    fn resize(&mut self, core: &Core) {
        self.depth = depth_target(&core.gpu, core.width, core.height);
        (self.bind_draw, self.bind_shade) = binds(core, &self.layout_draw, &self.layout_shade);
    }

    fn memory(&self, core: &Core) -> (u64, u64) {
        (0, core.width as u64 * core.height as u64 * 4)
    }
}

impl Region for Slices {
    fn compile(&mut self, _core: &mut Core, _req: &FrameRequest) -> anyhow::Result<()> {
        Ok(())
    }

    /// The world first: its loaded region (as of this frame) sets the plane count.
    fn setup(&mut self, core: &mut Core, req: &FrameRequest) {
        let c = &req.camera;
        self.world = Some(core.world.prepare(&core.gpu, c).clone());
        self.planes = core.world.bounds().map_or(0, |(lo, hi)| slices::planes(lo.to_array(), hi.to_array()));
        core.write_camera(c, req.mode, [self.planes, 0]);
        core.accum.begin(&core.gpu, c, req.mode);
        self.bound = core.storages.prepare(&core.gpu, &core.world);
    }

    fn stats(&self, core: &Core, req: &FrameRequest) -> (Vec<(String, f64)>, Option<Legend>, Vec<StageTime>) {
        let counters = [("planes".into(), self.planes as f64)].into_iter().chain(core.accum.counter(req.mode)).collect();
        (counters, None, Vec::new())
    }

    fn finish(&mut self, _: &Core, _: &mut wgpu::CommandEncoder, _: mr_gpu::Ticket) {}
}

/// The planes' raster pass, writing the records.
struct Visibility;
/// The common deferred pass over them.
struct Shading;

impl Node<Slices> for Visibility {
    fn run(&mut self, cx: &mut Cx<Slices>) {
        let (core, r, enc, ticket) = (&mut *cx.core, &*cx.region, &mut *cx.enc, cx.ticket);
        core.ring.stage(enc, ticket, "slices", &core.stage_nodes[1]);
        let target = core.edges.hits().expect("the planes' records").view();
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("slices"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: target, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &r.depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Discard }), stencil_ops: None }),
            timestamp_writes: core.ring.render_timestamps(ticket, true, false),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&r.draw);
        pass.set_bind_group(0, &r.bind_draw, &[]);
        pass.set_bind_group(1, r.world.as_ref().expect("set up"), &[]);
        r.bound.render(&mut pass);
        pass.draw(0..4, 0..r.planes);
    }
}

impl Node<Slices> for Shading {
    fn run(&mut self, cx: &mut Cx<Slices>) {
        let (core, r, enc, ticket) = (&mut *cx.core, &*cx.region, &mut *cx.enc, cx.ticket);
        core.ring.stage(enc, ticket, "shade", &core.stage_nodes[2]);
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("slices shade"), timestamp_writes: core.ring.compute_timestamps(ticket, false, true) });
        pass.set_pipeline(&r.shade);
        pass.set_bind_group(0, &r.bind_shade, &[]);
        pass.set_bind_group(1, r.world.as_ref().expect("set up"), &[]);
        r.bound.compute(&mut pass);
        pass.dispatch_workgroups(core.width.div_ceil(8), core.height.div_ceil(8), 1);
    }
}

/// The plane-sweeping executor for `g`: the planes' pass and the deferred pass as nodes (the
/// storages are the core's, uploaded with the world's changes).
pub(crate) fn create(p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked) -> anyhow::Result<Box<dyn Renderer>> {
    let passes = exec::passes(g, crate::home_slot(g), |role| -> Option<Box<dyn Node<Slices>>> {
        match role {
            Role::Visibility => Some(Box::new(Visibility)),
            Role::Shading => Some(Box::new(Shading)),
            _ => None,
        }
    })?;
    let (core, region) = Slices::new(p, devices, waker, g)?;
    exec::run(core, region, passes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every storage, walk and shading the graph may take composes and validates (no GPU
    /// needed), the planes' and the deferred pass's entries in it.
    #[test]
    fn programs_compose() {
        use mr_components::graph::Change;
        use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        let base = mr_components::graph::shipped("slices-gpu").unwrap();
        let mut n = 0;
        for st in base.choices("storage") {
            for k in base.choices("kernel") {
                for shading in base.choices("shading") {
                    let bounce = shade_state::stateful(shading).then(|| Change::bind("shading", "bounce", "kernel"));
                    let flat: Vec<Change> = [("storage", st), ("kernel", k), ("shading", shading)].into_iter().map(|(n, c)| Change::component(n, c)).chain(bounce).collect();
                    let Ok(gr) = base.with_edits(&flat) else { continue };
                    if gr.validate().is_err() {
                        continue;
                    }
                    let (_, p, root, deps) = program(&gr.check().unwrap(), shading).unwrap();
                    let m = p.naga(&root, &deps.iter().map(String::as_str).collect::<Vec<_>>()).unwrap_or_else(|e| panic!("{flat:?}: {e}"));
                    Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("{flat:?}: {e:?}"));
                    for e in ["vs", "fs", "shade"] {
                        assert!(m.entry_points.iter().any(|ep| ep.name == e), "{flat:?}: no entry `{e}`");
                    }
                    n += 1;
                }
            }
        }
        assert!(n > 20, "only {n} compositions composed");
    }
}
