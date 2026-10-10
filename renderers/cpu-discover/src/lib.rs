//! `cpu-discover`: self-directed (non screen-space) visibility ray casting on the CPU,
//! pipelined with a GPU rasteriser.
//!
//! Instead of one ray per pixel, an adaptive screen-space quadtree of CPU rays decides
//! where to look so that it casts as few rays as possible while still discovering every
//! block face visible from the camera (see [`discover`]). The discovered face set S lives
//! in world space and persists across frames (faces not re-confirmed for `keep_frames`
//! discovery passes expire). S is rasterised on the GPU as one instanced unit quad per
//! face (the `gpu-raster-naive` backend: 8-byte face record, analytic UVs, lit mode with
//! ray-traced shadows in the fragment shader). If S is complete, the image equals
//! rasterising the whole world.
//!
//! Pipelining: discovery for frame N+1 runs on a rayon pool while the GPU renders frame N,
//! so frame N is drawn with the S discovered from camera N-1 (one frame of visibility
//! latency). `predict` discovers from the camera extrapolated one frame ahead instead, and
//! `margin` widens the discovery FOV, both to cut disocclusion errors. The very first frame
//! (and the first after a resize) discovers synchronously.
//!
//! Counters: `rays` / `rays_per_pixel` (discovery rays), `faces_s` (|S|), `faces_drawn`
//! (valid records uploaded), `discovery_ms` (CPU job time), `wait_ms` (render thread
//! blocked on the job). `audit=true` also casts one CPU ray per pixel for the rendered
//! camera and counts pixels / distinct faces visible but missing from S.

mod discover;
mod grid;

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use discover::{Output, Settings, State, cast};
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::voxels::{DDA_OCCLUSION_WGSL, Shadows, VoxelWorld, VoxelWorldConfig, shader_source, shadows_param};
use mr_gpu::{FrameRing, Gpu, GpuRequest, GrowableBuffer, wgpu};
use rayon::prelude::*;
use serde::Deserialize;

pub fn entry() -> RendererEntry {
    RendererEntry { info, create: |p, w| Ok(Box::new(CpuDiscover::new(p, w)?)) }
}

fn info() -> RendererInfo {
    RendererInfo {
        name: "cpu-discover".into(),
        description: "Self-directed visibility: CPU adaptive-quadtree rays discover the visible face set (temporal, pipelined one frame ahead), GPU rasterises only those faces; lit mode adds ray-traced shadows".into(),
        tags: vec!["cpu".into(), "gpu".into(), "raster".into(), "raycast".into(), "hybrid".into(), "wgpu".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: params(),
    }
}

fn params() -> Vec<ParamSpec> {
    vec![
        ParamSpec::int("tile", "Initial quadtree tile size (px)", 2, 256, 32, true),
        ParamSpec::choice("criterion", "Tile coherence test: all corners on one face plane (then enumerate the plane's faces in the footprint) or on one face", &["plane", "face"], "plane", true),
        ParamSpec::int("sky_tile", "All-miss tiles at most this size (px) are accepted as sky", 1, 256, 8, true),
        ParamSpec::int("keep_frames", "Discovery passes a face stays in S without being re-confirmed", 1, 1000, 8, true),
        ParamSpec::int("max_footprint", "Max cells enumerated for one coherent tile (else split)", 1, 1 << 20, 4096, true),
        ParamSpec::int("dilate", "Fine-level samples add camera-facing exposed faces around the hit: 0 off, 1 cell+6 neighbours, 2 3x3x3", 0, 2, 0, true),
        ParamSpec::int("dilate_below", "Dilate samples traced at tile sizes <= this (px)", 1, 256, 2, true),
        ParamSpec::bool("predict", "Discover from the camera extrapolated one frame ahead", true, true),
        ParamSpec::float("margin", "Widen the discovery FOV by this fraction", 0.0, 0.5, false, 0.0, true),
        ParamSpec::bool("pipelined", "Discover frame N+1 while the GPU renders frame N (false = synchronous, zero latency)", true, true),
        ParamSpec::bool("audit", "Also cast one CPU ray per pixel and count visible faces missing from S", false, true),
        ParamSpec::int("threads", "Discovery threads (0 = all cores)", 0, 256, 0, false),
        shadows_param(),
        ParamSpec::int("max_steps", "Shadow-ray DDA step limit", 32, 16384, 2048, true),
        ParamSpec::bool("depth_prepass", "Lit mode: depth pre-pass so lighting runs once per pixel", true, true),
        ParamSpec::int("pool_mb", "Voxel pool limit for shadow rays (MiB)", 64, 6144, 1536, false),
        ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
    ]
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    adapter: Option<String>,
    tile: u32,
    criterion: String,
    sky_tile: u32,
    keep_frames: u32,
    max_footprint: u32,
    predict: bool,
    dilate: u32,
    dilate_below: u32,
    margin: f32,
    pipelined: bool,
    audit: bool,
    threads: usize,
    shadows: Shadows,
    max_steps: u32,
    depth_prepass: bool,
    pool_mb: u32,
    frames_in_flight: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            adapter: None,
            tile: 32,
            criterion: "plane".into(),
            sky_tile: 8,
            keep_frames: 8,
            max_footprint: 4096,
            predict: true,
            dilate: 0,
            dilate_below: 2,
            margin: 0.0,
            pipelined: true,
            audit: false,
            threads: 0,
            shadows: Shadows::All,
            max_steps: 2048,
            depth_prepass: true,
            pool_mb: 1536,
            frames_in_flight: 3,
        }
    }
}

impl Options {
    fn settings(&self) -> Settings {
        Settings {
            tile: self.tile.max(1),
            face_criterion: self.criterion == "face",
            sky_tile: self.sky_tile,
            keep_frames: self.keep_frames,
            max_footprint: self.max_footprint,
            margin: self.margin,
            dilate: self.dilate,
            dilate_below: self.dilate_below,
        }
    }
}

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
    _p0: u32,
    size: [f32; 2],
    _p1: [f32; 2],
    anchor: [i32; 4],
}

const REC_BYTES: u64 = 8;

pub struct CpuDiscover {
    gpu: Gpu,
    world: VoxelWorld,
    cpu: grid::CpuWorld,
    opts: Options,
    pool: Arc<rayon::ThreadPool>,
    state: Option<State>,
    pending: Option<Receiver<Output>>,
    prev_cam: Option<Camera>,
    pipe_fill: wgpu::RenderPipeline,
    pipe_depth: wgpu::RenderPipeline,
    pipe_equal: wgpu::RenderPipeline,
    frame_buf: wgpu::Buffer,
    rec_buf: GrowableBuffer,
    layout0: wgpu::BindGroupLayout,
    bind0: Option<wgpu::BindGroup>,
    output: wgpu::Texture,
    depth: wgpu::TextureView,
    ring: FrameRing,
    width: u32,
    height: u32,
    waker: Waker,
    overflowed: bool,
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

/// Wraps an angle difference into (-pi, pi].
fn wrap(a: f32) -> f32 {
    let t = std::f32::consts::TAU;
    a - t * ((a + std::f32::consts::PI) / t).floor()
}

/// Constant-velocity extrapolation of the camera one frame ahead.
fn extrapolate(prev: &Camera, cur: &Camera) -> Camera {
    let mut c = *cur;
    let dp = cur.position - prev.position;
    // A big jump is a cut (e.g. mode switch), not motion.
    if dp.length() < 64.0 {
        c.position += dp;
        c.yaw += wrap(cur.yaw - prev.yaw);
        c.pitch = (c.pitch + (cur.pitch - prev.pitch)).clamp(-1.5707, 1.5707);
    }
    c
}

impl CpuDiscover {
    pub fn new(p: InitParams, waker: Waker) -> anyhow::Result<Self> {
        let opts: Options = p.options()?;
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
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            },
        )?;
        let source = shader_source(DDA_OCCLUSION_WGSL, include_str!("raster.wgsl"));
        let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("cpu-discover raster"), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let buf_entry = |binding, ty, vis| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: vis,
            ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cpu-discover frame"),
            entries: &[
                buf_entry(0, wgpu::BufferBindingType::Uniform, wgpu::ShaderStages::VERTEX_FRAGMENT),
                buf_entry(1, wgpu::BufferBindingType::Storage { read_only: true }, wgpu::ShaderStages::VERTEX),
            ],
        });
        let pl = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cpu-discover"),
            bind_group_layouts: &[Some(&layout0), Some(world.layout())],
            immediate_size: 0,
        });
        let pipeline = |label, write: bool, compare, fragment: bool| {
            let targets = [Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba8Unorm, blend: None, write_mask: wgpu::ColorWrites::ALL })];
            gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pl),
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(write),
                    depth_compare: Some(compare),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: fragment.then(|| wgpu::FragmentState { module: &module, entry_point: Some("fs"), compilation_options: Default::default(), targets: &targets }),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipe_fill = pipeline("fill", true, wgpu::CompareFunction::Greater, true);
        let pipe_depth = pipeline("depth prepass", true, wgpu::CompareFunction::Greater, false);
        let pipe_equal = pipeline("shade equal", false, wgpu::CompareFunction::Equal, true);
        let frame_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame"),
            size: size_of::<FrameUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let rec_buf = GrowableBuffer::new(&gpu, "face records", 1 << 22, 2040 << 20, wgpu::BufferUsages::STORAGE);
        let pool = Arc::new(rayon::ThreadPoolBuilder::new().num_threads(opts.threads).thread_name(|i| format!("discover-{i}")).build()?);
        let fif = opts.frames_in_flight.max(1);
        Ok(Self {
            output: mr_gpu::output_texture(&gpu, p.width, p.height),
            depth: depth_view(&gpu, p.width, p.height),
            ring: FrameRing::new(&gpu, p.width, p.height, fif, waker.clone()),
            gpu,
            world,
            cpu: grid::CpuWorld::default(),
            opts,
            pool,
            state: Some(State::default()),
            pending: None,
            prev_cam: None,
            pipe_fill,
            pipe_depth,
            pipe_equal,
            frame_buf,
            rec_buf,
            layout0,
            bind0: None,
            width: p.width,
            height: p.height,
            waker,
            overflowed: false,
        })
    }

    /// Takes back the state from an in-flight job (discarding its result).
    fn cancel_pending(&mut self) {
        if let Some(rx) = self.pending.take() {
            if let Ok(out) = rx.recv() {
                self.state = Some(out.state);
            }
        }
        if self.state.is_none() {
            self.state = Some(State::default());
        }
    }

    /// One CPU ray per pixel for `cam`; returns (pixels whose face is missing from S,
    /// distinct missing faces, pixels hitting geometry).
    fn audit(&self, grid: &grid::Grid, s: &discover::FaceMap, cam: &Camera) -> (u64, u64, u64) {
        let (w, h) = (self.width, self.height);
        let rays = discover::Rays::new(cam, w, h, 0.0);
        let (bmin, bmax) = grid.bounds_rel(rays.cb);
        let (px, hit, keys) = self.pool.install(|| {
            (0..h)
                .into_par_iter()
                .map(|y| {
                    let (mut miss, mut hit, mut keys) = (0u64, 0u64, vec![]);
                    for x in 0..w {
                        if let Some((k, _, _)) = cast(grid, &rays, bmin, bmax, x as f32 + 0.5, y as f32 + 0.5) {
                            hit += 1;
                            if !s.contains_key(&k) {
                                miss += 1;
                                keys.push(k);
                            }
                        }
                    }
                    (miss, hit, keys)
                })
                .reduce(|| (0, 0, vec![]), |mut a, mut b| {
                    a.2.append(&mut b.2);
                    (a.0 + b.0, a.1 + b.1, a.2)
                })
        });
        let faces: HashSet<u64> = keys.into_iter().collect();
        (px, faces.len() as u64, hit)
    }
}

impl Renderer for CpuDiscover {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        self.cpu.apply(&updates);
        self.world.apply(&self.gpu, updates);
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let ticket = self.ring.begin(&self.gpu, sink)?;
        let start = Instant::now();
        let (w, h) = (self.width, self.height);
        let grid = self.cpu.snapshot();
        let settings = self.opts.settings();
        let c = req.camera;

        // 1. The face set for this frame: from the job launched last frame, or (first
        //    frame / not pipelined) discovered now for this exact camera.
        let out = match self.pending.take() {
            Some(rx) => rx.recv()?,
            None => {
                let st = self.state.take().unwrap_or_default();
                let g = grid.clone();
                self.pool.install(|| discover::discover(st, &g, &c, w, h, &settings))
            }
        };
        let wait_ms = start.elapsed().as_secs_f64() * 1e3;

        // 2. Upload the face records (full rewrite; S changes little but records are
        //    anchor-relative and validated against the current world every pass).
        let n = out.instances.len() as u32;
        let bytes = n as u64 * REC_BYTES;
        match self.rec_buf.reserve(&self.gpu, bytes) {
            Ok(grew) => {
                if grew {
                    self.bind0 = None;
                }
                if n > 0 {
                    self.gpu.queue.write_buffer(self.rec_buf.buffer(), 0, bytemuck::cast_slice(&out.instances));
                }
            }
            Err(e) => {
                if !self.overflowed {
                    tracing::error!("{e:#}");
                    self.overflowed = true;
                }
            }
        }
        let n = if bytes <= self.rec_buf.size() { n } else { 0 };

        // 3. Optional audit against a full per-pixel cast of the camera actually drawn.
        let audit = if self.opts.audit {
            let t = Instant::now();
            let (px, faces, hit) = self.audit(&grid, &out.state.s, &c);
            Some((px, faces, hit, t.elapsed().as_secs_f64() * 1e3))
        } else {
            None
        };

        // 4. Launch discovery for the next frame; it runs while the GPU draws this one.
        let faces_s = out.state.s.len();
        let next_cam = match (&self.prev_cam, self.opts.predict) {
            (Some(prev), true) => extrapolate(prev, &c),
            _ => c,
        };
        self.prev_cam = Some(c);
        self.state = Some(out.state);
        if self.opts.pipelined {
            let st = self.state.take().unwrap();
            let (tx, rx) = channel();
            let g = grid.clone();
            self.pool.spawn(move || {
                let _ = tx.send(discover::discover(st, &g, &next_cam, w, h, &settings));
            });
            self.pending = Some(rx);
        }

        // 5. Rasterise S.
        let frac = c.split_position().1;
        let view_proj = c.projection(w as f32 / h as f32) * c.view_rotation();
        let fu = FrameUniform {
            view_proj: view_proj.to_cols_array_2d(),
            cam_frac: frac.to_array(),
            mode: (req.mode == RenderMode::Lit) as u32,
            forward: c.forward().to_array(),
            tan_half_fov: (c.fov_y * 0.5).tan(),
            right: c.right().to_array(),
            aspect: w as f32 / h as f32,
            up: c.up().to_array(),
            _p0: 0,
            size: [w as f32, h as f32],
            _p1: [0.0; 2],
            anchor: [out.anchor.x, out.anchor.y, out.anchor.z, 0],
        };
        self.gpu.queue.write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&fu));
        let world_bg = self.world.prepare(&self.gpu, &c).clone();
        if self.bind0.is_none() {
            self.bind0 = Some(self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("cpu-discover frame"),
                layout: &self.layout0,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.frame_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: self.rec_buf.buffer().as_entire_binding() },
                ],
            }));
        }
        let bind0 = self.bind0.clone().unwrap();
        let sky = encode_srgb(self.world.env.sky);
        let clear = wgpu::Color { r: sky[0] as f64 / 255.0, g: sky[1] as f64 / 255.0, b: sky[2] as f64 / 255.0, a: 1.0 };
        let out_view = self.output.create_view(&Default::default());
        let prepass = self.opts.depth_prepass && req.mode == RenderMode::Lit;
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        if prepass {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("depth prepass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: self.ring.render_timestamps(ticket, true, false),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipe_depth);
            pass.set_bind_group(0, &bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            if n > 0 {
                pass.draw(0..4, 0..n);
            }
        }
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shade"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &out_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations { load: if prepass { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(0.0) }, store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: self.ring.render_timestamps(ticket, !prepass, true),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(if prepass { &self.pipe_equal } else { &self.pipe_fill });
            pass.set_bind_group(0, &bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            if n > 0 {
                pass.draw(0..4, 0..n);
            }
        }
        let pixels = (w as f64) * (h as f64);
        let mut counters = vec![
            ("rays".into(), out.rays as f64),
            ("rays_per_pixel".into(), out.rays as f64 / pixels),
            ("faces_s".into(), faces_s as f64),
            ("faces_drawn".into(), n as f64),
            ("faces_found".into(), out.faces_found as f64),
            ("quadtree_ms".into(), out.phase_ms[0] as f64),
            ("gather_ms".into(), out.quad_ms[0] as f64),
            ("trace_ms".into(), out.quad_ms[1] as f64),
            ("classify_ms".into(), out.quad_ms[2] as f64),
            ("merge_ms".into(), out.phase_ms[1] as f64),
            ("build_ms".into(), out.phase_ms[2] as f64),
            ("tiles_accepted".into(), out.tiles_accepted as f64),
            ("discovery_ms".into(), out.ms as f64),
            ("wait_ms".into(), wait_ms),
        ];
        if let Some((px, faces, hit, ms)) = audit {
            counters.push(("miss_px".into(), px as f64));
            counters.push(("miss_px_pct".into(), 100.0 * px as f64 / pixels));
            counters.push(("miss_faces".into(), faces as f64));
            counters.push(("hit_px".into(), hit as f64));
            counters.push(("audit_ms".into(), ms));
        }
        let stats = FrameStats { cpu_ms: start.elapsed().as_secs_f32() * 1e3, gpu_ms: None, counters };
        self.ring.submit(&self.gpu, enc, ticket, &self.output, req.frame_id, req.readback, stats);
        self.ring.poll(&self.gpu, sink, false)
    }

    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        let int = |d: i64| value.as_i64().unwrap_or(d) as u32;
        match name {
            "tile" => self.opts.tile = int(32),
            "criterion" => self.opts.criterion = value.as_str().unwrap_or("plane").to_string(),
            "sky_tile" => self.opts.sky_tile = int(8),
            "keep_frames" => self.opts.keep_frames = int(8),
            "max_footprint" => self.opts.max_footprint = int(4096),
            "dilate" => self.opts.dilate = int(0),
            "dilate_below" => self.opts.dilate_below = int(2),
            "predict" => self.opts.predict = value.as_bool().unwrap_or(true),
            "margin" => self.opts.margin = value.as_f64().unwrap_or(0.0) as f32,
            "pipelined" => {
                self.opts.pipelined = value.as_bool().unwrap_or(true);
                if !self.opts.pipelined {
                    self.cancel_pending();
                }
            }
            "audit" => self.opts.audit = value.as_bool().unwrap_or(false),
            "shadows" => self.world.cfg.shadows.set(value)?,
            "max_steps" => self.world.cfg.max_steps = value.as_i64().unwrap_or(2048) as u32,
            "depth_prepass" => self.opts.depth_prepass = value.as_bool().unwrap_or(true),
            _ => anyhow::bail!("`{name}` cannot change while running"),
        }
        Ok(())
    }

    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, true)
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.cancel_pending();
        self.width = width;
        self.height = height;
        self.output = mr_gpu::output_texture(&self.gpu, width, height);
        self.depth = depth_view(&self.gpu, width, height);
        self.ring = FrameRing::new(&self.gpu, width, height, self.opts.frames_in_flight.max(1), self.waker.clone());
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        let gpu = self.world.memory_bytes() + self.rec_buf.size() + self.width as u64 * self.height as u64 * 8;
        let s = self.state.as_ref().map_or(0, |s| s.s.len() as u64 * 16);
        MemoryReport { cpu_bytes: self.cpu.memory() + s + (self.width as u64 + 1) * (self.height as u64 + 1) * 36, gpu_bytes: gpu }
    }
}
