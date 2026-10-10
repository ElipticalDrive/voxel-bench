//! CPU back end: assembles a renderer from components (`mr-components`, IDEAS §12): a
//! **storage** (`Repr`, one generic instantiation per storage: monomorphised, so the split
//! costs nothing), a **visibility** walk (`refloor` with SIMD-friendly ray packets, or
//! `incremental`) and the **shading** component (`direct`), multi-threaded with rayon.
//! Optional hardware counters (`perf` knob) via `perf_event_open` report cycles,
//! instructions and cache misses per pixel.

mod perf;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use cpu_reference::Shadows;
use glam::{IVec3, Vec3};
use mr_components::storage::bits::cpu::Bits;
use mr_components::storage::bricks::cpu::U16;
use mr_components::storage::cpu::{Entry, Grid, Repr};
use mr_components::storage::hier::cpu::Hier;
use mr_components::storage::palette::cpu::Palette;
use mr_components::storage::prefix::cpu::Prefix;
use mr_components::storage::u8::cpu::U8;
use mr_components::visibility::cpu::Hit;
use mr_components::visibility::incremental::cpu::trace_incremental;
use mr_components::visibility::refloor::cpu::trace_packet;
use mr_components::{Slot, components};
use mr_core::chunk::local_of_block;
use mr_core::lighting::encode_srgb;
use mr_core::*;
use rayon::prelude::*;
use serde::Deserialize;

pub fn entry() -> RendererEntry {
    RendererEntry {
        info: || RendererInfo {
            name: "cpu".into(),
            description: "CPU back end: renders a composition of components (storage, visibility, shading), multi-threaded".into(),
            tags: vec!["cpu".into(), "raycast".into(), "simd".into()],
            modes: vec![RenderMode::Unlit, RenderMode::Lit],
            max_lod: 0,
            params: params(),
        },
        create: |p, _| {
            let opts: Options = p.options()?;
            let storage = check(Slot::Storage, &opts.storage)?;
            Ok(match storage.as_str() {
                "bricks" => Box::new(CpuBackend::<U16>::new(p)?),
                "u8" => Box::new(CpuBackend::<U8>::new(p)?),
                "bits" => Box::new(CpuBackend::<Bits>::new(p)?),
                "hier" => Box::new(CpuBackend::<Hier>::new(p)?),
                "prefix" => Box::new(CpuBackend::<Prefix>::new(p)?),
                "palette" => Box::new(CpuBackend::<Palette>::new(p)?),
                s => anyhow::bail!("storage `{s}` has no CPU implementation"),
            })
        },
    }
}

/// Checks that `name` is a `slot` component with a CPU implementation.
fn check(slot: Slot, name: &str) -> anyhow::Result<String> {
    match mr_components::unavailable("cpu", slot, name) {
        None => Ok(name.to_string()),
        Some(why) => anyhow::bail!(why),
    }
}

fn names(slot: Slot) -> Vec<&'static str> {
    components(slot).filter(|c| c.backends.contains(&"cpu")).map(|c| c.name).collect()
}

fn params() -> Vec<ParamSpec> {
    let mut p = cpu_reference::cpu_params();
    p.push(ParamSpec::choice("storage", "Storage component (CPU implementations)", &names(Slot::Storage), "bricks", false));
    p.push(ParamSpec::choice("visibility", "Visibility component. refloor: probe for the largest empty cube, step out, re-floor (ray packets). incremental: hierarchical DDA keeping per-level exit times (packets ignored)", &names(Slot::Visibility), "refloor", true));
    p.push(ParamSpec::choice("shading", "Shading component", &names(Slot::Shading), "direct", true));
    p.push(ParamSpec::bool("exact", "incremental: recompute exit times from the integer cell (no float drift) instead of accumulating them", true, true));
    p.push(ParamSpec::choice("packet", "Rays traced together in lockstep (refloor; measured: no gain on Zen 5)", &["1", "4", "8", "16"], "1", true));
    p.push(ParamSpec::choice("isa", "Instruction set for the hot loop", &["auto", "scalar", "avx2", "avx512"], "auto", true));
    for (k, size) in [(1, 2), (2, 4), (3, 8), (4, 16)] {
        p.push(ParamSpec::bool(&format!("skip{size}"), &format!("incremental: use {size}^3 cubes (level {k}) if the storage has them. Fewer levels mean fewer failed ascent tests but smaller skips"), true, true));
    }
    p.push(ParamSpec::bool("perf", "Hardware counters per frame (perf_event_open; cycles, instructions, cache misses per pixel)", false, true));
    p
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    threads: usize,
    shadows: Shadows,
    storage: String,
    visibility: String,
    shading: String,
    exact: bool,
    packet: String,
    isa: String,
    skip2: bool,
    skip4: bool,
    skip8: bool,
    skip16: bool,
    perf: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { threads: 0, shadows: Shadows::All, storage: "bricks".into(), visibility: "refloor".into(), shading: "direct".into(), exact: true, packet: "1".into(), isa: "auto".into(), skip2: true, skip4: true, skip8: true, skip16: true, perf: false }
    }
}

/// The visibility walk, with the `exact` knob folded in.
#[derive(Copy, Clone, PartialEq, Debug)]
enum Traversal {
    /// `trace_packet`: the probe returns a skip size, and coarse steps re-floor.
    Refloor,
    /// `trace_incremental`, exit times accumulated by adds.
    Incremental,
    /// `trace_incremental`, exit times recomputed from the integer cell (one multiply).
    IncrementalExact,
}

fn traversal(visibility: &str, exact: bool) -> anyhow::Result<Traversal> {
    Ok(match check(Slot::Visibility, visibility)?.as_str() {
        "refloor" => Traversal::Refloor,
        "incremental" if exact => Traversal::IncrementalExact,
        "incremental" => Traversal::Incremental,
        v => anyhow::bail!("visibility `{v}` has no CPU implementation"),
    })
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum Isa {
    Scalar,
    Avx2,
    Avx512,
}

fn detect(choice: &str) -> Isa {
    let avx512 = std::arch::is_x86_feature_detected!("avx512f") && std::arch::is_x86_feature_detected!("avx512vl") && std::arch::is_x86_feature_detected!("avx512bw");
    let avx2 = std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("fma");
    match choice {
        "scalar" => Isa::Scalar,
        "avx2" if avx2 => Isa::Avx2,
        "avx512" if avx512 => Isa::Avx512,
        "auto" if avx512 => Isa::Avx512,
        "auto" if avx2 => Isa::Avx2,
        _ => Isa::Scalar,
    }
}

// ------------------------------------------------------------------------- tracing

/// Per-row totals: lockstep lane steps (refloor) or occupancy tests (incremental), plus the
/// incremental breakdown.
#[derive(Copy, Clone, Default)]
struct RowWork {
    lane_steps: u64,
    steps: u64,
    up: u64,
    down: u64,
}

impl std::ops::Add for RowWork {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self { lane_steps: self.lane_steps + o.lane_steps, steps: self.steps + o.steps, up: self.up + o.up, down: self.down + o.down }
    }
}

struct Scene<R> {
    blocks: Arc<BlockRegistry>,
    env: Environment,
    chunks: HashMap<IVec3, Entry<R>>,
    grid: Grid<R>,
    grid_dirty: bool,
    lights: HashMap<u32, Light>,
    light_grid: HashMap<IVec3, Vec<Light>>,
    lights_dirty: bool,
    shadows: Shadows,
    traversal: Traversal,
    /// Levels the incremental traversals may use (bit k = 2^k cubes), from the `skip*` knobs.
    levels: u32,
}

const LIGHT_CELL_BITS: i32 = 4;

impl<R: Repr> Scene<R> {
    fn rebuild_grid(&mut self) {
        let mut it = self.chunks.keys();
        let Some(&first) = it.next() else {
            self.grid = Grid::default();
            self.grid_dirty = false;
            return;
        };
        let (lo, hi) = it.fold((first, first), |(lo, hi), &k| (lo.min(k), hi.max(k)));
        let dims = hi - lo + 1;
        let mut cells = vec![0u32; (dims.x * dims.y * dims.z) as usize];
        let mut entries = Vec::with_capacity(self.chunks.len());
        for (pos, e) in &self.chunks {
            entries.push(e.clone());
            let c = *pos - lo;
            cells[((c.z * dims.y + c.y) * dims.x + c.x) as usize] = entries.len() as u32;
        }
        self.grid = Grid { min: lo, dims, cells, entries };
        self.grid_dirty = false;
    }

    fn rebuild_lights(&mut self) {
        self.light_grid.clear();
        for l in self.lights.values() {
            let lo = (l.position - l.radius as f64).floor().as_ivec3() >> LIGHT_CELL_BITS;
            let hi = (l.position + l.radius as f64).floor().as_ivec3() >> LIGHT_CELL_BITS;
            for z in lo.z..=hi.z {
                for y in lo.y..=hi.y {
                    for x in lo.x..=hi.x {
                        self.light_grid.entry(IVec3::new(x, y, z)).or_default().push(*l);
                    }
                }
            }
        }
        self.lights_dirty = false;
    }

    fn bounds_rel(&self, cam: IVec3) -> (IVec3, IVec3) {
        if self.grid.cells.is_empty() {
            return (IVec3::ONE, IVec3::ZERO);
        }
        (self.grid.min * 32 - cam, (self.grid.min + self.grid.dims) * 32 - cam)
    }

    fn occluded(&self, cam: IVec3, p: Vec3, d: Vec3, dist: f32) -> bool {
        let (bmin, bmax) = self.bounds_rel(cam);
        match self.traversal {
            Traversal::Refloor => {}
            Traversal::Incremental => return trace_incremental::<R, false>(&self.grid, self.levels, cam, p, d, dist, bmin, bmax).0.is_some(),
            Traversal::IncrementalExact => return trace_incremental::<R, true>(&self.grid, self.levels, cam, p, d, dist, bmin, bmax).0.is_some(),
        }
        let mut out = [None];
        trace_packet::<R, 1>(&self.grid, cam, p, &[d.x], &[d.y], &[d.z], dist, bmin, bmax, &mut out);
        out[0].is_some()
    }

    fn shade(&self, cam: IVec3, o: Vec3, d: Vec3, h: &Hit, mode: RenderMode) -> [u8; 4] {
        let block = self.grid.block_at(h.cell + cam);
        let face = Face::entered(h.axis as usize, if d[h.axis as usize] > 0.0 { 1 } else { -1 });
        let ctx = mr_components::shading::direct::cpu::Ctx {
            blocks: &self.blocks,
            env: &self.env,
            cam,
            sun_shadows: self.shadows != Shadows::None,
            light_shadows: self.shadows == Shadows::All,
            lights_near: |world: IVec3| self.light_grid.get(&(world >> LIGHT_CELL_BITS)).map(|v| v.as_slice()),
            occluded: |p: Vec3, d: Vec3, dist: f32| self.occluded(cam, p, d, dist),
        };
        mr_components::shading::direct::cpu::shade(&ctx, block, face, h.cell, o + d * h.t, mode)
    }
}

// ------------------------------------------------------------------------- renderer

struct Pool {
    pool: rayon::ThreadPool,
    threads: usize,
}

impl Pool {
    fn new(threads: usize) -> anyhow::Result<Self> {
        Ok(Self { pool: rayon::ThreadPoolBuilder::new().num_threads(threads).build()?, threads })
    }
}

pub struct CpuBackend<R> {
    width: u32,
    height: u32,
    scene: Scene<R>,
    pool: Pool,
    perf: bool,
    counters: Option<perf::Counters>,
    packet: usize,
    isa: Isa,
    isa_choice: String,
    /// `visibility` and `exact` knobs (folded into `scene.traversal`).
    visibility: String,
    exact: bool,
    pixels: Vec<u8>,
}

impl<R: Repr> CpuBackend<R> {
    pub fn new(p: InitParams) -> anyhow::Result<Self> {
        let o: Options = p.options()?;
        if o.storage == "u8" && p.blocks.blocks.len() > 256 {
            anyhow::bail!("storage u8 needs <= 256 block types, registry has {}", p.blocks.blocks.len());
        }
        let isa = detect(&o.isa);
        tracing::info!("cpu: storage {}, visibility {}, {:?} hot loop, packets of {}", o.storage, o.visibility, isa, o.packet);
        check(Slot::Shading, &o.shading)?;
        Ok(Self {
            visibility: o.visibility.clone(),
            exact: o.exact,
            perf: o.perf,
            counters: None,
            width: p.width,
            height: p.height,
            scene: Scene {
                blocks: p.blocks.clone(),
                env: p.environment,
                chunks: HashMap::new(),
                grid: Grid::default(),
                grid_dirty: false,
                lights: HashMap::new(),
                light_grid: HashMap::new(),
                lights_dirty: false,
                shadows: o.shadows,
                traversal: traversal(&o.visibility, o.exact)?,
                levels: [o.skip2, o.skip4, o.skip8, o.skip16].iter().enumerate().fold(0b100001, |m, (i, &on)| m | (on as u32) << (i + 1)),
            },
            pool: Pool::new(o.threads)?,
            packet: o.packet.parse().unwrap_or(1),
            isa,
            isa_choice: o.isa,
            pixels: vec![0; (p.width * p.height * 4) as usize],
        })
    }
}

/// One image row, `N` pixels per packet. `#[inline(always)]` so each ISA wrapper below
/// compiles its own copy with its target features.
#[inline(always)]
fn render_row<R: Repr, const N: usize>(scene: &Scene<R>, cam: &Camera, y: u32, w: u32, h: u32, mode: RenderMode, row: &mut [u8]) -> RowWork {
    let mut work = RowWork::default();
    let (cam_block, o) = cam.split_position();
    // Camera basis once per row (Camera::ray_dir recomputes sin/cos per call).
    let (f, r, u) = (cam.forward(), cam.right(), cam.up());
    let tan = (cam.fov_y * 0.5).tan();
    let aspect = w as f32 / h as f32;
    let sy = (1.0 - 2.0 * (y as f32 + 0.5) / h as f32) * tan;
    let row_dir = f + sy * u;
    let (bmin, bmax) = scene.bounds_rel(cam_block);
    let sky = encode_srgb(scene.env.sky);
    let mut x0 = 0;
    while x0 < w {
        let (mut dx, mut dy, mut dz) = ([f32::NAN; N], [0f32; N], [0f32; N]);
        let mut dirs = [Vec3::ZERO; N];
        for l in 0..N {
            if x0 + (l as u32) < w {
                let sx = (2.0 * ((x0 + l as u32) as f32 + 0.5) / w as f32 - 1.0) * tan * aspect;
                let d = (row_dir + sx * r).normalize();
                (dx[l], dy[l], dz[l]) = (d.x, d.y, d.z);
                dirs[l] = d;
            }
        }
        let mut hits = [None; N];
        if scene.traversal == Traversal::Refloor {
            work.lane_steps += trace_packet::<R, N>(&scene.grid, cam_block, o, &dx, &dy, &dz, 1e7, bmin, bmax, &mut hits) as u64 * N as u64;
        } else {
            for l in 0..N {
                if !dx[l].is_nan() {
                    let (hit, t) = match scene.traversal {
                        Traversal::IncrementalExact => trace_incremental::<R, true>(&scene.grid, scene.levels, cam_block, o, dirs[l], 1e7, bmin, bmax),
                        _ => trace_incremental::<R, false>(&scene.grid, scene.levels, cam_block, o, dirs[l], 1e7, bmin, bmax),
                    };
                    hits[l] = hit;
                    // Occupancy tests: one per step and per descent, plus the first cell and
                    // the ascent attempts.
                    work.lane_steps += (t.steps + t.down + t.up + 1) as u64;
                    work.steps += t.steps as u64;
                    work.up += t.up as u64;
                    work.down += t.down as u64;
                }
            }
        }
        for l in 0..N {
            let x = x0 + l as u32;
            if x >= w {
                break;
            }
            let px = match &hits[l] {
                Some(hit) => scene.shade(cam_block, o, dirs[l], hit, mode),
                None => sky,
            };
            row[x as usize * 4..x as usize * 4 + 4].copy_from_slice(&px);
        }
        x0 += N as u32;
    }
    work
}

#[inline(always)]
fn render_row_dyn<R: Repr>(scene: &Scene<R>, cam: &Camera, y: u32, w: u32, h: u32, mode: RenderMode, row: &mut [u8], packet: usize) -> RowWork {
    match packet {
        1 => render_row::<R, 1>(scene, cam, y, w, h, mode, row),
        4 => render_row::<R, 4>(scene, cam, y, w, h, mode, row),
        16 => render_row::<R, 16>(scene, cam, y, w, h, mode, row),
        _ => render_row::<R, 8>(scene, cam, y, w, h, mode, row),
    }
}

#[target_feature(enable = "avx2,fma")]
fn render_row_avx2<R: Repr>(scene: &Scene<R>, cam: &Camera, y: u32, w: u32, h: u32, mode: RenderMode, row: &mut [u8], packet: usize) -> RowWork {
    render_row_dyn(scene, cam, y, w, h, mode, row, packet)
}

#[target_feature(enable = "avx512f,avx512vl,avx512bw,avx2,fma")]
fn render_row_avx512<R: Repr>(scene: &Scene<R>, cam: &Camera, y: u32, w: u32, h: u32, mode: RenderMode, row: &mut [u8], packet: usize) -> RowWork {
    render_row_dyn(scene, cam, y, w, h, mode, row, packet)
}

impl<R: Repr> CpuBackend<R> {
    /// (bytes read by traversal, bytes kept alive) over all dense chunks.
    fn world_bytes(&self) -> (u64, u64) {
        self.scene.chunks.values().fold((0, 0), |(t, a), e| match e {
            Entry::Dense(r) => (t + r.traversal_bytes() as u64, a + r.total_bytes() as u64),
            Entry::Uniform(_) => (t, a),
        })
    }
}

/// Reconstructs a u16 chunk from any representation (for edits).
fn dense_of<R: Repr>(r: &R) -> DenseChunk {
    r.to_dense()
}

impl<R: Repr> Renderer for CpuBackend<R> {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        let s = &mut self.scene;
        for u in updates {
            match u {
                WorldUpdate::ChunkLoad { key, data } if key.lod == 0 => {
                    match data {
                        ChunkData::Uniform(b) if b.is_air() => {
                            s.chunks.remove(&key.pos);
                        }
                        ChunkData::Uniform(b) => {
                            s.chunks.insert(key.pos, Entry::Uniform(b.0));
                        }
                        ChunkData::Dense(d) => {
                            s.chunks.insert(key.pos, Entry::Dense(R::build(&d)));
                        }
                    }
                    s.grid_dirty = true;
                }
                WorldUpdate::ChunkUnload { key } if key.lod == 0 => {
                    s.chunks.remove(&key.pos);
                    s.grid_dirty = true;
                }
                WorldUpdate::ChunkLoad { .. } | WorldUpdate::ChunkUnload { .. } => {}
                WorldUpdate::SetBlocks(edits) => {
                    // Apply all edits first, then rebuild each touched chunk's bricks once.
                    let mut touched: HashMap<IVec3, ChunkData> = HashMap::new();
                    for e in edits {
                        let pos = ChunkKey::of_block(e.pos).pos;
                        let data = match touched.get_mut(&pos) {
                            Some(d) => d,
                            None => {
                                let Some(entry) = s.chunks.get(&pos) else { continue };
                                let d = match entry {
                                    Entry::Uniform(b) => ChunkData::Uniform(BlockId(*b)),
                                    Entry::Dense(r) => ChunkData::Dense(Arc::new(dense_of(r))),
                                };
                                touched.entry(pos).or_insert(d)
                            }
                        };
                        data.set(local_of_block(e.pos), e.block);
                    }
                    for (pos, data) in touched {
                        let entry = match data.compact() {
                            ChunkData::Uniform(b) if b.is_air() => {
                                s.chunks.remove(&pos);
                                continue;
                            }
                            ChunkData::Uniform(b) => Entry::Uniform(b.0),
                            ChunkData::Dense(d) => Entry::Dense(R::build(&d)),
                        };
                        s.chunks.insert(pos, entry);
                    }
                    s.grid_dirty = true;
                }
                WorldUpdate::LightUpsert(ls) => {
                    for l in ls {
                        s.lights.insert(l.id, l);
                    }
                    s.lights_dirty = true;
                }
                WorldUpdate::LightRemove(ids) => {
                    for id in ids {
                        s.lights.remove(&id);
                    }
                    s.lights_dirty = true;
                }
                WorldUpdate::SetEnvironment(e) => s.env = e,
            }
        }
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let start = Instant::now();
        if self.scene.grid_dirty {
            self.scene.rebuild_grid();
        }
        if self.scene.lights_dirty {
            self.scene.rebuild_lights();
        }
        let (w, h) = (self.width, self.height);
        let (scene, isa, packet) = (&self.scene, self.isa, self.packet);
        let cam = req.camera;
        let mode = req.mode;
        if self.perf && self.counters.is_none() {
            self.counters = Some(perf::Counters::open_on(&self.pool.pool));
        }
        let counters = if self.perf { self.counters.as_ref() } else { None };
        let mut pixels = std::mem::take(&mut self.pixels);
        if let Some(c) = counters {
            c.start();
        }
        let work: RowWork = self.pool.pool.install(|| {
            pixels.par_chunks_mut((w * 4) as usize).enumerate().map(|(y, row)| {
                let y = y as u32;
                match isa {
                    // SAFETY: only selected when the CPU reports these features.
                    Isa::Avx512 => unsafe { render_row_avx512(scene, &cam, y, w, h, mode, row, packet) },
                    Isa::Avx2 => unsafe { render_row_avx2(scene, &cam, y, w, h, mode, row, packet) },
                    Isa::Scalar => render_row_dyn(scene, &cam, y, w, h, mode, row, packet),
                }
            }).reduce(RowWork::default, |a, b| a + b)
        });
        let hw = counters.map(|c| c.stop());
        self.pixels = pixels;
        let px = (w * h) as f64;
        let (trav, total) = self.world_bytes();
        let mut counters = vec![
            ("chunks".into(), self.scene.chunks.len() as f64),
            // Lockstep iterations x packet width (work including idle lanes); with
            // incremental traversals, occupancy tests per ray.
            ("lane_steps_per_pixel".into(), work.lane_steps as f64 / px),
            ("world_traversal_mb".into(), trav as f64 / 1048576.0),
            ("world_total_mb".into(), total as f64 / 1048576.0),
        ];
        if self.scene.traversal != Traversal::Refloor {
            counters.push(("dda_steps_per_pixel".into(), work.steps as f64 / px));
            counters.push(("ascent_tests_per_pixel".into(), work.up as f64 / px));
            counters.push(("descents_per_pixel".into(), work.down as f64 / px));
        }
        if let Some(hw) = hw {
            for (&(name, _, _), v) in perf::EVENTS.iter().zip(&hw) {
                if v.is_finite() {
                    counters.push((format!("{name}_per_px"), v / px));
                }
            }
            if hw[0].is_finite() && hw[1].is_finite() {
                counters.push(("ipc".into(), hw[1] / hw[0]));
            }
        }
        let stats = FrameStats { cpu_ms: start.elapsed().as_secs_f32() * 1e3, gpu_ms: None, counters };
        sink.complete(CompletedFrame { frame_id: req.frame_id, stats, pixels: Some(&self.pixels) });
        Ok(())
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        match name {
            "threads" => {
                let n = value.as_i64().unwrap_or(0) as usize;
                if n != self.pool.threads {
                    self.pool = Pool::new(n)?;
                }
            }
            "shadows" => self.scene.shadows = value.as_str().and_then(Shadows::parse).ok_or_else(|| anyhow::anyhow!("bad shadows"))?,
            "perf" => self.perf = value.as_bool().unwrap_or(false),
            "skip2" | "skip4" | "skip8" | "skip16" => {
                let bit = 1 << (name[4..].parse::<u32>()?.trailing_zeros());
                if value.as_bool().unwrap_or(true) {
                    self.scene.levels |= bit;
                } else {
                    self.scene.levels &= !bit;
                }
            }
            "visibility" => {
                self.visibility = value.as_str().unwrap_or("refloor").to_string();
                self.scene.traversal = traversal(&self.visibility, self.exact)?;
            }
            "exact" => {
                self.exact = value.as_bool().unwrap_or(true);
                self.scene.traversal = traversal(&self.visibility, self.exact)?;
            }
            "shading" => {
                check(Slot::Shading, value.as_str().unwrap_or(""))?;
            }
            "packet" => self.packet = value.as_str().and_then(|s| s.parse().ok()).unwrap_or(8),
            "isa" => {
                self.isa_choice = value.as_str().unwrap_or("auto").to_string();
                self.isa = detect(&self.isa_choice);
            }
            _ => anyhow::bail!("unknown parameter `{name}`"),
        }
        Ok(())
    }

    fn flush(&mut self, _: &mut dyn FrameSink) -> anyhow::Result<()> {
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.width = width;
        self.height = height;
        self.pixels = vec![0; (width * height * 4) as usize];
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        let (_, total) = self.world_bytes();
        MemoryReport { cpu_bytes: total + self.scene.grid.cells.len() as u64 * 4 + self.pixels.len() as u64, gpu_bytes: 0 }
    }
}

