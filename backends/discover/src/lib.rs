//! The discovery executor (PLAN step 13c): self-directed visibility (the user's idea: cast
//! as few rays as possible to discover every visible block face, keep the face set across
//! frames, rasterise only it), one executor for both of its configurations:
//! - `gpu-discover`: the sampler (`visibility` quadtree) on the GPU, with hole detection
//!   (`holes = coverage`) and the face set in a GPU hash set ([`gpu`]);
//! - `cpu-discover`: the sampler (quadtree or cluster) on the CPU over a CPU storage, its
//!   face records uploaded to the GPU, a frame ahead when the drawer's faces arrive `prev`
//!   ([`cpu`]).
//!
//! Which sampler runs is the visibility node's device in the graph; the upload and its
//! pipelining are the schedule's transfer of the faces from the CPU to the GPU (its timing).
//! The rest is shared ([`Core`]): the GPU device, the voxel world, the storages the GPU's
//! rays walk, the shading state, the face drawer (`draw`: raster, visbuffer or gbuffer)
//! with the sampler's face source, the frame uniform, the output and the frame ring.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::IVec3;
use mr_components::draw::{self, drawer::{DrawMode, DrawNodes, DrawerSpec, FaceDrawer}};
use mr_components::graph::{self, Checked, Device, Role};
use mr_components::shaders::Walks;
use mr_components::shading::state::{self as shade_state, ShadeState};
use mr_components::storage::gpu_data::{self, StorageSet};
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::voxels::{Shadows, VoxelWorld, VoxelWorldConfig};
use mr_gpu::{FrameRing, Gpu, GpuNeeds, wgpu};

mod cpu;
mod gpu;

pub fn gpu_entry() -> RendererEntry {
    RendererEntry { info: gpu::info, create: |p, d, w| Ok(Box::new(Discover::new(gpu::BACKEND, p, d, w)?)) }
}

pub fn cpu_entry() -> RendererEntry {
    RendererEntry { info: cpu::info, create: |p, d, w| Ok(Box::new(Discover::new(cpu::BACKEND, p, d, w)?)) }
}

/// The frame uniform of the face drawer (`frame.wgsl`).
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct FrameUniform {
    view_proj: [[f32; 4]; 4],
    cam_frac: [f32; 3],
    mode: u32,
    forward: [f32; 3],
    tan_half_fov: f32,
    right: [f32; 3],
    aspect: f32,
    up: [f32; 3],
    view: u32,
    size: [f32; 2],
    _p1: [f32; 2],
    anchor: [i32; 4],
    sky: [f32; 4],
}

/// What the core needs of a sampler's options.
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

/// A sampler's face source: its WGSL (after `frame.wgsl`) and the storage groups of its
/// sites' walks.
struct Source {
    wgsl: &'static str,
    /// The walks of the GPU's rays, and the indices of (discovery rays, exposure, shadow,
    /// bounce) in them (the CPU sampler's GPU walks are its shadow rays' and bounces').
    walks: Walks,
    sites: [usize; 4],
}

/// Everything but the sampler: the GPU drawing lane of both configurations.
pub(crate) struct Core {
    gpu: Arc<Gpu>,
    world: VoxelWorld,
    /// The storage instances the GPU's rays walk (PLAN step 9c).
    storages: StorageSet,
    /// Shading state (pathtrace's accumulation).
    accum: ShadeState,
    shading: String,
    /// The face drawer (`draw`), compiled against `program` (the walks' modules) with
    /// `deps` from `draw_src` (the frame, the face source, the shading and the face
    /// drawer's vertex stage) plus the draw component.
    drawer: FaceDrawer,
    draw: String,
    draw_src: String,
    program: mr_gpu::program::Program,
    deps: Vec<String>,
    layout_draw: wgpu::BindGroupLayout,
    pl_draw: wgpu::PipelineLayout,
    pl_shade: wgpu::PipelineLayout,
    images: wgpu::BindGroupLayout,
    frame_buf: wgpu::Buffer,
    output: wgpu::Texture,
    depth: wgpu::TextureView,
    ring: FrameRing,
    frames_in_flight: usize,
    /// The drawer's timed stages' nodes (PLAN step 11).
    draw_nodes: DrawNodes,
    depth_prepass: bool,
    width: u32,
    height: u32,
    waker: Waker,
}

/// What a face drawer is compiled from.
struct DrawParts<'a> {
    gpu: &'a Gpu,
    program: &'a mr_gpu::program::Program,
    deps: &'a [String],
    draw_src: &'a str,
    shading: &'a str,
    pl_draw: &'a wgpu::PipelineLayout,
    pl_shade: &'a wgpu::PipelineLayout,
    images: &'a wgpu::BindGroupLayout,
    width: u32,
    height: u32,
}

impl DrawParts<'_> {
    fn module(&self, label: &str, rest: &str) -> wgpu::ShaderModule {
        mr_gpu::program::compile(&self.gpu.device, label, self.program, &format!("{}\n{rest}", self.draw_src), &self.deps.iter().map(String::as_str).collect::<Vec<_>>())
    }

    /// The face drawer for `draw` (the draw component's WGSL over `draw_src`; forward
    /// drawing shades in the fragment shader).
    fn drawer(&self, draw: &str) -> anyhow::Result<FaceDrawer> {
        let mode = DrawMode::parse(draw).ok_or_else(|| anyhow::anyhow!("draw must be raster, visbuffer or gbuffer, not `{draw}`"))?;
        let vis = draw::gpu_wgsl(mode.name()).unwrap();
        let vis = if mode.deferred() {
            vis.to_string()
        } else {
            if let Some(why) = shade_state::fragment_blocker(self.shading, &self.gpu.device) {
                anyhow::bail!(why);
            }
            shade_state::fragment_wgsl(self.shading, vis)
        };
        let module = self.module("discover drawer", &vis);
        FaceDrawer::new(self.gpu, &DrawerSpec { module: &module, render_layout: self.pl_draw, shade_layout: self.pl_shade, strip: true, mesh_module: None }, mode, self.images, self.width, self.height)
    }
}

/// The drawing lane's WGSL before the draw component: the frame, the sampler's face source,
/// the shading state and component, the face drawer's vertex stage.
fn draw_src(source: &str, shading: &str) -> String {
    let shade = mr_components::shading::gpu_wgsl(shading).expect("validated");
    format!("{}\n{source}\n{}\n{shade}\n{}", include_str!("frame.wgsl"), ShadeState::wgsl(0, 3), draw::faces_wgsl(true))
}

fn depth_view(gpu: &Gpu, w: u32, h: u32) -> wgpu::TextureView {
    gpu.device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            // Hole detection reads last frame's depth.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

impl Core {
    fn new(p: &InitParams, devices: &Devices, waker: Waker, g: &Checked, o: &CoreOpts, depth_prepass: bool, src: Source) -> anyhow::Result<Self> {
        // The host hands the device in (GPU slot 0); the executor only states its needs.
        let gpu = mr_gpu::gpu(devices, 0, &GpuNeeds { experimental: false, required_features: wgpu::Features::empty(), optional_features: wgpu::Features::TIMESTAMP_QUERY | shade_state::FRAGMENT_FEATURE })?;
        let walks = src.walks;
        let world = VoxelWorld::new(
            &gpu,
            &p.blocks,
            p.environment,
            VoxelWorldConfig {
                max_lod: 0,
                // Region storages (tree64, ...) hold their own data instead of the voxel pool.
                upload_voxels: StorageSet::needs_pool(&walks),
                pool_mb: o.pool_mb,
                view_radius: p.view_radius_chunks,
                shadows: o.shadows,
                max_steps: o.max_steps,
                visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::VERTEX_FRAGMENT,
            },
        )?;
        let dev = &gpu.device;
        let storages = StorageSet::new(&gpu, &walks, wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::VERTEX_FRAGMENT, gpu_data::Limits { mb: o.pool_mb, view_radius: p.view_radius_chunks, wide_ids: p.blocks.blocks.len() > 256 });
        if storages.groups_end() > dev.limits().max_bind_groups {
            anyhow::bail!("{} storages need {} bind groups; this device has {}", walks.storages.len(), storages.groups_end(), dev.limits().max_bind_groups);
        }
        let shading = o.shading.to_string();
        let accum = ShadeState::new(&gpu, p.width, p.height, &shading, o.bounces, o.spp, o.accumulate);
        // The walks' modules: the roots cast through the discovery rays' kernel and read the
        // exposure storage; shading's rays through theirs.
        let [rays, exposure, shadow, bounce] = src.sites;
        let (program, deps) = mr_components::shaders::sites_program(&walks, rays, exposure, shadow, bounce);
        let draw_src = draw_src(src.wgsl, &shading);
        let buf = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let ro = wgpu::BufferBindingType::Storage { read_only: true };
        let [a, b] = ShadeState::layout_entries(3, wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE);
        // The frame, the face source's two buffers, the shading state.
        let layout_draw = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("discover draw"), entries: &[buf(0, wgpu::BufferBindingType::Uniform), buf(1, ro), buf(2, ro), a, b] });
        let images = FaceDrawer::images_layout(dev, false);
        let pl_draw = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("discover draw"), bind_group_layouts: &storages.layouts(&[Some(&layout_draw), Some(world.layout())]), immediate_size: 0 });
        let pl_shade = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("discover deferred shade"),
            bind_group_layouts: &storages.layouts(&[Some(&layout_draw), Some(world.layout()), None, Some(&images)]),
            immediate_size: 0,
        });
        let frame_buf = dev.create_buffer(&wgpu::BufferDescriptor { label: Some("frame"), size: size_of::<FrameUniform>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let fif = o.frames_in_flight.max(1);
        let parts = DrawParts { gpu: &gpu, program: &program, deps: &deps, draw_src: &draw_src, shading: &shading, pl_draw: &pl_draw, pl_shade: &pl_shade, images: &images, width: p.width, height: p.height };
        let drawer = parts.drawer(o.draw)?;
        Ok(Self {
            drawer,
            draw: o.draw.to_string(),
            output: mr_gpu::output_texture(&gpu, p.width, p.height),
            depth: depth_view(&gpu, p.width, p.height),
            ring: FrameRing::new(&gpu, p.width, p.height, fif, waker.clone()).with_stages(p.stage_times),
            frames_in_flight: fif,
            draw_nodes: DrawNodes { draw: g.ids(&[Role::Draw]), shade: g.ids(&[Role::Shading]), extra: Vec::new() },
            depth_prepass,
            gpu,
            world,
            storages,
            accum,
            shading,
            draw_src,
            program,
            deps,
            layout_draw,
            pl_draw,
            pl_shade,
            images,
            frame_buf,
            width: p.width,
            height: p.height,
            waker,
        })
    }

    fn parts(&self) -> DrawParts<'_> {
        DrawParts { gpu: &self.gpu, program: &self.program, deps: &self.deps, draw_src: &self.draw_src, shading: &self.shading, pl_draw: &self.pl_draw, pl_shade: &self.pl_shade, images: &self.images, width: self.width, height: self.height }
    }

    /// A module over the walks: `draw_src` plus `rest` (the sampler's own raster entry
    /// points).
    fn draw_module(&self, label: &str, rest: &str) -> wgpu::ShaderModule {
        self.parts().module(label, rest)
    }

    fn drawer(&self, draw: &str) -> anyhow::Result<FaceDrawer> {
        self.parts().drawer(draw)
    }

    /// A draw bind group: the frame, the face source's two buffers, the shading state.
    fn draw_bind(&self, a: &wgpu::Buffer, b: &wgpu::Buffer) -> wgpu::BindGroup {
        let [s0, s1] = self.accum.bind_entries(3);
        self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("discover draw"),
            layout: &self.layout_draw,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.frame_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: a.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: b.as_entire_binding() },
                s0,
                s1,
            ],
        })
    }

    fn sky_rgba(&self) -> [f32; 4] {
        let s = encode_srgb(self.world.env.sky);
        [s[0] as f32 / 255.0, s[1] as f32 / 255.0, s[2] as f32 / 255.0, 1.0]
    }

    /// Writes the frame uniform for `c` (`view`: the face source's tint; `anchor`: the CPU
    /// records' origin).
    fn write_frame(&self, c: &Camera, mode: RenderMode, view: u32, anchor: IVec3) {
        let (w, h) = (self.width as f32, self.height as f32);
        let fu = FrameUniform {
            view_proj: (c.projection(w / h) * c.view_rotation()).to_cols_array_2d(),
            cam_frac: c.split_position().1.to_array(),
            mode: (mode == RenderMode::Lit) as u32,
            forward: c.forward().to_array(),
            tan_half_fov: (c.fov_y * 0.5).tan(),
            right: c.right().to_array(),
            aspect: w / h,
            up: c.up().to_array(),
            view,
            size: [w, h],
            _p1: [0.0; 2],
            anchor: [anchor.x, anchor.y, anchor.z, 0],
            sky: self.sky_rgba(),
        };
        self.gpu.queue.write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&fu));
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

    /// The knobs of the drawing lane; `None` if `name` is not one.
    fn set_param(&mut self, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        Some(match name {
            "shadows" => self.world.cfg.shadows.set(value).map(|_| ()),
            "max_steps" => {
                self.world.cfg.max_steps = value.as_i64().unwrap_or(2048) as u32;
                Ok(())
            }
            "depth_prepass" => {
                self.depth_prepass = value.as_bool().unwrap_or(true);
                Ok(())
            }
            "draw" => {
                // Drawing only: the face set is unaffected.
                let draw = value.as_str().unwrap_or("raster");
                self.drawer(draw).map(|d| {
                    self.drawer = d;
                    self.draw = draw.into();
                })
            }
            _ => return self.accum.set_param(name, value).map(|_| Ok(())),
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.output = mr_gpu::output_texture(&self.gpu, width, height);
        self.depth = depth_view(&self.gpu, width, height);
        self.drawer.resize(&self.gpu, width, height);
        self.accum.resize(&self.gpu, width, height);
        self.ring = FrameRing::new(&self.gpu, width, height, self.frames_in_flight, self.waker.clone()).with_stages(self.ring.stages());
    }

    fn memory_bytes(&self) -> u64 {
        self.world.memory_bytes() + self.storages.memory_bytes() + self.accum.memory_bytes() + self.drawer.memory_bytes() + self.width as u64 * self.height as u64 * 8
    }
}

/// A sampler: the visibility node's implementation on its device.
trait Sampler {
    /// World changes before the GPU world applies them (the CPU world, event areas).
    fn updating(&mut self, updates: &[WorldUpdate]);
    /// Chunks the GPU world changed.
    fn updated(&mut self, core: &Core, changed: &[IVec3]);
    fn render(&mut self, core: &mut Core, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()>;
    /// Its knobs; `None` if `name` is not one.
    fn set_param(&mut self, core: &mut Core, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>>;
    fn resize(&mut self, core: &Core);
    /// (CPU, GPU) bytes beyond the core's.
    fn memory(&self, core: &Core) -> (u64, u64);
}

pub struct Discover {
    core: Core,
    sampler: Box<dyn Sampler>,
}

impl Discover {
    fn new(backend: &str, p: InitParams, devices: &Devices, waker: Waker) -> anyhow::Result<Self> {
        let g = graph::of_init(backend, &p)?;
        let vis = g.find(Role::Visibility).ok_or_else(|| anyhow::anyhow!("{backend}: no visibility node"))?;
        let (core, sampler): (Core, Box<dyn Sampler>) = match g.devices[vis] {
            Device::Cpu => {
                let (core, s) = cpu::CpuSampler::new(&p, devices, waker, &g)?;
                (core, Box::new(s))
            }
            Device::Gpu(_) => {
                let (core, s) = gpu::GpuSampler::new(&p, devices, waker, &g)?;
                (core, Box::new(s))
            }
        };
        Ok(Self { core, sampler })
    }
}

impl Renderer for Discover {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        if !updates.is_empty() {
            self.core.accum.invalidate();
        }
        self.sampler.updating(&updates);
        let c = &mut self.core;
        let changed = c.world.apply(&c.gpu, updates);
        c.storages.update(&c.gpu, &c.world, &changed)?;
        self.sampler.updated(&self.core, &changed);
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.sampler.render(&mut self.core, req, sink)
    }

    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.core.ring.poll(&self.core.gpu, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        if let Some(r) = self.sampler.set_param(&mut self.core, name, value) {
            return r;
        }
        self.core.set_param(name, value).unwrap_or_else(|| anyhow::bail!("`{name}` cannot change while running"))
    }

    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.core.ring.poll(&self.core.gpu, sink, true)
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.core.resize(width, height);
        self.sampler.resize(&self.core);
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        let (cpu, gpu) = self.sampler.memory(&self.core);
        MemoryReport { cpu_bytes: cpu, gpu_bytes: self.core.memory_bytes() + gpu }
    }
}

#[cfg(test)]
mod tests {
    use mr_components::draw::{self, drawer::DrawMode};
    use mr_components::shading::state as shade_state;
    use mr_components::{Slot, names_on};

    /// `backend`'s face source with every drawer it offers (the raster's fragment stage with
    /// the sampler's own entry points) composes and validates over `program`.
    pub(crate) fn drawers(program: &mr_gpu::program::Program, deps: &[String], shading: &str, what: &str, backend: &str) {
        let draw_src = crate::draw_src(if backend == crate::cpu::BACKEND { include_str!("records.wgsl") } else { include_str!("set_faces.wgsl") }, shading);
        use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        let deps: Vec<&str> = deps.iter().map(String::as_str).collect();
        let check = |root: &str, what: &str| {
            let m = program.naga(root, &deps).unwrap_or_else(|e| panic!("{what}: {e}"));
            Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("{what}: {e:?}"));
        };
        for d in names_on(backend, Slot::Draw) {
            let mode = DrawMode::parse(d).unwrap();
            let vis = draw::gpu_wgsl(mode.name()).unwrap();
            let vis = if mode.deferred() { vis.to_string() } else { shade_state::fragment_wgsl(shading, vis) };
            check(&format!("{draw_src}\n{vis}"), &format!("{backend} drawer {what} {d}"));
        }
    }

    /// The CPU sampler's drawing lane: the records' face source and every drawer, for every
    /// GPU storage and kernel of the shadow rays and every shading (no GPU needed).
    #[test]
    fn cpu_programs_compose() {
        use mr_components::graph::{Graph, Role};
        use mr_components::shaders::Walks;
        let backend = crate::cpu::BACKEND;
        for st in names_on(backend, Slot::ShadowStorage).into_iter().filter(|s| *s != "same") {
            for shading in ["direct", "pathtrace"] {
                let c = mr_components::Composition { shadow_storage: st.into(), shading: shading.into(), ..mr_components::Composition::new(backend) };
                let ck = Graph::from_composition(&c).unwrap().check().unwrap();
                let mut walks = Walks::default();
                let groups = &[2, 4];
                let shadow = walks.site(&ck, ck.site(Role::Shading, "shadow").unwrap(), groups, false);
                let bounce = ck.site(Role::Shading, "bounce").map_or(shadow, |s| walks.site(&ck, s, groups, false));
                let (program, deps) = mr_components::shaders::sites_program(&walks, shadow, shadow, shadow, bounce);
                drawers(&program, &deps, shading, &format!("{st} {shading}"), backend);
            }
        }
    }
}
