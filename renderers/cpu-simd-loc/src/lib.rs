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

//! Cache-locality / scheduling variants of `cpu-simd`.
//!
//! Same traversal and shading as `cpu-simd` (output is bit-identical), but:
//! * `layout`: how a chunk's 32^3 block ids are ordered in memory — `linear` (x-fastest,
//!   as cpu-simd), `brick` (each 4^3 brick's 64 ids contiguous) or `morton` (Z-order);
//! * `schedule`: `rows` (cpu-simd: one image row per task, packets along the row) or
//!   `tiles` (square tiles walked in Morton/Hilbert order, packets as 2x2/4x2/4x4 blocks);
//! * `prefetch`: software prefetch of the block `prefetch` voxels further along each ray;
//! * `counters`: per-thread hardware counters (perf_event_open) reported per frame.
//!
//! Measured on a Ryzen 9955HX (CCD1, 1080p): the traversal is *not* memory bound —
//! ~2000 cycles and ~4800 instructions per pixel at IPC ~2.3, but only ~0.1 L3/DRAM fills
//! per pixel. Brick-major/Morton ids cut L1d misses ~40% and L2 fills ~25% without
//! changing cycles/pixel (and cost extra re-encoding when chunks stream in); software
//! prefetch only adds instructions. Morton tiles help a little (~4% over row order) by
//! making consecutive rays' branches more predictable (branch misses -20%).

mod layout;
mod perf;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use cpu_reference::Shadows;
use glam::{IVec3, Vec3};
use layout::{BRICK, Ids, LINEAR, MORTON};
use mr_core::chunk::{cell_of_index, local_of_block};
use mr_core::lighting::{encode_srgb, point_light, srgb_to_linear, texel_coord};
use mr_core::*;
use rayon::prelude::*;
use serde::Deserialize;

#[derive(Clone, Copy)]
struct Preset {
    name: &'static str,
    description: &'static str,
    layout: &'static str,
    schedule: &'static str,
}

const PRESETS: &[Preset] = &[
    Preset { name: "cpu-simd-brickmajor", description: "cpu-simd with brick-major block ids (each 4^3 brick = 128 contiguous bytes)", layout: "brick", schedule: "rows" },
    Preset { name: "cpu-simd-morton", description: "cpu-simd with Morton (Z-order) block ids inside chunks", layout: "morton", schedule: "rows" },
    Preset { name: "cpu-simd-tiles", description: "cpu-simd scheduled in Morton-ordered screen tiles with 2D ray packets (+ optional software prefetch)", layout: "linear", schedule: "tiles" },
    Preset { name: "cpu-simd-best", description: "cpu-simd locality combo: Morton block ids + Morton-ordered screen tiles (measured: no better than cpu-simd-tiles)", layout: "morton", schedule: "tiles" },
];

macro_rules! entry_fn {
    ($f:ident, $i:expr) => {
        pub fn $f() -> RendererEntry {
            RendererEntry {
                info: || {
                    let p = PRESETS[$i];
                    RendererInfo {
                        name: p.name.into(),
                        description: p.description.into(),
                        tags: vec!["cpu".into(), "raycast".into(), "simd".into(), "locality".into()],
                        modes: vec![RenderMode::Unlit, RenderMode::Lit],
                        max_lod: 0,
                        params: params(p),
                    }
                },
                create: |p, _| Ok(Box::new(CpuSimdLoc::new(p, PRESETS[$i])?)),
            }
        }
    };
}

entry_fn!(entry_brickmajor, 0);
entry_fn!(entry_morton, 1);
entry_fn!(entry_tiles, 2);
entry_fn!(entry_best, 3);

fn params(p: Preset) -> Vec<ParamSpec> {
    let mut v = cpu_reference::cpu_params();
    v.push(ParamSpec::choice("packet", "Rays traced together in lockstep (tiles: 1, 2x2, 4x2, 4x4 pixel blocks)", &["1", "4", "8", "16"], "1", true));
    v.push(ParamSpec::choice("isa", "Instruction set for the hot loop", &["auto", "scalar", "avx2", "avx512"], "auto", true));
    v.push(ParamSpec::choice("layout", "Block-id order inside a chunk", &["linear", "brick", "morton"], p.layout, true));
    v.push(ParamSpec::choice("schedule", "Work split: image rows, or screen tiles along a space-filling curve", &["rows", "tiles"], p.schedule, true));
    v.push(ParamSpec::choice("tile_order", "Order tiles are handed out in", &["morton", "hilbert", "linear"], "morton", true));
    v.push(ParamSpec::int("tile", "Tile edge in pixels (schedule = tiles)", 4, 256, 16, true));
    v.push(ParamSpec::int("prefetch", "Software-prefetch the block this many voxels ahead along each ray (0 = off)", 0, 64, 0, true));
    v.push(ParamSpec::bool("counters", "Hardware counters per frame (perf_event_open, user space)", false, false));
    v
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    threads: usize,
    shadows: Shadows,
    packet: String,
    isa: String,
    layout: Option<String>,
    schedule: Option<String>,
    tile_order: String,
    tile: u32,
    prefetch: u32,
    counters: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { threads: 0, shadows: Shadows::All, packet: "1".into(), isa: "auto".into(), layout: None, schedule: None, tile_order: "morton".into(), tile: 16, prefetch: 0, counters: false }
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
    Dense { blocks: Arc<Ids>, bricks: [u64; 8] },
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
    fn probe_in<const L: u8>(&self, idx: u32, abs: IVec3) -> (u16, i32) {
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
                (blocks.0[layout::index::<L>(l)], 1)
            }
        }
    }
}

impl Grid {
    /// Software prefetch of the block ids at absolute cell `abs` (no-op for air/uniform).
    #[inline(always)]
    fn prefetch<const L: u8>(&self, abs: IVec3) {
        let idx = self.chunk(abs);
        if idx == 0 {
            return;
        }
        if let Entry::Dense { blocks, .. } = &self.entries[idx as usize - 1] {
            let p = blocks.0.as_ptr().wrapping_add(layout::index::<L>(abs & 31));
            // SAFETY: prefetch never faults; SSE is part of the x86-64 baseline.
            unsafe { core::arch::x86_64::_mm_prefetch::<{ core::arch::x86_64::_MM_HINT_T0 }>(p as *const i8) };
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
fn trace_packet<const N: usize, const L: u8, const PF: bool>(grid: &Grid, cam: IVec3, o: Vec3, dx: &[f32; N], dy: &[f32; N], dz: &[f32; N], max_t: f32, bmin: IVec3, bmax: IVec3, pf: f32, out: &mut [Option<Hit>; N]) -> u32 {
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
            if PF {
                let ta = t[l] + pf;
                let ahead = IVec3::new(ffloor(o.x + dx[l] * ta), ffloor(o.y + dy[l] * ta), ffloor(o.z + dz[l] * ta));
                grid.prefetch::<L>(ahead + cam);
            }
            let (b, s) = grid.probe_in::<L>(cached_idx[l], abs);
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
    prefetch: f32,
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

    fn occluded<const L: u8>(&self, cam: IVec3, p: Vec3, d: Vec3, dist: f32) -> bool {
        let (bmin, bmax) = self.bounds_rel(cam);
        let mut out = [None];
        trace_packet::<1, L, false>(&self.grid, cam, p, &[d.x], &[d.y], &[d.z], dist, bmin, bmax, self.prefetch, &mut out);
        out[0].is_some()
    }

    fn shade<const L: u8>(&self, cam: IVec3, o: Vec3, d: Vec3, h: &Hit, mode: RenderMode) -> [u8; 4] {
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
            if ndl > 0.0 && (self.shadows == Shadows::None || !self.occluded::<L>(cam, ps, -sun.direction, 1e7)) {
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
                    if self.shadows != Shadows::All || !self.occluded::<L>(cam, ps, to / dist, dist) {
                        light += c;
                    }
                }
            }
        }
        encode_srgb(albedo * light * self.env.exposure)
    }
}

// ------------------------------------------------------------------------- renderer

fn build_pool(threads: usize, counters: bool) -> anyhow::Result<rayon::ThreadPool> {
    let mut b = rayon::ThreadPoolBuilder::new().num_threads(threads);
    if counters {
        b = b.start_handler(|_| perf::open_thread());
    }
    Ok(b.build()?)
}

#[derive(Copy, Clone, PartialEq)]
enum TileOrder {
    Linear,
    Morton,
    Hilbert,
}

fn parse_order(s: &str) -> Option<TileOrder> {
    match s {
        "linear" => Some(TileOrder::Linear),
        "morton" => Some(TileOrder::Morton),
        "hilbert" => Some(TileOrder::Hilbert),
        _ => None,
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct Rect {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
}

fn morton2(x: u32, y: u32) -> u64 {
    let mut r = 0u64;
    for i in 0..16 {
        r |= (((x >> i) & 1) as u64) << (2 * i) | (((y >> i) & 1) as u64) << (2 * i + 1);
    }
    r
}

/// Distance along a Hilbert curve on an `n` x `n` grid (`n` a power of two).
fn hilbert(n: u32, mut x: u32, mut y: u32) -> u64 {
    let mut d = 0u64;
    let mut s = n / 2;
    while s > 0 {
        let rx = ((x & s) > 0) as u32;
        let ry = ((y & s) > 0) as u32;
        d += s as u64 * s as u64 * ((3 * rx) ^ ry) as u64;
        if ry == 0 {
            if rx == 1 {
                x = s - 1 - (x & (s - 1)) + (x & !(s - 1));
                y = s - 1 - (y & (s - 1)) + (y & !(s - 1));
                x &= s - 1;
                y &= s - 1;
            }
            std::mem::swap(&mut x, &mut y);
        }
        x &= s - 1;
        y &= s - 1;
        s /= 2;
    }
    d
}

fn tiles(w: u32, h: u32, tile: u32, order: TileOrder) -> Vec<Rect> {
    let (tw, th) = (w.div_ceil(tile), h.div_ceil(tile));
    let n = tw.max(th).next_power_of_two();
    let mut v: Vec<(u64, Rect)> = Vec::with_capacity((tw * th) as usize);
    for ty in 0..th {
        for tx in 0..tw {
            let key = match order {
                TileOrder::Linear => (ty * tw + tx) as u64,
                TileOrder::Morton => morton2(tx, ty),
                TileOrder::Hilbert => hilbert(n, tx, ty),
            };
            v.push((key, Rect { x0: tx * tile, y0: ty * tile, x1: ((tx + 1) * tile).min(w), y1: ((ty + 1) * tile).min(h) }));
        }
    }
    v.sort_by_key(|e| e.0);
    v.into_iter().map(|e| e.1).collect()
}

pub struct CpuSimdLoc {
    width: u32,
    height: u32,
    scene: Scene,
    pool: rayon::ThreadPool,
    threads: usize,
    counters: bool,
    packet: usize,
    isa: Isa,
    layout: u8,
    two_d: bool,
    order: TileOrder,
    tile: u32,
    tile_list: Vec<Rect>,
    pixels: Vec<u8>,
}

impl CpuSimdLoc {
    fn new(p: InitParams, preset: Preset) -> anyhow::Result<Self> {
        let o: Options = p.options()?;
        let isa = detect(&o.isa);
        let layout = layout::parse(o.layout.as_deref().unwrap_or(preset.layout)).ok_or_else(|| anyhow::anyhow!("bad layout"))?;
        let two_d = match o.schedule.as_deref().unwrap_or(preset.schedule) {
            "rows" => false,
            "tiles" => true,
            s => anyhow::bail!("bad schedule `{s}`"),
        };
        let order = parse_order(&o.tile_order).ok_or_else(|| anyhow::anyhow!("bad tile_order"))?;
        tracing::info!("{}: {:?} hot loop, packets of {}, layout {}, tiles {} ({}), prefetch {}", preset.name, isa, o.packet, layout, two_d, o.tile, o.prefetch);
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
                prefetch: o.prefetch as f32,
            },
            pool: build_pool(o.threads, o.counters)?,
            threads: o.threads,
            counters: o.counters,
            packet: o.packet.parse().unwrap_or(1),
            isa,
            layout,
            two_d,
            order,
            tile: o.tile.max(1),
            tile_list: Vec::new(),
            pixels: vec![0; (p.width * p.height * 4) as usize],
        })
    }

    fn set_layout(&mut self, layout: u8) {
        if layout == self.layout {
            return;
        }
        for e in self.scene.chunks.values_mut() {
            if let Entry::Dense { blocks, .. } = e {
                *blocks = Arc::new(layout::encode(&layout::decode(blocks, self.layout), layout));
            }
        }
        self.layout = layout;
        self.scene.grid_dirty = true;
    }
}

#[derive(Copy, Clone)]
struct OutPtr(*mut u8);
// SAFETY: every task writes a disjoint set of pixels.
unsafe impl Send for OutPtr {}
unsafe impl Sync for OutPtr {}

struct Job<'a> {
    scene: &'a Scene,
    cam: Camera,
    w: u32,
    h: u32,
    mode: RenderMode,
    packet: usize,
    layout: u8,
    two_d: bool,
    out: OutPtr,
}

/// Renders the pixels of `r`, `N` rays per packet: `N` along a row (`two_d = false`, as
/// cpu-simd) or a 1x1/2x2/4x2/4x4 block. `#[inline(always)]` so each ISA wrapper below
/// compiles its own copy with its target features.
#[inline(always)]
fn render_rect<const N: usize, const L: u8, const PF: bool>(job: &Job, r: Rect) -> u64 {
    let (scene, w, h) = (job.scene, job.w, job.h);
    let cam = &job.cam;
    let mut steps = 0u64;
    let (cam_block, o) = cam.split_position();
    let (f, rt, u) = (cam.forward(), cam.right(), cam.up());
    let tan = (cam.fov_y * 0.5).tan();
    let aspect = w as f32 / h as f32;
    let (bmin, bmax) = scene.bounds_rel(cam_block);
    let sky = encode_srgb(scene.env.sky);
    let (bw, bh) = if job.two_d {
        match N {
            1 => (1, 1),
            4 => (2, 2),
            8 => (4, 2),
            _ => (4, 4),
        }
    } else {
        (N as u32, 1)
    };
    let mut by = r.y0;
    while by < r.y1 {
        let mut bx = r.x0;
        while bx < r.x1 {
            let (mut dx, mut dy, mut dz) = ([f32::NAN; N], [0f32; N], [0f32; N]);
            let mut dirs = [Vec3::ZERO; N];
            let mut pix = [u32::MAX; N];
            for l in 0..N {
                let (x, y) = (bx + l as u32 % bw, by + l as u32 / bw);
                if x < r.x1 && y < r.y1 {
                    let sy = (1.0 - 2.0 * (y as f32 + 0.5) / h as f32) * tan;
                    let row_dir = f + sy * u;
                    let sx = (2.0 * (x as f32 + 0.5) / w as f32 - 1.0) * tan * aspect;
                    let d = (row_dir + sx * rt).normalize();
                    (dx[l], dy[l], dz[l]) = (d.x, d.y, d.z);
                    dirs[l] = d;
                    pix[l] = y * w + x;
                }
            }
            let mut hits = [None; N];
            steps += trace_packet::<N, L, PF>(&scene.grid, cam_block, o, &dx, &dy, &dz, 1e7, bmin, bmax, scene.prefetch, &mut hits) as u64 * N as u64;
            for l in 0..N {
                if pix[l] == u32::MAX {
                    continue;
                }
                let px = match &hits[l] {
                    Some(hit) => scene.shade::<L>(cam_block, o, dirs[l], hit, job.mode),
                    None => sky,
                };
                // SAFETY: pix < w*h and each pixel belongs to exactly one rect.
                unsafe { std::ptr::copy_nonoverlapping(px.as_ptr(), job.out.0.add(pix[l] as usize * 4), 4) };
            }
            bx += bw;
        }
        by += bh;
    }
    steps
}

#[inline(always)]
fn render_rect_dyn(job: &Job, r: Rect) -> u64 {
    macro_rules! by_layout {
        ($n:literal) => {
            match (job.layout, job.scene.prefetch > 0.0) {
                (BRICK, false) => render_rect::<$n, BRICK, false>(job, r),
                (MORTON, false) => render_rect::<$n, MORTON, false>(job, r),
                (_, false) => render_rect::<$n, LINEAR, false>(job, r),
                (BRICK, true) => render_rect::<$n, BRICK, true>(job, r),
                (MORTON, true) => render_rect::<$n, MORTON, true>(job, r),
                (_, true) => render_rect::<$n, LINEAR, true>(job, r),
            }
        };
    }
    match job.packet {
        1 => by_layout!(1),
        4 => by_layout!(4),
        16 => by_layout!(16),
        _ => by_layout!(8),
    }
}

#[target_feature(enable = "avx2,fma")]
fn render_rect_avx2(job: &Job, r: Rect) -> u64 {
    render_rect_dyn(job, r)
}

#[target_feature(enable = "avx512f,avx512vl,avx512bw,avx2,fma")]
fn render_rect_avx512(job: &Job, r: Rect) -> u64 {
    render_rect_dyn(job, r)
}

fn render_rect_isa(isa: Isa, job: &Job, r: Rect) -> u64 {
    match isa {
        // SAFETY: only selected when the CPU reports these features.
        Isa::Avx512 => unsafe { render_rect_avx512(job, r) },
        Isa::Avx2 => unsafe { render_rect_avx2(job, r) },
        Isa::Scalar => render_rect_dyn(job, r),
    }
}

impl Renderer for CpuSimdLoc {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        let layout = self.layout;
        let s = &mut self.scene;
        let dense = |d: &DenseChunk| Entry::Dense { blocks: Arc::new(layout::encode(d, layout)), bricks: bricks_of(d) };
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
                            s.chunks.insert(key.pos, Entry::Dense { blocks: Arc::new(layout::encode_owned(d, layout)), bricks });
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
                    // Apply all edits first, then re-encode each touched chunk once.
                    let mut touched: HashMap<IVec3, ChunkData> = HashMap::new();
                    for e in edits {
                        let pos = ChunkKey::of_block(e.pos).pos;
                        let data = match touched.get_mut(&pos) {
                            Some(d) => d,
                            None => {
                                let Some(entry) = s.chunks.get(&pos) else { continue };
                                let d = match entry {
                                    Entry::Uniform(b) => ChunkData::Uniform(BlockId(*b)),
                                    Entry::Dense { blocks, .. } => ChunkData::Dense(Arc::new(layout::decode(blocks, layout))),
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
                            ChunkData::Dense(d) => dense(&d),
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
        if self.two_d && self.tile_list.is_empty() {
            self.tile_list = tiles(w, h, self.tile, self.order);
        }
        let mut pixels = std::mem::take(&mut self.pixels);
        let job = Job { scene: &self.scene, cam: req.camera, w, h, mode: req.mode, packet: self.packet, layout: self.layout, two_d: self.two_d, out: OutPtr(pixels.as_mut_ptr()) };
        let (isa, tile_list, counters) = (self.isa, &self.tile_list, self.counters);
        if counters {
            self.pool.broadcast(|_| perf::take_thread());
        }
        let task = |r: Rect| {
            if counters {
                perf::task_begin();
            }
            let n = render_rect_isa(isa, &job, r);
            if counters {
                perf::task_end();
            }
            n
        };
        let lane_steps: u64 = self.pool.install(|| {
            if job.two_d {
                tile_list.par_iter().map(|&r| task(r)).sum()
            } else {
                (0..h).into_par_iter().map(|y| task(Rect { x0: 0, y0: y, x1: w, y1: y + 1 })).sum()
            }
        });
        let mut extra = Vec::new();
        if counters {
            let per_thread = self.pool.broadcast(|_| perf::take_thread());
            let px = (w * h) as f64;
            let mut tot = vec![0f64; perf::names().len()];
            for v in per_thread {
                for (t, x) in tot.iter_mut().zip(v) {
                    *t += x;
                }
            }
            extra.push(("pmc_ipc".to_string(), tot[1] / tot[0]));
            for (name, v) in perf::names().iter().zip(&tot) {
                extra.push((format!("pmc_{name}_per_px"), v / px));
            }
        }
        drop(job);
        self.pixels = pixels;
        let mut counters_out = vec![
            ("chunks".into(), self.scene.chunks.len() as f64),
            // Lockstep iterations x packet width: work including idle lanes.
            ("lane_steps_per_pixel".into(), lane_steps as f64 / (w * h) as f64),
        ];
        counters_out.extend(extra);
        let stats = FrameStats { cpu_ms: start.elapsed().as_secs_f32() * 1e3, gpu_ms: None, counters: counters_out };
        sink.complete(CompletedFrame { frame_id: req.frame_id, stats, pixels: Some(&self.pixels) });
        Ok(())
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        let bad = || anyhow::anyhow!("bad value for `{name}`");
        match name {
            "threads" => {
                let n = value.as_i64().unwrap_or(0) as usize;
                if n != self.threads {
                    self.pool = build_pool(n, self.counters)?;
                    self.threads = n;
                }
            }
            "shadows" => self.scene.shadows = value.as_str().and_then(Shadows::parse).ok_or_else(bad)?,
            "packet" => self.packet = value.as_str().and_then(|s| s.parse().ok()).unwrap_or(8),
            "isa" => self.isa = detect(value.as_str().unwrap_or("auto")),
            "layout" => self.set_layout(value.as_str().and_then(layout::parse).ok_or_else(bad)?),
            "schedule" => self.two_d = value.as_str() == Some("tiles"),
            "tile_order" => {
                self.order = value.as_str().and_then(parse_order).ok_or_else(bad)?;
                self.tile_list.clear();
            }
            "tile" => {
                self.tile = value.as_i64().ok_or_else(bad)?.max(1) as u32;
                self.tile_list.clear();
            }
            "prefetch" => self.scene.prefetch = value.as_i64().ok_or_else(bad)? as f32,
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
        self.tile_list.clear();
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        let dense = self.scene.chunks.values().filter(|e| matches!(e, Entry::Dense { .. })).count() as u64;
        MemoryReport { cpu_bytes: dense * (CHUNK_VOLUME as u64 * 2 + 64) + self.scene.grid.cells.len() as u64 * 4 + self.pixels.len() as u64, gpu_bytes: 0 }
    }
}
