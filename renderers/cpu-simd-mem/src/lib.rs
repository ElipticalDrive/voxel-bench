//! Memory-footprint variants of `cpu-simd`: the same f32 multi-level DDA and SIMD ray
//! packets, generic over the per-chunk voxel representation (see `repr`):
//! * `cpu-simd-u16`     — cpu-simd's u16-per-voxel layout (control, same code path);
//! * `cpu-simd-u8`      — u8 ids (≤ 256 block types), half the footprint;
//! * `cpu-simd-bits`    — 1 bit/voxel brick-major occupancy for traversal (4 KiB/chunk),
//!                        ids read only at the final hit;
//! * `cpu-simd-hier`    — bits + a 64-bit 8^3-region summary (skips 8 cells);
//! * `cpu-simd-prefix`  — the user's Morton masks + popcount prefix (skips 2..16 cells);
//! * `cpu-simd-palette` — per-chunk palette, indices packed at 1/2/4/8/16 bits.
//! Optional hardware counters (`perf` knob) via `perf_event_open` report cycles,
//! instructions and cache misses per pixel.

mod perf;
pub mod repr;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use cpu_reference::Shadows;
use repr::{Bits, Hier, Palette, Prefix, Repr, U16, U8};
use glam::{IVec3, Vec3};
use mr_core::chunk::local_of_block;
use mr_core::lighting::{encode_srgb, point_light, srgb_to_linear, texel_coord};
use mr_core::*;
use rayon::prelude::*;
use serde::Deserialize;

macro_rules! entry_fn {
    ($f:ident, $r:ty, $name:literal, $desc:literal) => {
        pub fn $f() -> RendererEntry {
            RendererEntry {
                info: || RendererInfo {
                    name: $name.into(),
                    description: $desc.into(),
                    tags: vec!["cpu".into(), "raycast".into(), "simd".into(), "memory".into()],
                    modes: vec![RenderMode::Unlit, RenderMode::Lit],
                    max_lod: 0,
                    params: params(),
                },
                create: |p, _| Ok(Box::new(CpuSimdMem::<$r>::new(p, $name)?)),
            }
        }
    };
}

entry_fn!(entry_u16, U16, "cpu-simd-u16", "cpu-simd footprint control: u16 ids per voxel + brick mask (64 KiB/chunk), generic code path");
entry_fn!(entry_u8, U8, "cpu-simd-u8", "cpu-simd with u8 block ids (32 KiB/chunk; needs <= 256 block types)");
entry_fn!(entry_bits, Bits, "cpu-simd-bits", "cpu-simd traversing 1-bit brick-major occupancy (4 KiB/chunk); ids read only at the hit");
entry_fn!(entry_hier, Hier, "cpu-simd-hier", "cpu-simd-bits + 64-bit 8^3-region summary per chunk (8-cell skips)");
entry_fn!(entry_prefix, Prefix, "cpu-simd-prefix", "Morton-ordered 4^3 occupancy masks + popcount prefix array: any aligned 2..16 cube's emptiness is one compare; the prefix doubles as a rank into compressed (solid-only) block ids");
entry_fn!(entry_palette, Palette, "cpu-simd-palette", "cpu-simd traversing per-chunk palette indices packed at 1/2/4/8/16 bits");

fn params() -> Vec<ParamSpec> {
    let mut p = cpu_reference::cpu_params();
    p.push(ParamSpec::choice("packet", "Rays traced together in lockstep (measured: no gain on Zen 5 — traversal is memory-latency bound)", &["1", "4", "8", "16"], "1", true));
    p.push(ParamSpec::choice("isa", "Instruction set for the hot loop", &["auto", "scalar", "avx2", "avx512"], "auto", true));
    p.push(ParamSpec::choice("traversal", "refloor: the probe returns a skip size and the DDA re-floors after every coarse step. incremental: hierarchical DDA that keeps per-level exit times and moves up and down levels with adds only. incremental-exact: the same, but exit times are recomputed from the integer cell (a multiply instead of an add; no float drift). Packets are ignored by both incremental modes", &["refloor", "incremental", "incremental-exact"], "refloor", true));
    for (k, size) in [(1, 2), (2, 4), (3, 8), (4, 16)] {
        p.push(ParamSpec::bool(&format!("skip{size}"), &format!("Incremental traversals: use {size}^3 cubes (level {k}) if the representation has them. Fewer levels mean fewer failed ascent tests but smaller skips"), true, true));
    }
    p.push(ParamSpec::bool("perf", "Hardware counters per frame (perf_event_open; cycles, instructions, cache misses per pixel)", false, true));
    p
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    threads: usize,
    shadows: Shadows,
    packet: String,
    isa: String,
    traversal: String,
    skip2: bool,
    skip4: bool,
    skip8: bool,
    skip16: bool,
    perf: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { threads: 0, shadows: Shadows::All, packet: "1".into(), isa: "auto".into(), traversal: "refloor".into(), skip2: true, skip4: true, skip8: true, skip16: true, perf: false }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum Traversal {
    /// `trace_packet`: the probe returns a skip size, and coarse steps re-floor.
    Refloor,
    /// `trace_incremental`, exit times accumulated by adds.
    Incremental,
    /// `trace_incremental`, exit times recomputed from the integer cell (one multiply).
    IncrementalExact,
}

fn parse_traversal(s: &str) -> anyhow::Result<Traversal> {
    match s {
        "refloor" => Ok(Traversal::Refloor),
        "incremental" => Ok(Traversal::Incremental),
        "incremental-exact" => Ok(Traversal::IncrementalExact),
        _ => anyhow::bail!("traversal must be `refloor`, `incremental` or `incremental-exact`, got `{s}`"),
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
enum Entry<R> {
    Uniform(u16),
    Dense(R),
}

/// Dense grid of chunk entries over the loaded region; index 0 = air.
struct Grid<R> {
    min: IVec3,
    dims: IVec3,
    cells: Vec<u32>,
    entries: Vec<Entry<R>>,
}

impl<R> Default for Grid<R> {
    fn default() -> Self {
        Self { min: IVec3::ZERO, dims: IVec3::ZERO, cells: Vec::new(), entries: Vec::new() }
    }
}

impl<R: Repr> Grid<R> {
    /// Entry index (0 = air or outside) of the chunk containing absolute cell `abs`.
    #[inline(always)]
    fn chunk(&self, abs: IVec3) -> u32 {
        let c: IVec3 = (abs >> 5) - self.min;
        if c.x < 0 || c.y < 0 || c.z < 0 || c.x >= self.dims.x || c.y >= self.dims.y || c.z >= self.dims.z {
            return 0;
        }
        self.cells[((c.z * self.dims.y + c.y) * self.dims.x + c.x) as usize]
    }

    /// Entry index (0 = air or outside) of the chunk at chunk coordinates `ch`.
    #[inline(always)]
    fn chunk_at(&self, ch: IVec3) -> u32 {
        self.chunk(ch << 5)
    }

    /// (non-zero if solid, step size) at absolute cell `abs` inside chunk entry `idx`.
    #[inline(always)]
    fn probe_in(&self, idx: u32, abs: IVec3) -> (u32, i32) {
        if idx == 0 {
            return (0, 32);
        }
        match &self.entries[idx as usize - 1] {
            Entry::Uniform(b) => (*b as u32, 1),
            Entry::Dense(r) => r.probe(abs & 31),
        }
    }

    /// Block id at absolute cell `abs` (called once per hit).
    fn block_at(&self, abs: IVec3) -> u16 {
        match self.chunk(abs) {
            0 => 0,
            idx => match &self.entries[idx as usize - 1] {
                Entry::Uniform(b) => *b,
                Entry::Dense(r) => r.block(abs & 31),
            },
        }
    }
}

#[derive(Copy, Clone, Default)]
struct Hit {
    t: f32,
    cell: IVec3,
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
fn trace_packet<R: Repr, const N: usize>(grid: &Grid<R>, cam: IVec3, o: Vec3, dx: &[f32; N], dy: &[f32; N], dz: &[f32; N], max_t: f32, bmin: IVec3, bmax: IVec3, out: &mut [Option<Hit>; N]) -> u32 {
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
                out[l] = Some(Hit { t: t[l], cell: c, axis: axis[l] });
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

/// Work done by `trace_incremental`: DDA steps, and occupancy tests made while
/// climbing (ascent attempts) and after descending.
#[derive(Copy, Clone, Default)]
struct Tally {
    steps: u32,
    up: u32,
    down: u32,
}

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

/// Next level above `level` in the level mask `mask` (5 = chunk is always present).
#[inline(always)]
fn level_above(mask: u32, level: u32) -> u32 {
    (mask >> (level + 1)).trailing_zeros() + level + 1
}

/// Next level below `level` in `mask` (0 is always present).
#[inline(always)]
fn level_below(mask: u32, level: u32) -> u32 {
    31 - (mask & ((1 << level) - 1)).leading_zeros()
}

/// Incremental-level hierarchical DDA (IDEAS §1, the user's intended traversal). The ray
/// keeps a cell `c` at the current level (absolute coordinates `>> level`) with per-axis
/// `tmax` (exit times) and `tdelta` (cell size in t). Every move is incremental, with no
/// re-flooring:
/// * **step** at the current level: classic DDA, one add;
/// * **ascend** one level: `tmax` gains `tdelta` on the axes where the cell was the first
///   child in the ray's direction, and `tdelta` doubles;
/// * **descend** one level: `tdelta` halves, the midplane exit is `tmax - tdelta`, and the
///   child is the second one along an axis exactly when the entry time has passed it.
///
/// Ascents are tried only after a step crosses the next level's boundary (otherwise the
/// parent is the one already known to be occupied). Levels a representation can't test
/// (`!R::LEVELS`) are skipped over in one go. Returns the hit and the occupancy tests made.
#[inline(always)]
fn trace_incremental<R: Repr, const EXACT: bool>(grid: &Grid<R>, levels: u32, cam: IVec3, o: Vec3, d: Vec3, max_t: f32, bmin: IVec3, bmax: IVec3) -> (Option<Hit>, Tally) {
    const TOP: u32 = 5;
    let mask = R::LEVELS & levels | 1 | 1 << TOP;
    let (bminf, bmaxf) = (bmin.as_vec3(), bmax.as_vec3());
    let mut inv = [0f32; 3];
    let mut stp = [0i32; 3];
    let mut tlo = [0f32; 3];
    let mut thi = [0f32; 3];
    for a in 0..3 {
        let da = if d[a].abs() < 1e-20 { 1e-20 } else { d[a] };
        inv[a] = 1.0 / da;
        stp[a] = if da > 0.0 { 1 } else { -1 };
        let t0 = (bminf[a] - o[a]) * inv[a];
        let t1 = (bmaxf[a] - o[a]) * inv[a];
        tlo[a] = t0.min(t1);
        thi[a] = t0.max(t1);
    }
    let mut t = tlo[0].max(tlo[1]).max(tlo[2]).max(0.0);
    let tfar = thi[0].min(thi[1]).min(thi[2]).min(max_t);
    if t > tfar {
        return (None, Tally::default());
    }
    let mut axis = 3u8;
    if t > 0.0 {
        axis = if tlo[0] >= tlo[1] && tlo[0] >= tlo[2] { 0 } else if tlo[1] >= tlo[2] { 1 } else { 2 };
    }
    let start = o.floor().as_ivec3() + cam;
    let mut c = [0i32; 3];
    let mut tmax = [0f32; 3];
    let mut tdelta = [0f32; 3];
    for a in 0..3 {
        let rel = ffloor(o[a] + d[a] * t).clamp(bmin[a], bmax[a] - 1);
        c[a] = rel + cam[a];
        tmax[a] = ((rel + (stp[a] > 0) as i32) as f32 - o[a]) * inv[a];
        tdelta[a] = inv[a].abs();
    }
    let mut level = 0u32;
    let mut may_ascend = true;
    let mut tally = Tally::default();
    // Exit time along axis `b` of the level-`level` cell `cb`, from integers (no drift).
    let exit = |b: usize, cb: i32, level: u32| ((((cb + (stp[b] > 0) as i32) << level) - cam[b]) as f32 - o[b]) * inv[b];
    let mut cached_chunk = IVec3::splat(i32::MIN);
    let mut cached_idx = 0u32;
    let mut occupied = |level: u32, c: [i32; 3]| -> bool {
        let shift = (TOP - level) as i32;
        let cv = IVec3::from_array(c);
        let ch = cv >> shift;
        if ch != cached_chunk {
            cached_chunk = ch;
            cached_idx = grid.chunk_at(ch);
        }
        match cached_idx {
            0 => false,
            idx => match &grid.entries[idx as usize - 1] {
                Entry::Uniform(_) => true,
                Entry::Dense(_) if level == TOP => true,
                Entry::Dense(r) => r.occupied(level, cv & ((1 << shift) - 1)),
            },
        }
    };
    for _ in 0..4096 {
        if occupied(level, c) {
            if level > 0 {
                // Descend to the next level the representation can test.
                let below = level_below(mask, level);
                while level > below {
                    for b in 0..3 {
                        tdelta[b] *= 0.5;
                        // Exit of the first child in the ray's direction (the midplane).
                        let tmid = if EXACT { exit(b, c[b] * 2 + (stp[b] < 0) as i32, level - 1) } else { tmax[b] - tdelta[b] };
                        let second = t >= tmid;
                        if !second {
                            tmax[b] = tmid;
                        }
                        c[b] = c[b] * 2 + (second ^ (stp[b] < 0)) as i32;
                    }
                    level -= 1;
                }
                tally.down += 1;
                may_ascend = false;
                continue;
            }
            if c != start.to_array() {
                return (Some(Hit { t, cell: IVec3::from_array(c) - cam, axis }), tally);
            }
        } else {
            // Climb while the parent is empty, then step at the highest empty level.
            while may_ascend && level < TOP {
                let above = level_above(mask, level);
                let k = (above - level) as i32;
                tally.up += 1;
                if occupied(above, [c[0] >> k, c[1] >> k, c[2] >> k]) {
                    break;
                }
                while level < above {
                    for b in 0..3 {
                        let first = (c[b] & 1 == 0) == (stp[b] > 0);
                        if first {
                            tmax[b] += tdelta[b];
                        }
                        tdelta[b] *= 2.0;
                        c[b] >>= 1;
                    }
                    level += 1;
                }
                if EXACT {
                    for b in 0..3 {
                        tmax[b] = exit(b, c[b], level);
                    }
                }
            }
        }
        // Step at the current level.
        let a = if tmax[0] <= tmax[1] && tmax[0] <= tmax[2] { 0 } else if tmax[1] <= tmax[2] { 1 } else { 2 };
        t = tmax[a];
        if t > tfar {
            return (None, tally);
        }
        tally.steps += 1;
        let old = c[a];
        c[a] += stp[a];
        tmax[a] = if EXACT { exit(a, c[a], level) } else { tmax[a] + tdelta[a] };
        axis = a as u8;
        let k = (level_above(mask, level).min(TOP + 1) - level) as i32;
        may_ascend = level < TOP && (old >> k) != (c[a] >> k);
    }
    (None, tally)
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
        let p = o + d * h.t;
        let local = p - h.cell.as_vec3();
        let uv = face.uv(local);
        let texel = self.blocks.texture_of(BlockId(block), face).texel(texel_coord(uv.x), texel_coord(uv.y));
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

struct Pool {
    pool: rayon::ThreadPool,
    threads: usize,
}

impl Pool {
    fn new(threads: usize) -> anyhow::Result<Self> {
        Ok(Self { pool: rayon::ThreadPoolBuilder::new().num_threads(threads).build()?, threads })
    }
}

pub struct CpuSimdMem<R> {
    width: u32,
    height: u32,
    scene: Scene<R>,
    pool: Pool,
    perf: bool,
    counters: Option<perf::Counters>,
    packet: usize,
    isa: Isa,
    isa_choice: String,
    pixels: Vec<u8>,
}

impl<R: Repr> CpuSimdMem<R> {
    pub fn new(p: InitParams, name: &'static str) -> anyhow::Result<Self> {
        let o: Options = p.options()?;
        if name == "cpu-simd-u8" && p.blocks.blocks.len() > 256 {
            anyhow::bail!("cpu-simd-u8 needs <= 256 block types, registry has {}", p.blocks.blocks.len());
        }
        let isa = detect(&o.isa);
        tracing::info!("{name}: {:?} hot loop, packets of {}", isa, o.packet);
        Ok(Self {
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
                traversal: parse_traversal(&o.traversal)?,
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

impl<R: Repr> CpuSimdMem<R> {
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

impl<R: Repr> Renderer for CpuSimdMem<R> {
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
            "traversal" => self.scene.traversal = parse_traversal(value.as_str().unwrap_or(""))?,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rng(s: &mut u64) -> u64 {
        *s ^= *s << 13;
        *s ^= *s >> 7;
        *s ^= *s << 17;
        *s
    }

    fn unit(s: &mut u64) -> f32 {
        (rng(s) >> 40) as f32 / (1u64 << 24) as f32
    }

    /// A 3×2×3-chunk world at negative and positive coordinates: rolling terrain with
    /// floating debris, one missing (air) chunk and one uniform stone chunk.
    fn world<R: Repr>(seed: u64) -> Grid<R> {
        let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut scene_chunks: HashMap<IVec3, Entry<R>> = HashMap::new();
        for cz in -1..=1 {
            for cy in -1..=0 {
                for cx in -1..=1 {
                    let pos = IVec3::new(cx, cy, cz);
                    if pos == IVec3::new(1, 0, -1) {
                        continue;
                    }
                    if pos == IVec3::new(-1, -1, 1) {
                        scene_chunks.insert(pos, Entry::Uniform(3));
                        continue;
                    }
                    let mut d = DenseChunk(Box::new([BlockId::AIR; CHUNK_VOLUME]));
                    for i in 0..CHUNK_VOLUME {
                        let w = pos * 32 + cell_of(i);
                        let h = (8.0 * ((w.x as f32 * 0.21).sin() + (w.z as f32 * 0.17).cos())) as i32 - 12;
                        if w.y < h || rng(&mut s) % 400 == 0 {
                            d.0[i] = BlockId(1 + (rng(&mut s) % 4) as u16);
                        }
                    }
                    scene_chunks.insert(pos, Entry::Dense(R::build(&Arc::new(d))));
                }
            }
        }
        let lo = IVec3::new(-1, -1, -1);
        let dims = IVec3::new(3, 2, 3);
        let mut cells = vec![0u32; 18];
        let mut entries = Vec::new();
        for (pos, e) in scene_chunks {
            entries.push(e);
            let c = pos - lo;
            cells[((c.z * dims.y + c.y) * dims.x + c.x) as usize] = entries.len() as u32;
        }
        Grid { min: lo, dims, cells, entries }
    }

    fn cell_of(i: usize) -> IVec3 {
        mr_core::chunk::cell_of_index(i)
    }

    fn solid<R: Repr>(g: &Grid<R>, abs: IVec3) -> bool {
        g.block_at(abs) != 0
    }

    /// The incremental traversal finds the same first hit as the re-flooring one, for every
    /// representation. Disagreements are allowed only where the ray grazes an edge (both
    /// hits at nearly the same t), and every reported hit must be a solid cell.
    fn agree<R: Repr, const EXACT: bool, const LEVELS: u32>(seed: u64) {
        let g = world::<R>(seed);
        let (bmin_abs, bmax_abs) = (g.min * 32, (g.min + g.dims) * 32);
        let mut s = seed | 1;
        let (mut n, mut differ) = (0, 0);
        for _ in 0..4000 {
            // Camera block somewhere in the world, origin fraction within it.
            let cam = IVec3::new(
                bmin_abs.x + (rng(&mut s) % 96) as i32,
                bmin_abs.y + 20 + (rng(&mut s) % 40) as i32,
                bmin_abs.z + (rng(&mut s) % 96) as i32,
            );
            let o = Vec3::new(unit(&mut s), unit(&mut s), unit(&mut s));
            let mut d = Vec3::new(unit(&mut s) - 0.5, unit(&mut s) - 0.7, unit(&mut s) - 0.5);
            // Some axis-aligned and diagonal rays.
            match rng(&mut s) % 8 {
                0 => d.x = 0.0,
                1 => (d.x, d.z) = (0.0, 0.0),
                2 => d = Vec3::new(1.0, -1.0, 1.0),
                _ => {}
            }
            let d = d.normalize();
            let (bmin, bmax) = (bmin_abs - cam, bmax_abs - cam);
            let mut out = [None];
            trace_packet::<R, 1>(&g, cam, o, &[d.x], &[d.y], &[d.z], 1e7, bmin, bmax, &mut out);
            let (inc, _) = trace_incremental::<R, EXACT>(&g, LEVELS, cam, o, d, 1e7, bmin, bmax);
            if let Some(h) = inc {
                assert!(solid(&g, h.cell + cam), "incremental hit an air cell");
                assert!(h.cell != o.floor().as_ivec3(), "hit the start cell");
            }
            n += 1;
            match (out[0], inc) {
                (Some(a), Some(b)) if a.cell == b.cell => {}
                (None, None) => {}
                (Some(a), Some(b)) => {
                    assert!((a.t - b.t).abs() < 1e-3 * a.t.max(1.0), "hits differ: {:?}/{} vs {:?}/{}", a.cell, a.t, b.cell, b.t);
                    differ += 1;
                }
                (a, b) => panic!("hit vs miss: {:?} vs {:?} (cam {cam}, o {o}, d {d})", a.map(|h| (h.cell, h.t)), b.map(|h| (h.cell, h.t))),
            }
        }
        assert!(differ * 1000 <= n, "{differ}/{n} rays differ");
    }

    #[test]
    fn incremental_matches_refloor() {
        for seed in [1, 2, 3] {
            agree::<U16, false, 0b111111>(seed);
            agree::<Bits, false, 0b111111>(seed);
            agree::<Hier, false, 0b111111>(seed);
            agree::<Prefix, false, 0b111111>(seed);
            agree::<Palette, false, 0b111111>(seed);
            agree::<Hier, true, 0b111111>(seed);
            agree::<Prefix, true, 0b111111>(seed);
            // Level subsets (the `skip*` knobs).
            agree::<Prefix, true, 0b101101>(seed);
            agree::<Prefix, true, 0b100001>(seed);
            agree::<Prefix, false, 0b110011>(seed);
        }
    }

    #[test]
    fn level_masks() {
        assert_eq!(level_above(0b100101, 0), 2);
        assert_eq!(level_above(0b100101, 2), 5);
        assert_eq!(level_below(0b100101, 5), 2);
        assert_eq!(level_below(0b111111, 1), 0);
    }
}
