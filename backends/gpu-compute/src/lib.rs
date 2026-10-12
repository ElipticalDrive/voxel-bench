//! GPU-compute back end: assembles a renderer from components (`mr-components`, IDEAS
//! §12): a **storage**, a **visibility** walk and a **shading** component, each a WGSL module
//! from `components/`, pasted into one compute shader before compilation, so the compiler
//! optimises across them and the split costs nothing at run time.
//!
//! Storages answer the interface in `mr_components::storage`; optional fast-path queries the
//! chosen storage lacks get generic fallbacks, so every storage works with every
//! visibility component. Shadow rays use the same composed walk. The storage's own data
//! (per-slot side data, or region blobs) is kept by `storage::gpu_data::StorageData`.
//! Meshes (mesh mode) are composited by `mr_gpu::meshes::MeshComposite`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use mr_components::storage::gpu_data::{self, StorageData};
use mr_components::shading::state::{self as shade_state, ShadeState};
use mr_components::{names_on, require, Slot, components, storage, visibility};
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::meshes::{MESH_COMPOSITE_WGSL, MeshComposite, Occlusion};
use mr_gpu::voxels::{Shadows, VoxelWorld, VoxelWorldConfig, shader_source, shadows_param};
use mr_gpu::{FrameRing, Gpu, GpuRequest, wgpu};
use serde::Deserialize;

pub fn entry() -> RendererEntry {
    RendererEntry { info, create: |p, w| Ok(Box::new(GpuCompose::new(p, w)?)) }
}


const BACKEND: &str = "gpu-compute";

fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu-compute".into(),
        description: "GPU-compute back end: renders a composition of components (storage, visibility, shading) fused into one compute shader".into(),
        tags: vec!["gpu".into(), "compute".into(), "raycast".into(), "modular".into(), "wgpu".into(), "meshes".into(), "lod".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        // LOD with pool storages and refloor (cells 2x coarser per level; was gpu-dda-lod).
        max_lod: 6,
        params: mr_components::lighting_knobs(BACKEND, params()),
    }
}


fn describe(slot: Slot) -> String {
    components(slot).filter(|c| c.backends.contains(&"gpu-compute")).map(|c| format!("{}: {}", c.name, c.description)).collect::<Vec<_>>().join(" ")
}

fn params() -> Vec<ParamSpec> {
    let mut p = vec![
        ParamSpec::choice("storage", &format!("Storage component. {}", describe(Slot::Storage)), &names_on(BACKEND, Slot::Storage), "bricks", false),
        ParamSpec::choice("visibility", &format!("Visibility component (any storage works with any; missing fast paths use generic fallbacks). {}", describe(Slot::Visibility)), &names_on(BACKEND, Slot::Visibility), "refloor", true),
        ParamSpec::choice("shading", &format!("Shading component. {}", describe(Slot::Shading)), &names_on(BACKEND, Slot::Shading), "direct", true),
        ParamSpec::choice("prepass", &format!("Prepass component (a stage before the walk). {}", describe(Slot::Prepass)), &names_on(BACKEND, Slot::Prepass), "none", true),
        ParamSpec::choice("meshes", &format!("Meshes component (mesh mode). {}", describe(Slot::Meshes)), &names_on(BACKEND, Slot::Meshes), "composite", true),
        ParamSpec::int("tile", "beam prepass: tile size in pixels (1 = no prepass)", 1, 64, 8, true).of("prepass", &["beam"]),
    ];
    p.extend(shade_state::params());
    p.push(ParamSpec::bool("exact", "incremental: recompute exit times from the integer cell after each move (no float drift) instead of accumulating them", true, true).of("visibility", &["incremental"]));
    for (k, size) in [(1, 2), (2, 4), (3, 8), (4, 16)] {
        p.push(ParamSpec::bool(&format!("skip{size}"), &format!("incremental: use {size}^3 cubes (level {k}) if the storage has them"), true, true).of("visibility", &["incremental"]));
    }
    p.push(ParamSpec::choice("dispatch", "How pixels are run: `compute` (one compute thread per pixel) or `fragment` (a fragment shader over a full-screen triangle; was gpu-frag-dda)", &["compute", "fragment"], "compute", true));
    p.push(ParamSpec::choice("view", "image: the rendered scene. steps: primary-ray loop iterations per pixel on a colour scale (log; white above view_max)", &["image", "steps"], "image", true));
    p.push(ParamSpec::int("view_max", "steps view: the top of the colour scale (iterations)", 1, 16384, 256, true));
    p.extend([
        shadows_param(),
        ParamSpec::int("max_steps", "Loop iteration limit per ray", 32, 16384, 2048, true),
        ParamSpec::int("pool_mb", "Voxel pool limit (MiB); for region storages (tree64) the limit of their own data", 64, 6144, 2048, false),
        ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
    ]);
    p
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    adapter: Option<String>,
    storage: String,
    /// `traversal` is the step-6′ name of this option.
    #[serde(alias = "traversal")]
    visibility: String,
    shading: String,
    prepass: String,
    meshes: String,
    tile: u32,
    bounces: u32,
    spp: u32,
    accumulate: bool,
    exact: bool,
    skip2: bool,
    skip4: bool,
    skip8: bool,
    skip16: bool,
    view: String,
    view_max: u32,
    dispatch: String,
    shadows: Shadows,
    pool_mb: u32,
    max_steps: u32,
    frames_in_flight: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            adapter: None,
            storage: "bricks".into(),
            visibility: "refloor".into(),
            shading: "direct".into(),
            prepass: "none".into(),
            meshes: "composite".into(),
            tile: 8,
            bounces: 1,
            spp: 1,
            accumulate: true,
            exact: true,
            skip2: true,
            skip4: true,
            skip8: true,
            skip16: true,
            view: "image".into(),
            view_max: 256,
            dispatch: "compute".into(),
            shadows: Shadows::All,
            pool_mb: 2048,
            max_steps: 2048,
            frames_in_flight: 3,
        }
    }
}

/// The compile-time part of a composition: everything baked into the shader.
#[derive(Clone, PartialEq, Debug)]
struct Composition {
    storage: String,
    visibility: String,
    shading: String,
    prepass: String,
    /// Prepass tile size in pixels (1 = the prepass is skipped).
    tile: u32,
    exact: bool,
    /// Levels allowed by the skip knobs (bit k = 2^k cubes).
    levels: u32,
    /// `view = "steps"`: write iteration counts instead of colour.
    steps_view: bool,
    /// LOD chunks are streamed and walked (fixed at creation).
    lod: bool,
    /// `dispatch = "fragment"`: pixels in a fragment shader over a full-screen triangle.
    fragment: bool,
}

impl Composition {
    fn storage(&self) -> storage::GpuStorage {
        storage::gpu(&self.storage).expect("validated")
    }

    /// The prepass in effect: a tile of 1 pixel turns it off.
    fn prepass(&self) -> &str {
        if self.tile <= 1 { "none" } else { &self.prepass }
    }

    fn source(&self) -> String {
        let st = self.storage();
        let vis = visibility::gpu_wgsl(&self.visibility).expect("validated");
        let shade = mr_components::shading::gpu_wgsl(&self.shading).expect("validated");
        let pre = mr_components::prepass::gpu_wgsl(self.prepass()).expect("validated");
        let own = format!(
            "// Composition: storage = {}, prepass = {}, visibility = {}, shading = {}\n\
             const PREPASS_TILE: u32 = {}u;\n\
             const LOD: bool = {};\n\
             const EXACT: bool = {};\n\
             const VIEW_STEPS: bool = {};\n\
             const LEVEL_MASK: u32 = (S_LEVELS & {:#x}u) | 1u | (1u << S_TOP);\n\
             {MESH_COMPOSITE_WGSL}\n{}\n{}\n{}\n{vis}\n{shade}\n{}\n{pre}",

            self.storage,
            self.prepass(),
            self.visibility,
            self.shading,
            self.tile.max(1),
            self.lod,
            self.exact,
            self.steps_view,
            self.levels,
            mr_core::legend::ramp_wgsl(),
            ShadeState::wgsl(0, 4),
            st.source(),
            include_str!("main.wgsl"),
        );
        shader_source(visibility::OCCLUDED_WGSL, &own)
    }
}


/// Why LOD cannot run with this composition, if it can't.
fn lod_blocker(c: &Composition) -> Option<String> {
    if !c.storage().has_lod() {
        Some(format!("storage `{}` holds level 0 only (LOD needs a pool storage: bricks, hier, prefix, distance, rows)", c.storage))
    } else if c.visibility != "refloor" {
        Some(format!("visibility `{}` has no LOD walk (refloor has)", c.visibility))
    } else if c.prepass() != "none" {
        Some(format!("prepass `{}` reads level 0 only", c.prepass()))
    } else {
        None
    }
}

fn parse_dispatch(s: &str) -> anyhow::Result<bool> {
    match s {
        "compute" => Ok(false),
        "fragment" => Ok(true),
        _ => anyhow::bail!("dispatch must be `compute` or `fragment`, got `{s}`"),
    }
}

fn parse_view(s: &str) -> anyhow::Result<bool> {
    match s {
        "image" => Ok(false),
        "steps" => Ok(true),
        _ => anyhow::bail!("view must be `image` or `steps`, got `{s}`"),
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct CameraUniform {
    forward: [f32; 3],
    tan_half_fov: f32,
    right: [f32; 3],
    aspect: f32,
    up: [f32; 3],
    mode: u32,
    origin: [f32; 3],
    sky: u32,
    width: u32,
    height: u32,
    view_max: u32,
    _pad: u32,
}

pub struct GpuCompose {
    gpu: Gpu,
    world: VoxelWorld,
    /// Mesh mode: meshes rasterised first, rays capped at their depth.
    meshes: MeshComposite,
    comp: Composition,
    /// Composition the pipeline was built from (rebuilt lazily when knobs change it).
    built: Composition,
    pipeline: Main,
    /// The prepass component's own pass, if it has one.
    prepass: Option<wgpu::ComputePipeline>,
    pipeline_layout: wgpu::PipelineLayout,
    layout0: wgpu::BindGroupLayout,
    layout2: wgpu::BindGroupLayout,
    camera_buf: wgpu::Buffer,
    output: wgpu::Texture,
    bind0: wgpu::BindGroup,
    /// Group 0 for `dispatch = "fragment"`: a 1x1 stand-in instead of the output texture.
    bind0_frag: wgpu::BindGroup,
    stand_in: wgpu::Texture,
    /// Prepass output: one f32 per tile (sized for 1-pixel tiles), and statistics.
    beam: Beam,
    /// Shading state (pathtrace's accumulation).
    accum: ShadeState,
    /// `view = steps`: the top of the colour scale.
    view_max: u32,
    /// The meshes stage draws meshes (`composite`), or not (`none`).
    draw_meshes: bool,
    /// The storage's own data (group 2).
    data: StorageData,
    ring: FrameRing,
    width: u32,
    height: u32,
    depth: usize,
    waker: Waker,
}

impl GpuCompose {
    pub fn new(p: InitParams, waker: Waker) -> anyhow::Result<Self> {
        let opts: Options = p.options()?;
        let comp = Composition {
            storage: require(BACKEND, Slot::Storage, &opts.storage)?,
            visibility: require(BACKEND, Slot::Visibility, &opts.visibility)?,
            shading: require(BACKEND, Slot::Shading, &opts.shading)?,
            prepass: require(BACKEND, Slot::Prepass, &opts.prepass)?,
            tile: opts.tile.clamp(1, 64),
            exact: opts.exact,
            steps_view: parse_view(&opts.view)?,
            levels: [opts.skip2, opts.skip4, opts.skip8, opts.skip16].iter().enumerate().fold(0, |m, (i, &on)| m | (on as u32) << (i + 1)),
            lod: false,
            fragment: parse_dispatch(&opts.dispatch)?,
        };
        let mut comp = comp;
        if p.max_lod > 0 {
            // LOD needs chunks at every level (pool storages), a walk that scales its cells
            // (refloor) and no prepass (the beam reads level 0 only).
            match lod_blocker(&comp) {
                None => comp.lod = true,
                Some(why) => tracing::warn!("gpu-compute: running without LOD (max_lod = {}): {why}", p.max_lod),
            }
        }
        let gpu = Gpu::new(GpuRequest {
            experimental: false,
            adapter: opts.adapter.as_deref(),
            required_features: wgpu::Features::empty(),
            optional_features: wgpu::Features::TIMESTAMP_QUERY,
            limits: &|a| a.clone(),
        })?;
        let world = VoxelWorld::new(
            &gpu,
            &p.blocks,
            p.environment,
            VoxelWorldConfig {
                max_lod: if comp.lod { p.max_lod.min(6) } else { 0 },
                // Region storages (tree64) hold their own data instead of the voxel pool.
                upload_voxels: matches!(comp.storage().kind, storage::Kind::Pool { .. }),
                pool_mb: opts.pool_mb,
                view_radius: p.view_radius_chunks,
                shadows: opts.shadows,
                max_steps: opts.max_steps,
                // Fragment too: the mesh raster pass shades with voxel shadow rays.
                visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
            },
        )?;
        let compute = wgpu::ShaderStages::COMPUTE;
        // Fragment too: `dispatch = "fragment"` runs the pixels in a fragment shader.
        let pixel = compute | wgpu::ShaderStages::FRAGMENT;
        // Fragment too: mesh shadows walk the composed storage.
        let layout2 = gpu_data::layout(&gpu, &comp.storage(), compute | wgpu::ShaderStages::FRAGMENT);
        let occ = visibility::occlusion_wgsl(&comp.storage(), comp.lod);
        let meshes = MeshComposite::new(&gpu, &world, p.width, p.height, Occlusion { wgsl: &occ, layout: &layout2 });
        let rw = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: pixel,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compose camera"),
            entries: &[
                rw(2),
                rw(3),
                ShadeState::layout_entries(4, pixel)[0],
                ShadeState::layout_entries(4, pixel)[1],
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: pixel,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: compute,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compose"),
            bind_group_layouts: &[Some(&layout0), Some(world.layout()), Some(&layout2), Some(meshes.layout())],
            immediate_size: 0,
        });
        let (pipeline, prepass) = Self::pipeline(&gpu, &pipeline_layout, &comp)?;
        let data = StorageData::new(&gpu, comp.storage(), gpu_data::Limits { mb: opts.pool_mb, view_radius: p.view_radius_chunks, wide_ids: p.blocks.blocks.len() > 256 });
        let camera_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let output = mr_gpu::output_texture(&gpu, p.width, p.height);
        let beam = Beam::new(&gpu, p.width, p.height);
        let accum = ShadeState::new(&gpu, p.width, p.height, &comp.shading, opts.bounces, opts.spp, opts.accumulate);
        let bind0 = Self::bind0(&gpu, &layout0, &camera_buf, &output, &beam, &accum);
        let stand_in = mr_gpu::output_texture(&gpu, 1, 1);
        let bind0_frag = Self::bind0(&gpu, &layout0, &camera_buf, &stand_in, &beam, &accum);
        let depth = opts.frames_in_flight.max(1);
        let ring = FrameRing::new(&gpu, p.width, p.height, depth, waker.clone());
        tracing::info!("gpu-compute: storage {}, visibility {}, shading {}", comp.storage, comp.visibility, comp.shading);
        Ok(Self {
            gpu,
            world,
            meshes,
            built: comp.clone(),
            comp,
            pipeline,
            prepass,
            pipeline_layout,
            layout0,
            layout2,
            camera_buf,
            output,
            bind0,
            bind0_frag,
            stand_in,
            beam,
            accum,
            view_max: opts.view_max.clamp(1, 16384),
            draw_meshes: require(BACKEND, Slot::Meshes, &opts.meshes)? != "none",
            data,
            ring,
            width: p.width,
            height: p.height,
            depth,
            waker,
        })
    }

    /// The main pipeline and, if the prepass has one, its pass.
    fn pipeline(gpu: &Gpu, layout: &wgpu::PipelineLayout, comp: &Composition) -> anyhow::Result<(Main, Option<wgpu::ComputePipeline>)> {
        let source = comp.source();
        let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("compose"), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let entry = |name| {
            gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(name),
                layout: Some(layout),
                module: &module,
                entry_point: Some(name),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let pipeline = if comp.fragment {
            let targets = [Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba8Unorm, blend: None, write_mask: wgpu::ColorWrites::ALL })];
            Main::Fragment(gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("compose fragment"),
                layout: Some(layout),
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_full"), compilation_options: Default::default(), buffers: &[] },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs_full"), compilation_options: Default::default(), targets: &targets }),
                multiview_mask: None,
                cache: None,
            }))
        } else {
            Main::Compute(entry("main"))
        };
        let prepass = mr_components::prepass::has_pass(comp.prepass()).then(|| entry("prepass"));
        if let Some(e) = pollster::block_on(scope.pop()) {
            anyhow::bail!("compiling {comp:?}: {e}");
        }
        Ok((pipeline, prepass))
    }

    fn rebind(&mut self) {
        self.bind0 = Self::bind0(&self.gpu, &self.layout0, &self.camera_buf, &self.output, &self.beam, &self.accum);
        self.bind0_frag = Self::bind0(&self.gpu, &self.layout0, &self.camera_buf, &self.stand_in, &self.beam, &self.accum);
    }

    fn bind0(gpu: &Gpu, layout: &wgpu::BindGroupLayout, cam: &wgpu::Buffer, out: &wgpu::Texture, beam: &Beam, accum: &ShadeState) -> wgpu::BindGroup {
        let view = out.create_view(&Default::default());
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("compose camera"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: cam.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: beam.tiles.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: beam.stats.as_entire_binding() },
                accum.bind_entries(4)[0].clone(),
                accum.bind_entries(4)[1].clone(),
            ],
        })
    }
}

impl Renderer for GpuCompose {
    fn update_meshes(&mut self, update: MeshUpdate) -> anyhow::Result<()> {
        self.meshes.meshes.apply(update);
        Ok(())
    }

    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        if !updates.is_empty() {
            self.accum.invalidate();
        }
        let changed = self.world.apply(&self.gpu, updates);
        self.data.update(&self.gpu, &self.world, &changed)
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        if self.comp != self.built {
            (self.pipeline, self.prepass) = Self::pipeline(&self.gpu, &self.pipeline_layout, &self.comp)?;
            if self.accum.set_shading(&self.gpu, &self.comp.shading) {
                self.rebind();
            }
            self.built = self.comp.clone();
        }
        let ticket = self.ring.begin(&self.gpu, sink)?;
        let start = Instant::now();
        let c = &req.camera;
        let (_, frac) = c.split_position();
        self.accum.begin(&self.gpu, c, req.mode);
        let cam = CameraUniform {
            forward: c.forward().to_array(),
            tan_half_fov: (c.fov_y * 0.5).tan(),
            right: c.right().to_array(),
            aspect: self.width as f32 / self.height as f32,
            up: c.up().to_array(),
            mode: (req.mode == RenderMode::Lit) as u32,
            origin: frac.to_array(),
            sky: u32::from_le_bytes(encode_srgb(self.world.env.sky)),
            width: self.width,
            height: self.height,
            view_max: self.view_max,
            _pad: 0,
        };
        self.gpu.queue.write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&cam));
        let world_bg = self.world.prepare(&self.gpu, c).clone();
        let bind2 = self.data.prepare(&self.gpu, &self.world, &self.layout2).clone();
        self.meshes.meshes.set_occlusion_bind_group(&bind2);
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        // The first pass carries the frame's begin timestamp: meshes, else the prepass.
        let want_meshes = req.meshes && self.draw_meshes;
        let meshes_on = self.meshes.active(want_meshes);
        let ts = if meshes_on { self.ring.render_timestamps(ticket, true, false) } else { None };
        self.meshes.encode(&self.gpu, &mut enc, c, self.width as f32 / self.height as f32, req.mode, want_meshes, &world_bg, ts);
        let copy_stats = self.beam.begin(&mut enc, self.prepass.is_some());
        if let Some(pre) = &self.prepass {
            let tile = self.comp.tile.max(1);
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("prepass"),
                timestamp_writes: if meshes_on { None } else { self.ring.compute_timestamps(ticket, true, false) },
            });
            pass.set_pipeline(pre);
            pass.set_bind_group(0, &self.bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.set_bind_group(2, &bind2, &[]);
            pass.set_bind_group(3, self.meshes.bind_group(), &[]);
            pass.dispatch_workgroups(self.width.div_ceil(tile).div_ceil(8), self.height.div_ceil(tile).div_ceil(8), 1);
        }
        let first = !meshes_on && self.prepass.is_none();
        match &self.pipeline {
            Main::Compute(pipeline) => {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("compose"),
                    timestamp_writes: self.ring.compute_timestamps(ticket, first, true),
                });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &self.bind0, &[]);
                pass.set_bind_group(1, &world_bg, &[]);
                pass.set_bind_group(2, &bind2, &[]);
                pass.set_bind_group(3, self.meshes.bind_group(), &[]);
                pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
            }
            Main::Fragment(pipeline) => {
                let view = self.output.create_view(&Default::default());
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("compose fragment"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: self.ring.render_timestamps(ticket, first, true),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(pipeline);
                // The output is the colour attachment here, so group 0 binds a stand-in
                // for the storage-texture output.
                pass.set_bind_group(0, &self.bind0_frag, &[]);
                pass.set_bind_group(1, &world_bg, &[]);
                pass.set_bind_group(2, &bind2, &[]);
                pass.set_bind_group(3, self.meshes.bind_group(), &[]);
                pass.draw(0..3, 0..1);
            }
        }
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            counters: [("dropped_chunks".to_string(), self.world.dropped_chunks() as f64)]
                .into_iter()
                .chain(self.data.counters())
                .chain(self.prepass.is_some().then(|| ("avg_skip_blocks".to_string(), self.beam.avg_skip)))
                .chain(self.accum.counter(req.mode))
                .collect(),
            legend: self.built.steps_view.then(|| Legend::ramp("loop iterations (primary ray)", self.view_max as f64, true)),
        };
        if copy_stats {
            self.beam.copy(&mut enc);
        }
        self.ring.submit(&self.gpu, enc, ticket, &self.output, req.frame_id, req.readback, stats);
        if copy_stats {
            self.beam.read();
        }
        self.ring.poll(&self.gpu, sink, false)
    }

    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        match name {
            "shadows" => self.world.cfg.shadows.set(value)?,
            "max_steps" => self.world.cfg.max_steps = value.as_i64().unwrap_or(2048) as u32,
            "visibility" | "traversal" | "prepass" | "tile" => {
                let mut c = self.comp.clone();
                match name {
                    "prepass" => c.prepass = require(BACKEND, Slot::Prepass, value.as_str().unwrap_or(""))?,
                    "tile" => c.tile = value.as_i64().unwrap_or(8).clamp(1, 64) as u32,
                    _ => c.visibility = require(BACKEND, Slot::Visibility, value.as_str().unwrap_or(""))?,
                }
                if let (true, Some(why)) = (c.lod, lod_blocker(&c)) {
                    anyhow::bail!("this run streams LOD chunks: {why}");
                }
                self.comp = c;
            }
            "shading" => self.comp.shading = require(BACKEND, Slot::Shading, value.as_str().unwrap_or(""))?,
            "meshes" => self.draw_meshes = require(BACKEND, Slot::Meshes, value.as_str().unwrap_or(""))? != "none",
            "view_max" => self.view_max = value.as_i64().unwrap_or(256).clamp(1, 16384) as u32,
            "view" => self.comp.steps_view = parse_view(value.as_str().unwrap_or(""))?,
            "dispatch" => self.comp.fragment = parse_dispatch(value.as_str().unwrap_or(""))?,
            "exact" => self.comp.exact = value.as_bool().unwrap_or(true),
            "skip2" | "skip4" | "skip8" | "skip16" => {
                let bit = 1 << name[4..].parse::<u32>()?.trailing_zeros();
                if value.as_bool().unwrap_or(true) {
                    self.comp.levels |= bit;
                } else {
                    self.comp.levels &= !bit;
                }
            }
            _ if self.accum.set_param(name, value).is_some() => {}
            _ => anyhow::bail!("`{name}` cannot change while running"),
        }
        Ok(())
    }

    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, true)
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.width = width;
        self.height = height;
        self.output = mr_gpu::output_texture(&self.gpu, width, height);
        self.meshes.resize(&self.gpu, width, height);
        self.beam = Beam::new(&self.gpu, width, height);
        self.accum.resize(&self.gpu, width, height);
        self.rebind();
        self.ring = FrameRing::new(&self.gpu, width, height, self.depth, self.waker.clone());
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        MemoryReport { cpu_bytes: 0, gpu_bytes: self.world.memory_bytes() + self.data.memory_bytes() + self.accum.memory_bytes() + self.beam.tiles.size() + self.width as u64 * self.height as u64 * 4 }
    }
}

/// The prepass's buffers: one f32 start distance per tile and the statistics (sum of
/// skipped blocks, tiles), read back without blocking, a few frames late.
struct Beam {
    tiles: wgpu::Buffer,
    stats: wgpu::Buffer,
    read: wgpu::Buffer,
    /// 0 idle, 1 mapping, 2 mapped.
    state: Arc<AtomicU32>,
    avg_skip: f64,
}

impl Beam {
    fn new(gpu: &Gpu, width: u32, height: u32) -> Self {
        let buf = |label, size: u64, usage| gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false });
        Self {
            tiles: buf("beam tiles", (width as u64 * height as u64 * 4).max(16), wgpu::BufferUsages::STORAGE),
            stats: buf("beam stats", 16, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST),
            read: buf("beam stats readback", 16, wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST),
            state: Arc::new(AtomicU32::new(0)),
            avg_skip: 0.0,
        }
    }

    /// Takes in a finished readback and clears the sums for this frame. Returns whether
    /// this frame's sums should be copied out.
    fn begin(&mut self, enc: &mut wgpu::CommandEncoder, active: bool) -> bool {
        if self.state.load(Ordering::Acquire) == 2 {
            if let Ok(data) = self.read.slice(..).get_mapped_range() {
                let v: &[u32] = bytemuck::cast_slice(&data[..8]);
                if v[1] > 0 {
                    self.avg_skip = v[0] as f64 / v[1] as f64;
                }
            }
            self.read.unmap();
            self.state.store(0, Ordering::Release);
        }
        if active {
            enc.clear_buffer(&self.stats, 0, None);
        }
        active && self.state.load(Ordering::Acquire) == 0
    }

    fn copy(&self, enc: &mut wgpu::CommandEncoder) {
        enc.copy_buffer_to_buffer(&self.stats, 0, &self.read, 0, 16);
    }

    fn read(&self) {
        self.state.store(1, Ordering::Release);
        let state = self.state.clone();
        self.read.slice(..).map_async(wgpu::MapMode::Read, move |r| state.store(if r.is_ok() { 2 } else { 0 }, Ordering::Release));
    }
}

/// The per-pixel pipeline: compute, or a fragment shader over a full-screen triangle.
enum Main {
    Compute(wgpu::ComputePipeline),
    Fragment(wgpu::RenderPipeline),
}
