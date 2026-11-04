//! The CPU sampler (`cpu-discover`): self-directed (non screen-space) visibility ray
//! casting on the CPU, drawn by the executor's GPU lane. The CPU **storage** discovery rays
//! walk (any CPU representation, refloor walk, monomorphised:
//! `mr_components::visibility::discover::grid`), the **visibility** component (`quadtree`,
//! or `cluster`, the user's face-cluster design);
//! the shading's shadow rays walk the GPU storage `shadow_storage`.
//!
//! Instead of one ray per pixel, an adaptive screen-space quadtree of CPU rays decides
//! where to look so that it casts as few rays as possible while still discovering every
//! block face visible from the camera (the discovery code is the components':
//! `mr_components::visibility::{discover, quadtree, cluster}`). The discovered face set S lives
//! in world space and persists across frames (faces not re-confirmed for `keep_frames`
//! discovery passes expire). S is rasterised on the GPU as one instanced unit quad per
//! face (8-byte face record, analytic UVs, lit mode with ray-traced shadows in the fragment
//! shader). If S is complete, the image equals rasterising the whole world.
//!
//! The upload of the records is the schedule's transfer of the faces from the CPU to the
//! GPU; its timing is the pipelining (the drawer's faces `prev`; the `pipelined` knob spells
//! it): discovery for frame N+1 runs on a rayon pool while the GPU renders frame N,
//! so frame N is drawn with the S discovered from camera N-1 (one frame of visibility
//! latency). `predict` discovers from the camera extrapolated one frame ahead instead, and
//! `margin` widens the discovery FOV, both to cut disocclusion errors. The very first frame
//! (and the first after a resize) discovers synchronously.
//!
//! Counters: `rays` / `rays_per_pixel` (discovery rays), `faces_s` (|S|), `faces_drawn`
//! (valid records uploaded), `discovery_ms` (CPU job time), `wait_ms` (render thread
//! blocked on the job). `audit=true` also casts one CPU ray per pixel for the rendered
//! camera and counts pixels / distinct faces visible but missing from S.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::Instant;

use glam::IVec3;
use mr_components::draw::drawer::{DrawFrame, DrawNodes};
use mr_components::graph::{Checked, Role};
use mr_components::shaders::Walks;
use mr_components::shading::state as shade_state;
use mr_components::visibility::discover::{Output, Settings, State, grid};
use mr_components::{names_on, require, Slot};
use mr_core::*;
use mr_gpu::voxels::{Shadows, shadows_param};
use mr_gpu::{GrowableBuffer, wgpu};
use serde::Deserialize;

use super::{CoreOpts, Draw, FaceSource, SourceSpec};
use crate::Core;

pub(crate) const BACKEND: &str = "cpu-discover";

pub(crate) fn info() -> RendererInfo {
    RendererInfo {
        name: BACKEND.into(),
        description: "CPU-discover back end (CPU storage, quadtree or cluster visibility, shading). Self-directed visibility: CPU rays (adaptive quadtree, or face clusters) discover the visible face set (temporal, pipelined one frame ahead), GPU rasterises only those faces; lit mode adds ray-traced shadows".into(),
        tags: vec!["cpu".into(), "gpu".into(), "raster".into(), "raycast".into(), "hybrid".into(), "discovery".into(), "modular".into(), "wgpu".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: mr_components::lighting_knobs(BACKEND, params()),
    }
}

fn params() -> Vec<ParamSpec> {
    vec![
        ParamSpec::choice("storage", "Storage component: the CPU representation discovery rays walk (refloor)", &names_on(BACKEND, Slot::Storage), "bricks", false),
        ParamSpec::choice("visibility", "Visibility component (was `algorithm`). quadtree: adaptive screen-space tiles judged by corner rays. cluster: seeds on a coarse-to-fine lattice, then flood fills over coplanar faces confirmed by corner rays (was cpu-cluster)", &names_on(BACKEND, Slot::Visibility), "quadtree", true),
        ParamSpec::choice("traversal", "Traversal kernel of the CPU discovery rays and the GPU shadow and bounce rays", &names_on(BACKEND, Slot::Traversal), "refloor", false),
        ParamSpec::choice("shading", "Shading component", &names_on(BACKEND, Slot::Shading), "direct", false),
        ParamSpec::choice("shadow_storage", "GPU storage the GPU's shadow and bounce rays walk (`same`: the GPU twin of the storage discovery rays walk, if it has one); the storage slot is the CPU representation discovery rays walk", &names_on(BACKEND, Slot::ShadowStorage), "bricks", false),
        ParamSpec::choice("shadow_traversal", "GPU kernel of the shadow and bounce rays (`same`: the traversal kernel)", &names_on(BACKEND, Slot::ShadowTraversal), "same", false),
        ParamSpec::int("seed_step", "Cluster: coarsest seed lattice spacing (px, rounded to a power of two)", 1, 256, 16, true).of("visibility", &["cluster"]),
        ParamSpec::choice("sky", "Cluster: corners = a lattice cell at spacing <= sky_tile is sky when all four corner seeds miss; sparse = one missed seed covers its cell (cheaper; thin things may take a few frames to appear)", &["corners", "sparse"], "corners", true).of("visibility", &["cluster"]),
        ParamSpec::choice("view", "image: the rendered scene. Cluster discovery only: clusters = one colour per cluster found this pass; kinds = confirmed faces (all corners visible) green, faces with an occluded corner yellow, faces carried over from earlier passes grey", &["image", "clusters", "kinds"], "image", true),
        ParamSpec::int("tile", "Initial quadtree tile size (px)", 2, 256, 32, true).of("visibility", &["quadtree"]),
        ParamSpec::choice("criterion", "Tile coherence test: all corners on one face plane (then enumerate the plane's faces in the footprint) or on one face", &["plane", "face"], "plane", true).of("visibility", &["quadtree"]),
        ParamSpec::int("sky_tile", "All-miss tiles at most this size (px) are accepted as sky (default 8 for quadtree, 16 for cluster)", 1, 256, 8, true),
        ParamSpec::int("keep_frames", "Discovery passes a face stays in S without being re-confirmed", 1, 1000, 8, true),
        ParamSpec::int("max_footprint", "Max cells enumerated for one coherent tile (else split)", 1, 1 << 20, 4096, true).of("visibility", &["quadtree"]),
        ParamSpec::int("dilate", "Fine-level samples add camera-facing exposed faces around the hit: 0 off, 1 cell+6 neighbours, 2 3x3x3", 0, 2, 0, true).of("visibility", &["quadtree"]),
        ParamSpec::int("dilate_below", "Dilate samples traced at tile sizes <= this (px)", 1, 256, 2, true).of("visibility", &["quadtree"]),
        ParamSpec::bool("predict", "Discover from the camera extrapolated one frame ahead", true, true),
        ParamSpec::float("margin", "Widen the discovery FOV by this fraction", 0.0, 0.5, false, 0.0, true),
        ParamSpec::bool("pipelined", "Discover frame N+1 while the GPU renders frame N (false = synchronous, zero latency)", true, true),
        ParamSpec::bool("audit", "Also cast one CPU ray per pixel and count visible faces missing from S", false, true),
        ParamSpec::int("threads", "Discovery threads (0 = all cores)", 0, 256, 0, false),
        shadows_param(),
        ParamSpec::int("max_steps", "Shadow-ray DDA step limit (compiled into the kernels)", 32, 16384, 2048, false).baked(true),
        ParamSpec::choice("draw", "Draw component (the shared face drawer): raster = depth-tested and shaded per fragment; visbuffer = face ids, then one compute pass shades each pixel; gbuffer = the common hit records, then the common deferred pass (no diagnostic views)", &names_on(BACKEND, Slot::Draw), "raster", true),
        ParamSpec::bool("depth_prepass", "Lit mode, draw = raster: depth pre-pass so lighting runs once per pixel", true, true),
        ParamSpec::int("pool_mb", "Voxel pool limit for shadow rays (MiB)", 64, 6144, 1536, false),
        ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
    ]
    .into_iter()
    .chain(shade_state::params())
    .collect()
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    storage: String,
    /// `algorithm` is its name from before the component split.
    #[serde(alias = "algorithm")]
    visibility: String,
    traversal: String,
    shading: String,
    shadow_storage: String,
    shadow_traversal: String,
    bounces: u32,
    spp: u32,
    accumulate: bool,
    seed_step: u32,
    sky: String,
    view: String,
    tile: u32,
    criterion: String,
    sky_tile: Option<u32>,
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
    draw: String,
    depth_prepass: bool,
    pool_mb: u32,
    frames_in_flight: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            storage: "bricks".into(),
            visibility: "quadtree".into(),
            traversal: "refloor".into(),
            shading: "direct".into(),
            shadow_storage: "bricks".into(),
            shadow_traversal: "same".into(),
            bounces: 1,
            spp: 1,
            accumulate: true,
            seed_step: 16,
            sky: "corners".into(),
            view: "image".into(),
            tile: 32,
            criterion: "plane".into(),
            sky_tile: None,
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
            draw: "raster".into(),
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
            sky_tile: self.sky_tile.unwrap_or(if self.visibility == "cluster" { 16 } else { 8 }),
            keep_frames: self.keep_frames,
            max_footprint: self.max_footprint,
            margin: self.margin,
            dilate: self.dilate,
            dilate_below: self.dilate_below,
            cluster: self.visibility == "cluster",
            seed_step: self.seed_step,
            sky_sparse: self.sky == "sparse",
        }
    }
}

const REC_BYTES: u64 = 8;

/// Bind groups of the GPU storage instances: the shadow rays' at 2, the bounces' after the
/// deferred drawers' images (group 3).
const STORAGE_GROUPS: &[u32] = &[2, 4];

pub(crate) struct CpuSampler {
    /// The CPU world discovery rays walk (the storage monomorphised).
    cpu: grid::CpuWorld,
    opts: Options,
    pool: Arc<rayon::ThreadPool>,
    state: Option<State>,
    pending: Option<Receiver<Output>>,
    prev_cam: Option<Camera>,
    rec_buf: GrowableBuffer,
    /// Per-record tags for the diagnostic views (cluster discovery).
    tag_buf: GrowableBuffer,
    tags_valid: bool,
    bind0: Option<wgpu::BindGroup>,
    /// The nodes of the timed stage: discovery on the CPU.
    vis_nodes: Vec<String>,
    overflowed: bool,
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
        c.pitch = (c.pitch + (cur.pitch - prev.pitch)).clamp(-1.55, 1.55);
    }
    c
}

impl CpuSampler {
    /// `world`: the CPU world of a pipeline compiled for one storage (else the `storage` knob's).
    pub(crate) fn new(p: &InitParams, devices: &Devices, waker: Waker, g: &Checked, world: Option<fn() -> grid::CpuWorld>) -> anyhow::Result<(Core, Draw, Self)> {
        let mut opts: Options = p.options()?;
        let storage = require(BACKEND, Slot::Storage, &opts.storage)?;
        require(BACKEND, Slot::Visibility, &opts.visibility)?;
        require(BACKEND, Slot::Traversal, &opts.traversal)?;
        require(BACKEND, Slot::Draw, &opts.draw)?;
        require(BACKEND, Slot::Shading, &opts.shading)?;
        // The faces cross from the CPU to the GPU: the schedule's transfer, whose timing is
        // the pipelining (`prev`: drawn a frame after they were discovered).
        let schedule = g.schedule().map_err(|e| anyhow::anyhow!(e))?;
        let faces = schedule.transfers.iter().find(|e| e.port == "faces" && g.node(&e.to).role == Role::Draw).ok_or_else(|| anyhow::anyhow!("{BACKEND}: the faces do not cross from the CPU to the GPU"))?;
        opts.pipelined = faces.when == mr_components::graph::When::Prev;
        // The GPU sites' walks: shadow rays (the shadow stages), bounces their own or the same.
        let mut walks = Walks::of(g);
        let shadow = walks.site(g, g.site(Role::Shading, "shadow").ok_or_else(|| anyhow::anyhow!("{BACKEND}: GPU shadow rays are not bound"))?, STORAGE_GROUPS, false);
        let bounce = g.site(Role::Shading, "bounce").map_or(shadow, |s| walks.site(g, s, STORAGE_GROUPS, false));
        walks.set_max_steps(opts.max_steps);
        let o = CoreOpts { shading: &opts.shading, draw: &opts.draw, bounces: opts.bounces, spp: opts.spp, accumulate: opts.accumulate, shadows: opts.shadows, max_steps: opts.max_steps, pool_mb: opts.pool_mb, frames_in_flight: opts.frames_in_flight };
        let (core, drawing) = super::new(p, devices, waker, g, &o, opts.depth_prepass, SourceSpec {
            wgsl: include_str!("records.wgsl").into(),
            walks,
            lead: None,
            sites: [shadow, bounce],
            varyings: true,
            strip: true,
            meshes: None,
            draw_meshes: false,
            features: wgpu::Features::empty(),
            depth_read: false,
            nodes: DrawNodes { draw: g.ids(&[Role::Draw]), shade: g.ids(&[Role::Shading]), extra: Vec::new() },
        })?;
        let rec_buf = GrowableBuffer::new(&core.gpu, "face records", 1 << 22, 2040 << 20, wgpu::BufferUsages::STORAGE);
        let tag_buf = GrowableBuffer::new(&core.gpu, "face tags", 1 << 21, 1020 << 20, wgpu::BufferUsages::STORAGE);
        let pool = Arc::new(rayon::ThreadPoolBuilder::new().num_threads(p.devices.cpu_threads(opts.threads)).thread_name(|i| format!("discover-{i}")).build()?);
        let s = Self {
            cpu: match world {
                Some(world) => world(),
                None => grid::CpuWorld::new(&storage)?,
            },
            opts,
            pool,
            state: Some(State::default()),
            pending: None,
            prev_cam: None,
            rec_buf,
            tag_buf,
            tags_valid: false,
            bind0: None,
            vis_nodes: g.ids(&[Role::Visibility]),
            overflowed: false,
        };
        Ok((core, drawing, s))
    }

    /// Takes back the state from an in-flight job (discarding its result).
    fn cancel_pending(&mut self) {
        if let Some(rx) = self.pending.take()
            && let Ok(out) = rx.recv() {
            self.state = Some(out.state);
        }
        if self.state.is_none() {
            self.state = Some(State::default());
        }
    }

    /// The transfer adapter of the faces (CPU to GPU): the records into `rec_buf`, the
    /// views' tags into `tag_buf`; returns how many records the GPU draws.
    fn upload(&mut self, core: &Core, out: &Output) -> u32 {
        // Full rewrite: S changes little but records are anchor-relative and validated
        // against the current world every pass.
        let n = out.instances.len() as u32;
        let bytes = n as u64 * REC_BYTES;
        match self.rec_buf.reserve(&core.gpu, bytes) {
            Ok(grew) => {
                if grew {
                    self.bind0 = None;
                }
                if n > 0 {
                    core.gpu.queue.write_buffer(self.rec_buf.buffer(), 0, bytemuck::cast_slice(&out.instances));
                }
            }
            Err(e) => {
                if !self.overflowed {
                    tracing::error!("{e:#}");
                    self.overflowed = true;
                }
            }
        }
        // Tags for the views, when the algorithm provides them.
        self.tags_valid = false;
        if out.tags.len() == out.instances.len() && !out.tags.is_empty() && self.opts.view != "image"
            && let Ok(grew) = self.tag_buf.reserve(&core.gpu, out.tags.len() as u64 * 4) {
            if grew {
                self.bind0 = None;
            }
            core.gpu.queue.write_buffer(self.tag_buf.buffer(), 0, bytemuck::cast_slice(&out.tags));
            self.tags_valid = true;
        }
        if bytes <= self.rec_buf.size() { n } else { 0 }
    }
}

impl FaceSource for CpuSampler {
    fn updating(&mut self, updates: &[WorldUpdate]) {
        self.cpu.apply(updates);
    }

    fn updated(&mut self, _: &Core, _: &[IVec3]) {}

    fn render(&mut self, core: &mut Core, drawing: &mut Draw, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        drawing.check_state(req.mode, None)?;
        let ticket = core.ring.begin(&core.gpu, sink)?;
        let start = Instant::now();
        let (w, h) = (core.width, core.height);
        let grid = self.cpu.snapshot();
        let settings = self.opts.settings();
        let c = req.camera;

        // 1. The face set for this frame: from the job launched last frame, or (first
        //    frame / not pipelined) discovered now for this exact camera.
        let out = match self.pending.take() {
            Some(rx) => rx.recv()?,
            None => {
                let st = self.state.take().unwrap_or_default();
                self.pool.install(|| grid.discover(st, &c, w, h, &settings))
            }
        };
        let wait_ms = start.elapsed().as_secs_f64() * 1e3;

        // 2. The faces to the GPU.
        let n = self.upload(core, &out);

        // 3. Optional audit against a full per-pixel cast of the camera actually drawn.
        let audit = if self.opts.audit {
            let t = Instant::now();
            let (px, faces, hit) = self.pool.install(|| grid.audit(&out.state.s, &c, w, h));
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
                let _ = tx.send(g.discover(st, &next_cam, w, h, &settings));
            });
            self.pending = Some(rx);
        }

        // 5. Rasterise S.
        let view = if self.tags_valid {
            match self.opts.view.as_str() {
                "clusters" => 1,
                "kinds" => 2,
                _ => 0,
            }
        } else {
            0
        };
        drawing.write_frame(core, &c, req.mode, view, out.anchor);
        let world_bg = core.world.prepare(&core.gpu, &c).clone();
        let bound = core.storages.prepare(&core.gpu, &core.world);
        let (storage_bg, more) = bound.split();
        core.accum.begin(&core.gpu, &c, req.mode);
        if self.bind0.is_none() {
            self.bind0 = Some(drawing.bind(core, self.rec_buf.buffer(), self.tag_buf.buffer()));
        }
        let mut enc = core.gpu.device.create_command_encoder(&Default::default());
        if core.build_accel(&mut enc, ticket, req) {
            self.bind0 = Some(drawing.bind(core, self.rec_buf.buffer(), self.tag_buf.buffer()));
        }
        let bind0 = self.bind0.clone().unwrap();
        let binds: Vec<&wgpu::BindGroup> = [Some(&bind0), Some(&world_bg), storage_bg].into_iter().flatten().collect();
        let sky = core.sky();
        let clear = wgpu::Color { r: sky[0] as f64 / 255.0, g: sky[1] as f64 / 255.0, b: sky[2] as f64 / 255.0, a: 1.0 };
        let out_view = core.output.create_view(&Default::default());
        let frame = DrawFrame {
            out: &out_view,
            depth: &drawing.depth,
            clear,
            prepass: drawing.depth_prepass && req.mode == RenderMode::Lit,
            keep_depth: false,
            binds: &binds,
            more: &more,
            mesh_verts: None,
            ring: &core.ring,
            ticket,
            ts: (true, true),
            nodes: &drawing.draw_nodes,
        };
        let draw = |pass: &mut wgpu::RenderPass<'_>| {
            if n > 0 {
                pass.draw(0..4, 0..n);
            }
        };
        drawing.drawer.encode(&core.gpu, &mut enc, &frame, &draw, None, None);
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
            ("algorithm_cluster".into(), (self.opts.visibility == "cluster") as u32 as f64),
            ("wait_ms".into(), wait_ms),
        ];
        if let Some((px, faces, hit, ms)) = audit {
            counters.push(("miss_px".into(), px as f64));
            counters.push(("miss_px_pct".into(), 100.0 * px as f64 / pixels));
            counters.push(("miss_faces".into(), faces as f64));
            counters.push(("hit_px".into(), hit as f64));
            counters.push(("audit_ms".into(), ms));
        }
        counters.extend(out.extra);
        let legend = match (self.tags_valid, self.opts.view.as_str()) {
            (true, "clusters") => Some(Legend::categories(&[("one random colour per cluster found this pass", None), ("carried over from earlier passes (dark grey)", Some([64, 64, 64]))])),
            (true, "kinds") => Some(Legend::categories(&[
                ("confirmed: all corners visible (green tint)", Some([51, 230, 77])),
                ("partial: an occluded corner (yellow tint)", Some([255, 217, 26])),
                ("carried over from earlier passes (dark grey)", Some([64, 64, 64])),
            ])),
            _ => None,
        };
        counters.extend(core.accum.counter(req.mode));
        counters.extend(core.storages.counters());
        // Discovery's own time (pipelined: overlapping the previous frame's drawing).
        let (accel, accel_stages) = core.accel_stats();
        counters.extend(accel);
        let stages = [StageTime::cpu("discover", &self.vis_nodes, out.ms)].into_iter().chain(accel_stages).collect();
        let stats = FrameStats { cpu_ms: start.elapsed().as_secs_f32() * 1e3, gpu_ms: None, counters, legend, stages };
        core.ring.submit(&core.gpu, enc, ticket, &core.output, req.frame_id, req.readback, stats);
        core.ring.poll(&core.gpu, sink, false)
    }

    fn set_param(&mut self, _: &mut Core, _: &mut Draw, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        let int = |d: i64| value.as_i64().unwrap_or(d) as u32;
        match name {
            "tile" => self.opts.tile = int(32),
            "algorithm" | "visibility" => match require(BACKEND, Slot::Visibility, value.as_str().unwrap_or("quadtree")) {
                Ok(v) => self.opts.visibility = v,
                Err(e) => return Some(Err(e)),
            },
            "seed_step" => self.opts.seed_step = int(16),
            "sky" => self.opts.sky = value.as_str().unwrap_or("corners").to_string(),
            "view" => self.opts.view = value.as_str().unwrap_or("image").to_string(),
            "criterion" => self.opts.criterion = value.as_str().unwrap_or("plane").to_string(),
            "sky_tile" => self.opts.sky_tile = Some(int(8)),
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
            _ => return None,
        }
        Some(Ok(()))
    }

    fn resize(&mut self, _: &Core, _: &Draw) {
        self.cancel_pending();
        self.bind0 = None;
    }

    fn memory(&self, core: &Core) -> (u64, u64) {
        let s = self.state.as_ref().map_or(0, |s| s.s.len() as u64 * 16);
        (self.cpu.memory() + s + (core.width as u64 + 1) * (core.height as u64 + 1) * 36, self.rec_buf.size())
    }
}
