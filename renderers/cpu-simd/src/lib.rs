//! Fast CPU ray caster with SIMD ray packets.
//!
//! Where `cpu-reference` is written for clarity (f64, hash map per chunk step), this one
//! is written for speed:
//! * camera-relative `f32` math and the same multi-level stepping as the GPU ray casters
//!   (empty chunk → 32, empty 4^3 brick → 4, else voxel);
//! * a dense chunk grid over the loaded region (no hashing in the inner loop);
//! * rays traced in lockstep *packets* of N adjacent pixels with structure-of-arrays
//!   state, so the per-step arithmetic vectorises; the hot loop is compiled separately
//!   for AVX2 and AVX-512 and picked at runtime (`isa` knob), alongside a scalar build;
//! * tiles of rows spread over a rayon pool.
//!
//! Knobs `packet` (1/4/8/16) and `isa` make the SIMD contribution measurable. Measured on
//! a Ryzen 9955HX (terrain, 1080p): ~1.5x the reference, and packets of 4–16 lanes with
//! AVX2/AVX-512 are no faster than single rays — the traversal is bound by dependent
//! memory lookups (grid → chunk → brick → voxel), not arithmetic. The biggest single win
//! was avoiding libm `floor` calls (the baseline x86-64 target lacks SSE4.1).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use cpu_reference::{Pool, Shadows};
use glam::{IVec3, Vec3};
use mr_core::chunk::{cell_index, cell_of_index, local_of_block};
use mr_core::lighting::{encode_srgb, point_light, srgb_to_linear, texel_coord};
use mr_core::*;
use rayon::prelude::*;
use serde::Deserialize;

pub fn entry() -> RendererEntry {
    RendererEntry {
        info: || RendererInfo {
            name: "cpu-simd".into(),
            description: "Fast CPU ray caster: f32 multi-level DDA over a dense chunk grid, SIMD ray packets (AVX2/AVX-512, runtime-selected)".into(),
            tags: vec!["cpu".into(), "raycast".into(), "simd".into(), "packets".into()],
            modes: vec![RenderMode::Unlit, RenderMode::Lit],
            max_lod: 0,
            params: params(),
        },
        create: |p, _| Ok(Box::new(CpuSimd::new(p)?)),
    }
}

fn params() -> Vec<ParamSpec> {
    let mut p = cpu_reference::cpu_params();
    p.push(ParamSpec::choice("packet", "Rays traced together in lockstep (measured: no gain on Zen 5 — traversal is memory-latency bound)", &["1", "4", "8", "16"], "1", true));
    p.push(ParamSpec::choice("isa", "Instruction set for the hot loop", &["auto", "scalar", "avx2", "avx512"], "auto", true));
    p
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    threads: usize,
    shadows: Shadows,
    packet: String,
    isa: String,
}

impl Default for Options {
    fn default() -> Self {
        Self { threads: 0, shadows: Shadows::All, packet: "1".into(), isa: "auto".into() }
    }
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

// ------------------------------------------------------------------------- world

#[derive(Clone)]
enum Entry {
    Uniform(u16),
    Dense { blocks: Arc<DenseChunk>, bricks: [u64; 8] },
}

/// Dense grid of chunk entries over the loaded region; index 0 = air.
#[derive(Default)]
struct Grid {
    min: IVec3,
    dims: IVec3,
    cells: Vec<u32>,
    entries: Vec<Entry>,
}

impl Grid {
    /// Entry index (0 = air or outside) of the chunk containing absolute cell `abs`.
    #[inline(always)]
    fn chunk(&self, abs: IVec3) -> u32 {
        let c: IVec3 = (abs >> 5) - self.min;
        if c.x < 0 || c.y < 0 || c.z < 0 || c.x >= self.dims.x || c.y >= self.dims.y || c.z >= self.dims.z {
            return 0;
        }
        self.cells[((c.z * self.dims.y + c.y) * self.dims.x + c.x) as usize]
    }

    /// (block, step size) at absolute cell `abs` inside chunk entry `idx`.
    #[inline(always)]
    fn probe_in(&self, idx: u32, abs: IVec3) -> (u16, i32) {
        if idx == 0 {
            return (0, 32);
        }
        match &self.entries[idx as usize - 1] {
            Entry::Uniform(b) => (b.clone(), 1),
            Entry::Dense { blocks, bricks } => {
                let l: IVec3 = abs & 31;
                let b: IVec3 = l >> 2;
                let bi = (b.x | (b.z << 3) | (b.y << 6)) as u32;
                if bricks[(bi >> 6) as usize] >> (bi & 63) & 1 == 0 {
                    return (0, 4);
                }
                (blocks.0[cell_index(l)].0, 1)
            }
        }
    }
}

fn bricks_of(d: &DenseChunk) -> [u64; 8] {
    let mut bits = [0u64; 8];
    for (i, b) in d.0.iter().enumerate() {
        if b.is_solid() {
            let l: IVec3 = cell_of_index(i) >> 2;
            let bi = (l.x | (l.z << 3) | (l.y << 6)) as usize;
            bits[bi >> 6] |= 1 << (bi & 63);
        }
    }
    bits
}

#[derive(Copy, Clone, Default)]
struct Hit {
    t: f32,
    cell: IVec3,
    block: u16,
    axis: u8,
}

// ------------------------------------------------------------------------- tracing

/// `floor` without a libm call (the baseline x86-64 target has no SSE4.1 `roundss`, so
/// `f32::floor` is a function call — it dominated the step loop).
#[inline(always)]
fn ffloor(x: f32) -> i32 {
    let i = x as i32;
    i - (x < i as f32) as i32
}

/// Traces `N` rays in lockstep from the shared origin `o` (relative to `cam`).
/// Lanes with `d.x == NaN` are inactive. Each lane ignores the start cell (like the
/// reference) and stops at `tfar`.
#[inline(always)]
fn trace_packet<const N: usize>(grid: &Grid, cam: IVec3, o: Vec3, dx: &[f32; N], dy: &[f32; N], dz: &[f32; N], max_t: f32, bmin: IVec3, bmax: IVec3, out: &mut [Option<Hit>; N]) -> u32 {
    let mut iterations = 0u32;
    let mut inv = [[0f32; N]; 3];
    let mut stp = [[0i32; N]; 3];
    let mut cell = [[0i32; N]; 3];
    let mut t = [0f32; N];
    let mut tfar = [0f32; N];
    let mut axis = [3u8; N];
    let mut active = [false; N];
    let start = o.floor().as_ivec3();
    let (bminf, bmaxf) = (bmin.as_vec3(), bmax.as_vec3());
    for l in 0..N {
        out[l] = None;
        if dx[l].is_nan() {
            continue;
        }
        let d = [dx[l], dy[l], dz[l]];
        let mut tlo = [0f32; 3];
        let mut thi = [0f32; 3];
        for a in 0..3 {
            let da = if d[a].abs() < 1e-20 { 1e-20 } else { d[a] };
            inv[a][l] = 1.0 / da;
            stp[a][l] = if da > 0.0 { 1 } else { -1 };
            let t0 = (bminf[a] - o[a]) * inv[a][l];
            let t1 = (bmaxf[a] - o[a]) * inv[a][l];
            tlo[a] = t0.min(t1);
            thi[a] = t0.max(t1);
        }
        let tnear = tlo[0].max(tlo[1]).max(tlo[2]).max(0.0);
        let tf = thi[0].min(thi[1]).min(thi[2]).min(max_t);
        if tnear > tf {
            continue;
        }
        for a in 0..3 {
            cell[a][l] = ffloor(o[a] + d[a] * tnear).clamp(bmin[a], bmax[a] - 1);
        }
        if tnear > 0.0 {
            axis[l] = if tlo[0] >= tlo[1] && tlo[0] >= tlo[2] { 0 } else if tlo[1] >= tlo[2] { 1 } else { 2 };
        }
        t[l] = tnear;
        tfar[l] = tf;
        active[l] = true;
    }
    let mut size = [1i32; N];
    // Per-lane cache of the current chunk (most steps stay inside it).
    let mut cached_chunk = [IVec3::splat(i32::MIN); N];
    let mut cached_idx = [0u32; N];
    for _ in 0..4096 {
        // Lookups (gathers): scalar per lane.
        let mut any = false;
        for l in 0..N {
            if !active[l] {
                continue;
            }
            let c = IVec3::new(cell[0][l], cell[1][l], cell[2][l]);
            let abs = c + cam;
            let ch = abs >> 5;
            if ch != cached_chunk[l] {
                cached_chunk[l] = ch;
                cached_idx[l] = grid.chunk(abs);
            }
            let (b, s) = grid.probe_in(cached_idx[l], abs);
            if b != 0 && c != start {
                out[l] = Some(Hit { t: t[l], cell: c, block: b, axis: axis[l] });
                active[l] = false;
                continue;
            }
            size[l] = s;
            any = true;
        }
        if !any {
            break;
        }
        iterations += 1;
        // Stepping: branch-free arithmetic over all lanes (vectorises).
        for l in 0..N {
            let s = size[l];
            let mut tb = [0f32; 3];
            let mut base = [0i32; 3];
            let mut bound = [0i32; 3];
            for a in 0..3 {
                let abs = cell[a][l] + cam[a];
                base[a] = (abs & -s) - cam[a];
                bound[a] = base[a] + if stp[a][l] > 0 { s } else { 0 };
                tb[a] = (bound[a] as f32 - o[a]) * inv[a][l];
            }
            let a = if tb[0] <= tb[1] && tb[0] <= tb[2] { 0 } else if tb[1] <= tb[2] { 1 } else { 2 };
            let tn = tb[a];
            let d = [dx[l], dy[l], dz[l]];
            for k in 0..3 {
                cell[k][l] = if k == a {
                    if stp[k][l] > 0 { bound[k] } else { base[k] - 1 }
                } else if s == 1 {
                    // Voxel step: only the crossing axis moves.
                    cell[k][l]
                } else {
                    ffloor(o[k] + d[k] * tn).clamp(base[k], base[k] + s - 1)
                };
            }
            t[l] = tn;
            axis[l] = a as u8;
            if tn > tfar[l] {
                active[l] = false;
            }
        }
    }
    iterations
}

struct Scene {
    blocks: Arc<BlockRegistry>,
    env: Environment,
    chunks: HashMap<IVec3, Entry>,
    grid: Grid,
    grid_dirty: bool,
    lights: HashMap<u32, Light>,
    light_grid: HashMap<IVec3, Vec<Light>>,
    lights_dirty: bool,
    shadows: Shadows,
}

const LIGHT_CELL_BITS: i32 = 4;

impl Scene {
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
        let mut out = [None];
        trace_packet::<1>(&self.grid, cam, p, &[d.x], &[d.y], &[d.z], dist, bmin, bmax, &mut out);
        out[0].is_some()
    }

    fn shade(&self, cam: IVec3, o: Vec3, d: Vec3, h: &Hit, mode: RenderMode) -> [u8; 4] {
        let face = Face::entered(h.axis as usize, if d[h.axis as usize] > 0.0 { 1 } else { -1 });
        let p = o + d * h.t;
        let local = p - h.cell.as_vec3();
        let uv = face.uv(local);
        let texel = self.blocks.texture_of(BlockId(h.block), face).texel(texel_coord(uv.x), texel_coord(uv.y));
        if mode == RenderMode::Unlit {
            return texel;
        }
        let albedo = Vec3::new(srgb_to_linear(texel[0]), srgb_to_linear(texel[1]), srgb_to_linear(texel[2]));
        let n = face.normal().as_vec3();
        let ps = p + n * 1e-3;
        let mut light = self.env.ambient;
        if let Some(sun) = self.env.sun {
            let ndl = n.dot(-sun.direction);
            if ndl > 0.0 && (self.shadows == Shadows::None || !self.occluded(cam, ps, -sun.direction, 1e7)) {
                light += sun.color * ndl;
            }
        }
        let world = (cam.as_dvec3() + ps.as_dvec3()).floor().as_ivec3() >> LIGHT_CELL_BITS;
        if let Some(ls) = self.light_grid.get(&world) {
            for l in ls {
                let to = (l.position - cam.as_dvec3()).as_vec3() - ps;
                let c = point_light(n, to, l);
                if c != Vec3::ZERO {
                    let dist = to.length();
                    if self.shadows != Shadows::All || !self.occluded(cam, ps, to / dist, dist) {
                        light += c;
                    }
                }
            }
        }
        encode_srgb(albedo * light * self.env.exposure)
    }
}

// ------------------------------------------------------------------------- renderer

pub struct CpuSimd {
    width: u32,
    height: u32,
    scene: Scene,
    pool: Pool,
    packet: usize,
    isa: Isa,
    isa_choice: String,
    pixels: Vec<u8>,
}

impl CpuSimd {
    pub fn new(p: InitParams) -> anyhow::Result<Self> {
        let o: Options = p.options()?;
        let isa = detect(&o.isa);
        tracing::info!("cpu-simd: {:?} hot loop, packets of {}", isa, o.packet);
        Ok(Self {
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
fn render_row<const N: usize>(scene: &Scene, cam: &Camera, y: u32, w: u32, h: u32, mode: RenderMode, row: &mut [u8]) -> u64 {
    let mut steps = 0u64;
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
        steps += trace_packet::<N>(&scene.grid, cam_block, o, &dx, &dy, &dz, 1e7, bmin, bmax, &mut hits) as u64 * N as u64;
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
    steps
}

#[inline(always)]
fn render_row_dyn(scene: &Scene, cam: &Camera, y: u32, w: u32, h: u32, mode: RenderMode, row: &mut [u8], packet: usize) -> u64 {
    match packet {
        1 => render_row::<1>(scene, cam, y, w, h, mode, row),
        4 => render_row::<4>(scene, cam, y, w, h, mode, row),
        16 => render_row::<16>(scene, cam, y, w, h, mode, row),
        _ => render_row::<8>(scene, cam, y, w, h, mode, row),
    }
}

#[target_feature(enable = "avx2,fma")]
fn render_row_avx2(scene: &Scene, cam: &Camera, y: u32, w: u32, h: u32, mode: RenderMode, row: &mut [u8], packet: usize) -> u64 {
    render_row_dyn(scene, cam, y, w, h, mode, row, packet)
}

#[target_feature(enable = "avx512f,avx512vl,avx512bw,avx2,fma")]
fn render_row_avx512(scene: &Scene, cam: &Camera, y: u32, w: u32, h: u32, mode: RenderMode, row: &mut [u8], packet: usize) -> u64 {
    render_row_dyn(scene, cam, y, w, h, mode, row, packet)
}

impl Renderer for CpuSimd {
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
                            let bricks = bricks_of(&d);
                            s.chunks.insert(key.pos, Entry::Dense { blocks: d, bricks });
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
                                    Entry::Dense { blocks, .. } => ChunkData::Dense(blocks.clone()),
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
                            ChunkData::Dense(d) => {
                                let bricks = bricks_of(&d);
                                Entry::Dense { blocks: d, bricks }
                            }
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
        let mut pixels = std::mem::take(&mut self.pixels);
        let lane_steps: u64 = self.pool.install(|| {
            pixels.par_chunks_mut((w * 4) as usize).enumerate().map(|(y, row)| {
                let y = y as u32;
                match isa {
                    // SAFETY: only selected when the CPU reports these features.
                    Isa::Avx512 => unsafe { render_row_avx512(scene, &cam, y, w, h, mode, row, packet) },
                    Isa::Avx2 => unsafe { render_row_avx2(scene, &cam, y, w, h, mode, row, packet) },
                    Isa::Scalar => render_row_dyn(scene, &cam, y, w, h, mode, row, packet),
                }
            }).sum()
        });
        self.pixels = pixels;
        let stats = FrameStats { cpu_ms: start.elapsed().as_secs_f32() * 1e3, gpu_ms: None, counters: vec![
            ("chunks".into(), self.scene.chunks.len() as f64),
            // Lockstep iterations x packet width: work including idle lanes.
            ("lane_steps_per_pixel".into(), lane_steps as f64 / (w * h) as f64),
        ] };
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
        let dense = self.scene.chunks.values().filter(|e| matches!(e, Entry::Dense { .. })).count() as u64;
        MemoryReport { cpu_bytes: dense * (CHUNK_VOLUME as u64 * 2 + 64) + self.scene.grid.cells.len() as u64 * 4 + self.pixels.len() as u64, gpu_bytes: 0 }
    }
}
