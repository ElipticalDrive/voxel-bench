//! The faces lane: every pipeline whose image is the
//! shared face drawer's (`draw`: raster, visbuffer or gbuffer) over a set of block faces, one
//! lane for the three face sources that can feed the drawer's `faces`:
//! - `gpu-raster`: meshing ([`mesh`]): the geometry component meshes changed chunks on the
//!   CPU (view-independent: every exposed face), uploaded per chunk (the schedule's transfer
//!   of the faces from the CPU to the GPU), culled per chunk and face direction, drawn with
//!   multi-draw-indirect; with meshes (mesh mode) in the same depth buffer;
//! - `gpu-discover`: discovery on the GPU ([`gpu`]): the sampler (`visibility` quadtree, with
//!   hole detection `holes = coverage`) finds the faces the camera sees and keeps them in a
//!   GPU hash set;
//! - `cpu-discover`: discovery on the CPU ([`cpu`]): the sampler (quadtree or cluster) over a
//!   CPU storage, its face records uploaded to the GPU, a frame ahead when the drawer's faces
//!   arrive `prev`.
//!
//! Which source runs is the graph's: meshing when the drawer's faces come from the `faces`
//! visibility (every meshed face), else discovery on the visibility node's device. The rest is
//! the lane's ([`Draw`]) over the executor's core: the face drawer compiled over the shared
//! frame (the core's group 0: its camera uniform, shading state, acceleration structures; the
//! source's two buffers at the lane's bindings) and the source's WGSL, the depth buffer.

use glam::IVec3;
use mr_components::draw::{self, drawer::{DrawMode, DrawNodes, DrawerSpec, FaceDrawer}};
use mr_components::graph::{self, Device, Role};
use mr_components::shaders::Walks;
use mr_components::shading::state::{self as shade_state, ShadeState};
use mr_core::*;
use mr_gpu::voxels::Shadows;
use mr_gpu::{Gpu, GpuNeeds, wgpu};

use crate::{Core, CoreSpec, Lane};

pub(crate) mod cpu;
pub(crate) mod gpu;
pub(crate) mod mesh;

/// The lane for graph `g`: meshing when the drawer's faces are every meshed face (the `faces`
/// visibility), else discovery on the visibility node's device (on the CPU in `cpu_world`'s
/// storage when a compiled pipeline names one).
pub(crate) fn create(backend: &str, p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked, cpu_world: Option<fn() -> mr_components::visibility::discover::grid::CpuWorld>) -> anyhow::Result<Box<dyn Renderer>> {
    let faces = g.input(Role::Draw, "faces").ok_or_else(|| anyhow::anyhow!("{backend}: the drawer's faces are not bound"))?;
    if g.node(faces).component == "faces" {
        return run(mesh::Meshing::new(p, devices, waker, g)?);
    }
    let vis = g.find(Role::Visibility).ok_or_else(|| anyhow::anyhow!("{backend}: no visibility node"))?;
    match g.devices[vis] {
        Device::Cpu => run(cpu::CpuSampler::new(p, devices, waker, g, cpu_world)?),
        Device::Gpu(_) => run(gpu::GpuSampler::new(p, devices, waker, g)?),
        Device::Server => unreachable!("checked graphs run only the world on the server"),
    }
}

fn run<S: FaceSource + 'static>((core, draw, source): (Core, Draw, S)) -> anyhow::Result<Box<dyn Renderer>> {
    crate::run((core, Faces { draw, source }))
}

/// What the core and the drawer need of a face source's options.
struct CoreOpts<'a> {
    shading: &'a str,
    draw: &'a str,
    bounces: u32,
    spp: u32,
    accumulate: bool,
    shadows: Shadows,
    max_steps: u32,
    pool_mb: u32,
    frames_in_flight: usize,
}

/// What a face source gives the core: its WGSL (after `frame.wgsl`), the storage groups of
/// its sites' walks and how the drawer draws its faces.
struct SourceSpec {
    wgsl: String,
    /// The walks of the GPU's rays: the indices of the discovery rays' kernel and the
    /// exposure storage in them (the GPU sampler's), and of the shadow and bounce rays'.
    walks: Walks,
    lead: Option<(usize, usize)>,
    sites: [usize; 2],
    /// The drawer's fragment inputs: interpolated (unit faces) or rebuilt from the instance
    /// (merged faces, `draw::faces_wgsl`).
    varyings: bool,
    /// Instances drawn as triangle strips (else lists: `cubes`).
    strip: bool,
    /// Meshes drawn with the faces (mesh mode): the mesh-shadow site's walk. Deferred drawers
    /// then compile a variant with the record's mesh capability for frames with meshes.
    meshes: Option<usize>,
    /// Frames draw meshes (the meshes stage is not `none`; live).
    draw_meshes: bool,
    /// GPU features the source needs.
    features: wgpu::Features,
    /// The source reads the depth buffer (hole detection: last frame's depth).
    depth_read: bool,
    /// The drawer's timed stages' nodes.
    nodes: DrawNodes,
}

/// The mesh capability's variant of a program.
struct Meshed {
    program: mr_gpu::program::Program,
    deps: Vec<String>,
}

/// The lane's drawing, every face source's: the face drawer and what it is compiled from,
/// group 0's layout, the depth buffer.
pub(crate) struct Draw {
    shading: String,
    /// The face drawer (`draw`), compiled against `program` (the walks' modules) with
    /// `deps` from `draw_src` (the frame, the face source, the shading and the face
    /// drawer's vertex stage) plus the draw component.
    drawer: FaceDrawer,
    draw: String,
    source: String,
    varyings: bool,
    strip: bool,
    draw_src: String,
    program: mr_gpu::program::Program,
    deps: Vec<String>,
    /// With meshes: the program with the mesh capability, and whether frames draw meshes
    /// (the meshes stage is live).
    meshed: Option<Meshed>,
    draw_meshes: bool,
    layout_draw: wgpu::BindGroupLayout,
    pl_draw: wgpu::PipelineLayout,
    pl_shade: wgpu::PipelineLayout,
    images: wgpu::BindGroupLayout,
    depth: wgpu::TextureView,
    depth_read: bool,
    /// The drawer's timed stages' nodes.
    draw_nodes: DrawNodes,
    depth_prepass: bool,
}

/// What a face drawer is compiled from.
struct DrawParts<'a> {
    gpu: &'a Gpu,
    program: &'a mr_gpu::program::Program,
    deps: &'a [String],
    meshed: Option<&'a Meshed>,
    strip: bool,
    draw_src: &'a str,
    shading: &'a str,
    pl_draw: &'a wgpu::PipelineLayout,
    pl_shade: &'a wgpu::PipelineLayout,
    images: &'a wgpu::BindGroupLayout,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
}

impl DrawParts<'_> {
    fn module(&self, label: &str, rest: &str) -> wgpu::ShaderModule {
        Self::compile(self.gpu, label, self.program, self.deps, &format!("{}\n{rest}", self.draw_src))
    }

    fn compile(gpu: &Gpu, label: &str, program: &mr_gpu::program::Program, deps: &[String], root: &str) -> wgpu::ShaderModule {
        mr_gpu::program::compile(&gpu.device, label, program, root, &deps.iter().map(String::as_str).collect::<Vec<_>>())
    }

    /// The face drawer for `draw` (the draw component's WGSL over `draw_src`; forward
    /// drawing shades in the fragment shader); deferred drawers of a source with meshes also
    /// the mesh capability's variant, for frames with meshes.
    fn drawer(&self, draw: &str) -> anyhow::Result<FaceDrawer> {
        let mode = DrawMode::parse(draw).ok_or_else(|| anyhow::anyhow!("draw must be raster, visbuffer or gbuffer, not `{draw}`"))?;
        if !mode.deferred()
            && let Some(why) = shade_state::fragment_blocker(self.shading, &self.gpu.device) {
            anyhow::bail!(why);
        }
        let vis = draw_wgsl(mode, self.shading);
        let module = self.module("face drawer", &vis);
        let meshed = self.meshed.filter(|_| mode.deferred()).map(|m| Self::compile(self.gpu, "face drawer with meshes", &m.program, &m.deps, &format!("{}\n{vis}", self.draw_src)));
        FaceDrawer::new(self.gpu, &DrawerSpec { module: &module, render_layout: self.pl_draw, shade_layout: self.pl_shade, strip: self.strip, mesh_module: meshed.as_ref(), format: self.format }, mode, self.images, self.width, self.height)
    }
}

/// The draw component's WGSL after `draw_src`: deferred drawers shade in compute, forward
/// drawing in the fragment shader.
fn draw_wgsl(mode: DrawMode, shading: &str) -> String {
    let vis = draw::gpu_wgsl(mode.name()).unwrap();
    if mode.deferred() { vis.to_string() } else { shade_state::fragment_wgsl(shading, vis) }
}

/// The walks' program for a source's sites (discovery rays, exposure, shadow, bounce) and the
/// root's dependencies; with meshes (the mesh-shadow site's walk) also the mesh capability's
/// variant, the triangles in the images group.
fn programs(walks: &Walks, lead: Option<(usize, usize)>, [shadow, bounce]: [usize; 2], meshes: Option<usize>) -> (mr_gpu::program::Program, Vec<String>, Option<Meshed>) {
    let (program, deps) = mr_components::shaders::sites_program(walks, lead, shadow, bounce);
    let meshed = meshes.map(|mesh_shadow| {
        let (mut program, mut deps) = (program.clone(), deps.clone());
        deps.insert(0, mr_components::shaders::meshes(&mut program, walks, shadow, mesh_shadow, 3, 2));
        Meshed { program, deps }
    });
    (program, deps, meshed)
}

/// The drawing lane's WGSL before the draw component: the frame, the face source, the
/// shading state and component, the face drawer's vertex stage.
fn draw_src(source: &str, shading: &str, varyings: bool) -> String {
    let shade = mr_components::shading::gpu_wgsl(shading).expect("validated");
    format!("{}\n{source}\n{}\n{shade}\n{}", include_str!("frame.wgsl"), ShadeState::wgsl(0, crate::STATE_BINDING), draw::faces_wgsl(varyings))
}

fn depth_view(gpu: &Gpu, w: u32, h: u32, read: bool) -> wgpu::TextureView {
    let read = if read { wgpu::TextureUsages::TEXTURE_BINDING } else { wgpu::TextureUsages::empty() };
    gpu.device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            // Hole detection reads last frame's depth.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | read,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

/// The core and the drawing for a face source's spec.
fn new(p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked, o: &CoreOpts, depth_prepass: bool, src: SourceSpec) -> anyhow::Result<(Core, Draw)> {
    let core = Core::new(
        p,
        devices,
        waker,
        g,
        CoreSpec {
            gpu: GpuNeeds { experimental: false, required_features: src.features, optional_features: wgpu::Features::TIMESTAMP_QUERY | shade_state::FRAGMENT_FEATURE },
            walks: &src.walks,
            accel: crate::accel::AccelSpec::of(g, &src.walks)?,
            stages: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::VERTEX_FRAGMENT,
            max_lod: 0,
            voxels: false,
            records: false,
            shading: o.shading,
            bounces: o.bounces,
            spp: o.spp,
            accumulate: o.accumulate,
            shadows: o.shadows,
            max_steps: o.max_steps,
            pool_mb: o.pool_mb,
            frames_in_flight: o.frames_in_flight,
        },
    )?;
    let (gpu, dev) = (&core.gpu, &core.gpu.device);
    let shading = o.shading.to_string();
    // The walks' modules: the roots cast through the discovery rays' kernel and read the
    // exposure storage; shading's rays through theirs.
    let (mut program, deps, mut meshed) = programs(&src.walks, src.lead, src.sites, src.meshes);
    // Ray queries at the sites: their bindings and trace in every root.
    core.trace_root(&mut program, "0u");
    if let Some(m) = &mut meshed {
        core.trace_root(&mut m.program, "0u");
    }
    let draw_src = draw_src(&src.wgsl, &shading, src.varyings);
    let buf = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    };
    // The core's group 0 (the camera, the shading state, the acceleration structures; the
    // output is the drawer's render attachment, not bound) and the face source's two buffers.
    let mut entries = core.layout_entries_with(false);
    entries.extend([buf(crate::LANE_BINDING), buf(crate::LANE_BINDING + 1)]);
    let layout_draw = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("face draw"), entries: &entries });
    let images = FaceDrawer::images_layout(dev, meshed.is_some(), core.output_format());
    let pl_draw = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("face draw"), bind_group_layouts: &core.storages.layouts(&[Some(&layout_draw), Some(core.world.layout())]), immediate_size: 0 });
    let pl_shade = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("face deferred shade"),
        bind_group_layouts: &core.storages.layouts(&[Some(&layout_draw), Some(core.world.layout()), None, Some(&images)]),
        immediate_size: 0,
    });
    let parts = DrawParts { gpu, program: &program, deps: &deps, meshed: meshed.as_ref().filter(|_| src.draw_meshes), strip: src.strip, draw_src: &draw_src, shading: &shading, pl_draw: &pl_draw, pl_shade: &pl_shade, images: &images, width: p.width, height: p.height, format: core.output_format() };
    let drawer = parts.drawer(o.draw)?;
    let draw = Draw {
        drawer,
        draw: o.draw.to_string(),
        depth: depth_view(gpu, p.width, p.height, src.depth_read),
        depth_read: src.depth_read,
        draw_nodes: src.nodes,
        depth_prepass,
        shading,
        source: src.wgsl,
        varyings: src.varyings,
        strip: src.strip,
        draw_src,
        program,
        deps,
        draw_meshes: src.draw_meshes && meshed.is_some(),
        meshed,
        layout_draw,
        pl_draw,
        pl_shade,
        images,
    };
    Ok((core, draw))
}

fn sky_rgba(core: &Core) -> [f32; 4] {
    let s = core.sky();
    [s[0] as f32 / 255.0, s[1] as f32 / 255.0, s[2] as f32 / 255.0, 1.0]
}

impl Draw {
    fn parts<'a>(&'a self, core: &'a Core) -> DrawParts<'a> {
        let meshed = self.meshed.as_ref().filter(|_| self.draw_meshes);
        DrawParts { gpu: &core.gpu, program: &self.program, deps: &self.deps, meshed, strip: self.strip, draw_src: &self.draw_src, shading: &self.shading, pl_draw: &self.pl_draw, pl_shade: &self.pl_shade, images: &self.images, width: core.width, height: core.height, format: core.output_format() }
    }

    /// A module over the walks: `draw_src` plus `rest` (the GPU sampler's own raster entry
    /// points).
    fn module(&self, core: &Core, label: &str, rest: &str) -> wgpu::ShaderModule {
        self.parts(core).module(label, rest)
    }

    fn drawer(&self, core: &Core, draw: &str) -> anyhow::Result<FaceDrawer> {
        self.parts(core).drawer(draw)
    }

    /// Recompiles the drawer for a new face source WGSL (`strip`: its primitive), shading
    /// (also the shading state's) and whether frames draw meshes; returns whether the shading
    /// state's bindings changed (the source's draw bind group is then stale).
    fn rebuild(&mut self, core: &mut Core, source: String, strip: bool, shading: &str, meshes: bool) -> anyhow::Result<bool> {
        let rebound = core.accum.set_shading(&core.gpu, shading);
        self.shading = shading.to_string();
        self.draw_src = draw_src(&source, shading, self.varyings);
        self.source = source;
        self.strip = strip;
        self.draw_meshes = meshes && self.meshed.is_some();
        self.drawer = self.drawer(core, &self.draw)?;
        Ok(rebound)
    }

    /// A draw bind group: the core's group 0 and the face source's two buffers.
    fn bind(&self, core: &Core, a: &wgpu::Buffer, b: &wgpu::Buffer) -> wgpu::BindGroup {
        let mut entries = core.bind_entries_with(None);
        entries.extend([wgpu::BindGroupEntry { binding: crate::LANE_BINDING, resource: a.as_entire_binding() }, wgpu::BindGroupEntry { binding: crate::LANE_BINDING + 1, resource: b.as_entire_binding() }]);
        core.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("face draw"), layout: &self.layout_draw, entries: &entries })
    }

    /// Writes the core's camera uniform for `c` (`view`: the face source's tint, the lane's
    /// first word; `anchor`: the CPU records' origin).
    fn write_frame(&self, core: &Core, c: &Camera, mode: RenderMode, view: u32, anchor: IVec3) {
        core.write_camera_at(c, mode, [view, 0], anchor);
    }

    /// Lit `stateful` shading drawn forward may update its state once per pixel only
    /// (`painter`: the sampler's drawing order, if it has that knob).
    fn check_state(&self, mode: RenderMode, painter: Option<bool>) -> anyhow::Result<()> {
        if !self.drawer.mode().deferred() && shade_state::stateful(&self.shading) && mode == RenderMode::Lit && (painter == Some(true) || !self.depth_prepass) {
            let order = if painter.is_some() { "order = depth and " } else { "" };
            anyhow::bail!("`{}` shading with draw = raster needs {order}depth_prepass in lit mode (exactly one fragment per pixel may update its state)", self.shading);
        }
        Ok(())
    }

    /// The drawing's knobs; `None` if `name` is not one.
    fn set_param(&mut self, core: &Core, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        Some(match name {
            "depth_prepass" => {
                self.depth_prepass = value.as_bool().unwrap_or(true);
                Ok(())
            }
            "draw" => {
                // Drawing only: the face set is unaffected.
                let draw = value.as_str().unwrap_or("raster");
                self.drawer(core, draw).map(|d| {
                    self.drawer = d;
                    self.draw = draw.into();
                })
            }
            _ => return None,
        })
    }

    fn resize(&mut self, core: &Core) {
        self.depth = depth_view(&core.gpu, core.width, core.height, self.depth_read);
        self.drawer.resize(&core.gpu, core.width, core.height);
    }

    fn memory_bytes(&self, core: &Core) -> u64 {
        self.drawer.memory_bytes() + core.width as u64 * core.height as u64 * 4
    }
}

/// A face source: what feeds the drawer's faces, on its device (meshing, or a sampler).
trait FaceSource {
    /// World changes before the GPU world applies them (the CPU world, event areas).
    fn updating(&mut self, updates: &[WorldUpdate]);
    /// Chunks the GPU world changed.
    fn updated(&mut self, core: &Core, changed: &[IVec3]);
    /// Meshes (mesh mode), for a source that draws them.
    fn update_meshes(&mut self, _update: MeshUpdate) {}
    fn render(&mut self, core: &mut Core, draw: &mut Draw, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()>;
    /// Its knobs; `None` if `name` is not one.
    fn set_param(&mut self, core: &mut Core, draw: &mut Draw, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>>;
    fn resize(&mut self, core: &Core, draw: &Draw);
    /// (CPU, GPU) bytes beyond the core's and the drawing's.
    fn memory(&self, core: &Core) -> (u64, u64);
}

/// The drawing and one face source (static dispatch).
struct Faces<S> {
    draw: Draw,
    source: S,
}

impl<S: FaceSource> Lane for Faces<S> {
    fn updating(&mut self, updates: &[WorldUpdate]) {
        self.source.updating(updates);
    }

    fn updated(&mut self, core: &Core, changed: &[IVec3]) -> anyhow::Result<()> {
        self.source.updated(core, changed);
        Ok(())
    }

    fn update_meshes(&mut self, update: MeshUpdate) {
        self.source.update_meshes(update);
    }

    fn render(&mut self, core: &mut Core, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.source.render(core, &mut self.draw, req, sink)
    }

    fn set_param(&mut self, core: &mut Core, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        self.source.set_param(core, &mut self.draw, name, value).or_else(|| self.draw.set_param(core, name, value))
    }

    fn resize(&mut self, core: &Core) {
        self.draw.resize(core);
        self.source.resize(core, &self.draw);
    }

    fn memory(&self, core: &Core) -> (u64, u64) {
        let (cpu, gpu) = self.source.memory(core);
        (cpu, gpu + self.draw.memory_bytes(core))
    }
}

#[cfg(test)]
mod tests {
    use mr_components::draw::drawer::DrawMode;
    use mr_components::{Slot, names_on};

    /// A face source's drawing lane (`draw_src`) with every drawer `backend` offers (the
    /// raster's fragment stage with the sampler's own entry points) composes and validates
    /// over `program`, and over the mesh capability's variant where deferred drawers have it.
    pub(crate) fn drawers(program: &mr_gpu::program::Program, deps: &[String], meshed: Option<&super::Meshed>, draw_src: &str, shading: &str, what: &str, backend: &str) {
        use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        let check = |program: &mr_gpu::program::Program, deps: &[String], root: &str, what: &str| {
            let deps: Vec<&str> = deps.iter().map(String::as_str).collect();
            let m = program.naga(root, &deps).unwrap_or_else(|e| panic!("{what}: {e}"));
            Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("{what}: {e:?}"));
        };
        for d in names_on(backend, Slot::Draw) {
            let mode = DrawMode::parse(d).unwrap();
            let root = format!("{draw_src}\n{}", super::draw_wgsl(mode, shading));
            check(program, deps, &root, &format!("{backend} drawer {what} {d}"));
            if let Some(m) = meshed.filter(|_| mode.deferred()) {
                check(&m.program, &m.deps, &root, &format!("{backend} drawer with meshes {what} {d}"));
            }
        }
    }

    /// The CPU sampler's drawing lane: the records' face source and every drawer, for every
    /// GPU storage and kernel of the shadow rays and every shading (no GPU needed).
    #[test]
    fn cpu_programs_compose() {
        use mr_components::graph::{Graph, Role};
        use mr_components::shaders::Walks;
        let backend = super::cpu::BACKEND;
        for st in names_on(backend, Slot::ShadowStorage).into_iter().filter(|s| *s != "same") {
            for shading in ["direct", "pathtrace"] {
                let c = mr_components::Composition { shadow_storage: st.into(), shading: shading.into(), ..mr_components::Composition::new(backend) };
                let ck = Graph::from_composition(&c).unwrap().check().unwrap();
                let mut walks = Walks::of(&ck);
                let groups = &[2, 4];
                let shadow = walks.site(&ck, ck.site(Role::Shading, "shadow").unwrap(), groups, false);
                let bounce = ck.site(Role::Shading, "bounce").map_or(shadow, |s| walks.site(&ck, s, groups, false));
                let (program, deps) = mr_components::shaders::sites_program(&walks, None, shadow, bounce);
                drawers(&program, &deps, None, &super::draw_src(include_str!("records.wgsl"), shading, true), shading, &format!("{st} {shading}"), backend);
            }
        }
    }
}
