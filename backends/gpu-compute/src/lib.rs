//! GPU-compute back end: assembles a renderer from components (`mr-components`, IDEAS
//! §12): a **storage**, a **visibility** walk and a **shading** component, each a WGSL module
//! from `components/`, pasted into one compute shader before compilation, so the compiler
//! optimises across them and the split costs nothing at run time.
//!
//! Storages answer the interface in `mr_components::storage`; optional fast-path queries the
//! chosen storage lacks get generic fallbacks, so every storage works with every
//! visibility component. Shadow rays use the same composed walk. Per-slot side data lives
//! in `aux`, indexed like the shared pool. Meshes (mesh mode) are composited by
//! `mr_gpu::meshes::MeshComposite`.

use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use mr_components::{Slot, components, storage, visibility};
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::meshes::{MESH_COMPOSITE_WGSL, MeshComposite};
use mr_gpu::voxels::{Shadows, VoxelWorld, VoxelWorldConfig, shader_source, shadows_param};
use mr_gpu::{FrameRing, Gpu, GpuRequest, GrowableBuffer, wgpu};
use serde::Deserialize;

const CHUNK_BYTES: u64 = CHUNK_VOLUME as u64 * 2;

pub fn entry() -> RendererEntry {
    RendererEntry { info, create: |p, w| Ok(Box::new(GpuCompose::new(p, w)?)) }
}


fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu-compute".into(),
        description: "GPU-compute back end: renders a composition of components (storage, visibility, shading) fused into one compute shader".into(),
        tags: vec!["gpu".into(), "compute".into(), "raycast".into(), "modular".into(), "wgpu".into(), "meshes".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: params(),
    }
}

fn names(slot: Slot) -> Vec<&'static str> {
    components(slot).filter(|c| c.backends.contains(&"gpu-compute")).map(|c| c.name).collect()
}

fn describe(slot: Slot) -> String {
    components(slot).filter(|c| c.backends.contains(&"gpu-compute")).map(|c| format!("{}: {}", c.name, c.description)).collect::<Vec<_>>().join(" ")
}

fn params() -> Vec<ParamSpec> {
    let mut p = vec![
        ParamSpec::choice("storage", &format!("Storage component. {}", describe(Slot::Storage)), &names(Slot::Storage), "bricks", false),
        ParamSpec::choice("visibility", &format!("Visibility component (any storage works with any; missing fast paths use generic fallbacks). {}", describe(Slot::Visibility)), &names(Slot::Visibility), "refloor", true),
        ParamSpec::choice("shading", &format!("Shading component. {}", describe(Slot::Shading)), &names(Slot::Shading), "direct", true),
        ParamSpec::bool("exact", "incremental: recompute exit times from the integer cell after each move (no float drift) instead of accumulating them", true, true),
    ];
    for (k, size) in [(1, 2), (2, 4), (3, 8), (4, 16)] {
        p.push(ParamSpec::bool(&format!("skip{size}"), &format!("incremental: use {size}^3 cubes (level {k}) if the storage has them"), true, true));
    }
    p.push(ParamSpec::choice("view", "image: the rendered scene. steps: primary-ray loop iterations per pixel (red = low 8 bits, green = high bits, blue = log-scale brightness)", &["image", "steps"], "image", true));
    p.extend([
        shadows_param(),
        ParamSpec::int("max_steps", "Loop iteration limit per ray", 32, 16384, 2048, true),
        ParamSpec::int("pool_mb", "Voxel pool limit (MiB)", 64, 6144, 2048, false),
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
    exact: bool,
    skip2: bool,
    skip4: bool,
    skip8: bool,
    skip16: bool,
    view: String,
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
            exact: true,
            skip2: true,
            skip4: true,
            skip8: true,
            skip16: true,
            view: "image".into(),
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
    exact: bool,
    /// Levels allowed by the skip knobs (bit k = 2^k cubes).
    levels: u32,
    /// `view = "steps"`: write iteration counts instead of colour.
    steps_view: bool,
}

impl Composition {
    fn storage(&self) -> storage::GpuStorage {
        storage::gpu(&self.storage).expect("validated")
    }

    fn source(&self) -> String {
        let st = self.storage();
        let vis = visibility::gpu_wgsl(&self.visibility).expect("validated");
        let shade = mr_components::shading::gpu_wgsl(&self.shading).expect("validated");
        let own = format!(
            "// Composition: storage = {}, visibility = {}, shading = {}\n\
             @group(2) @binding(0) var<storage, read> aux: array<u32>;\n\
             const AUX_WORDS: u32 = {}u;\n\
             const EXACT: bool = {};\n\
             const VIEW_STEPS: bool = {};\n\
             const LEVEL_MASK: u32 = (S_LEVELS & {:#x}u) | 0x21u;\n\
             {MESH_COMPOSITE_WGSL}\n{}\n{}\n{vis}\n{shade}\n{}",
            self.storage,
            self.visibility,
            self.shading,
            st.aux_words.max(1),
            self.exact,
            self.steps_view,
            self.levels,
            st.wgsl,
            storage::fallbacks(st.provides),
            include_str!("main.wgsl"),
        );
        let occlusion = "fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool { return ctrace(p, d, max_t).hit; }\n";
        shader_source(occlusion, &own)
    }
}

/// Checks that `name` is a `slot` component with a gpu-compute implementation.
fn check(slot: Slot, name: &str) -> anyhow::Result<String> {
    match mr_components::unavailable("gpu-compute", slot, name) {
        None => Ok(name.to_string()),
        Some(why) => anyhow::bail!(why),
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
    _pad: [u32; 2],
}

pub struct GpuCompose {
    gpu: Gpu,
    world: VoxelWorld,
    /// Mesh mode: meshes rasterised first, rays capped at their depth.
    meshes: MeshComposite,
    comp: Composition,
    /// Composition the pipeline was built from (rebuilt lazily when knobs change it).
    built: Composition,
    pipeline: wgpu::ComputePipeline,
    pipeline_layout: wgpu::PipelineLayout,
    layout0: wgpu::BindGroupLayout,
    layout2: wgpu::BindGroupLayout,
    camera_buf: wgpu::Buffer,
    output: wgpu::Texture,
    bind0: wgpu::BindGroup,
    /// Per-slot storage data (`prefix`), or a placeholder.
    aux: GrowableBuffer,
    bind2: Option<wgpu::BindGroup>,
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
            storage: check(Slot::Storage, &opts.storage)?,
            visibility: check(Slot::Visibility, &opts.visibility)?,
            shading: check(Slot::Shading, &opts.shading)?,
            exact: opts.exact,
            steps_view: parse_view(&opts.view)?,
            levels: [opts.skip2, opts.skip4, opts.skip8, opts.skip16].iter().enumerate().fold(0, |m, (i, &on)| m | (on as u32) << (i + 1)),
        };
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
                max_lod: 0,
                upload_voxels: true,
                pool_mb: opts.pool_mb,
                view_radius: p.view_radius_chunks,
                shadows: opts.shadows,
                max_steps: opts.max_steps,
                // Fragment too: the mesh raster pass shades with voxel shadow rays.
                visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
            },
        )?;
        let meshes = MeshComposite::new(&gpu, &world, p.width, p.height);
        let compute = wgpu::ShaderStages::COMPUTE;
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compose camera"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: compute,
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
        let layout2 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compose storage"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: compute,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let pipeline_layout = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compose"),
            bind_group_layouts: &[Some(&layout0), Some(world.layout()), Some(&layout2), Some(meshes.layout())],
            immediate_size: 0,
        });
        let pipeline = Self::pipeline(&gpu, &pipeline_layout, &comp)?;
        // Side data covers the same slots as the pool, up to the same limit.
        let max_slots = (opts.pool_mb as u64 * 1024 * 1024 / CHUNK_BYTES).max(1);
        let aux_bytes = comp.storage().aux_words as u64 * 4;
        let aux = GrowableBuffer::new(&gpu, "compose aux", 256, (max_slots * aux_bytes).max(256), wgpu::BufferUsages::STORAGE);
        let camera_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let output = mr_gpu::output_texture(&gpu, p.width, p.height);
        let bind0 = Self::bind0(&gpu, &layout0, &camera_buf, &output);
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
            pipeline_layout,
            layout0,
            layout2,
            camera_buf,
            output,
            bind0,
            aux,
            bind2: None,
            ring,
            width: p.width,
            height: p.height,
            depth,
            waker,
        })
    }

    fn pipeline(gpu: &Gpu, layout: &wgpu::PipelineLayout, comp: &Composition) -> anyhow::Result<wgpu::ComputePipeline> {
        let source = comp.source();
        let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("compose"), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let pipeline = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("compose"),
            layout: Some(layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        if let Some(e) = pollster::block_on(scope.pop()) {
            anyhow::bail!("compiling {comp:?}: {e}");
        }
        Ok(pipeline)
    }

    fn bind0(gpu: &Gpu, layout: &wgpu::BindGroupLayout, cam: &wgpu::Buffer, out: &wgpu::Texture) -> wgpu::BindGroup {
        let view = out.create_view(&Default::default());
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("compose camera"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: cam.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
            ],
        })
    }

    /// Bytes of side data per pool slot for the chosen storage (0 = none).
    fn aux_bytes(&self) -> u64 {
        self.comp.storage().aux_words as u64 * 4
    }
}

impl Renderer for GpuCompose {
    fn update_meshes(&mut self, update: MeshUpdate) -> anyhow::Result<()> {
        self.meshes.meshes.apply(update);
        Ok(())
    }

    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        let changed = self.world.apply(&self.gpu, updates);
        let unit = self.aux_bytes();
        if unit > 0 {
            // Keep the side data indexed like the pool: every changed dense chunk rewrites
            // its slot (slots of unloaded chunks are simply no longer referenced).
            if self.aux.reserve(&self.gpu, self.world.pool_slots() as u64 * unit)? {
                self.bind2 = None;
            }
            let build = self.comp.storage().build;
            for pos in changed {
                if let (Some(slot), Some(ChunkData::Dense(d))) = (self.world.slot_of(pos), self.world.chunk(pos)) {
                    self.gpu.queue.write_buffer(self.aux.buffer(), slot as u64 * unit, bytemuck::cast_slice(&build(d)));
                }
            }
        }
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        if self.comp != self.built {
            self.pipeline = Self::pipeline(&self.gpu, &self.pipeline_layout, &self.comp)?;
            self.built = self.comp.clone();
        }
        let ticket = self.ring.begin(&self.gpu, sink)?;
        let start = Instant::now();
        let c = &req.camera;
        let (_, frac) = c.split_position();
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
            _pad: [0; 2],
        };
        self.gpu.queue.write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&cam));
        let world_bg = self.world.prepare(&self.gpu, c).clone();
        if self.bind2.is_none() {
            self.bind2 = Some(self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("compose storage"),
                layout: &self.layout2,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: self.aux.buffer().as_entire_binding() }],
            }));
        }
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        // With meshes, the mesh pass carries the frame's begin timestamp.
        let meshes_on = self.meshes.active(req.meshes);
        let ts = if meshes_on { self.ring.render_timestamps(ticket, true, false) } else { None };
        self.meshes.encode(&self.gpu, &mut enc, c, self.width as f32 / self.height as f32, req.mode, req.meshes, &world_bg, ts);
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("compose"),
                timestamp_writes: self.ring.compute_timestamps(ticket, !meshes_on, true),
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.set_bind_group(2, self.bind2.as_ref(), &[]);
            pass.set_bind_group(3, self.meshes.bind_group(), &[]);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
        }
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            counters: vec![("dropped_chunks".into(), self.world.dropped_chunks() as f64)],
        };
        self.ring.submit(&self.gpu, enc, ticket, &self.output, req.frame_id, req.readback, stats);
        self.ring.poll(&self.gpu, sink, false)
    }

    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        match name {
            "shadows" => self.world.cfg.shadows.set(value)?,
            "max_steps" => self.world.cfg.max_steps = value.as_i64().unwrap_or(2048) as u32,
            "visibility" | "traversal" => self.comp.visibility = check(Slot::Visibility, value.as_str().unwrap_or(""))?,
            "shading" => self.comp.shading = check(Slot::Shading, value.as_str().unwrap_or(""))?,
            "view" => self.comp.steps_view = parse_view(value.as_str().unwrap_or(""))?,
            "exact" => self.comp.exact = value.as_bool().unwrap_or(true),
            "skip2" | "skip4" | "skip8" | "skip16" => {
                let bit = 1 << name[4..].parse::<u32>()?.trailing_zeros();
                if value.as_bool().unwrap_or(true) {
                    self.comp.levels |= bit;
                } else {
                    self.comp.levels &= !bit;
                }
            }
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
        self.bind0 = Self::bind0(&self.gpu, &self.layout0, &self.camera_buf, &self.output);
        self.ring = FrameRing::new(&self.gpu, width, height, self.depth, self.waker.clone());
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        let aux = if self.aux_bytes() > 0 { self.aux.size() } else { 0 };
        MemoryReport { cpu_bytes: 0, gpu_bytes: self.world.memory_bytes() + aux + self.width as u64 * self.height as u64 * 4 }
    }
}
