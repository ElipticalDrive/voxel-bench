//! CPU voxel world for discovery rays: the dense chunk grid and multi-level DDA copied
//! from `cpu-simd` (unchanged traversal), plus the chunk bookkeeping that keeps it in sync
//! with the `WorldUpdate` stream.

use std::collections::HashMap;
use std::sync::Arc;

use glam::{IVec3, Vec3};
use mr_core::chunk::{cell_index, cell_of_index, local_of_block};
use mr_core::*;

// ------------------------------------------------------------------------- world

#[derive(Clone)]
pub(crate) enum Entry {
    Uniform(u16),
    Dense { blocks: Arc<DenseChunk>, bricks: [u64; 8] },
}

/// Dense grid of chunk entries over the loaded region; index 0 = air.
#[derive(Default)]
pub(crate) struct Grid {
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

pub(crate) fn bricks_of(d: &DenseChunk) -> [u64; 8] {
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
pub(crate) struct Hit {
    pub t: f32,
    pub cell: IVec3,
    #[allow(dead_code)] // kept from cpu-simd; discovery re-reads blocks when validating S
    pub block: u16,
    pub axis: u8,
}

// ------------------------------------------------------------------------- tracing

/// `floor` without a libm call (the baseline x86-64 target has no SSE4.1 `roundss`, so
/// `f32::floor` is a function call — it dominated the step loop).
#[inline(always)]
pub(crate) fn ffloor(x: f32) -> i32 {
    let i = x as i32;
    i - (x < i as f32) as i32
}

/// Traces `N` rays in lockstep from the shared origin `o` (relative to `cam`).
/// Lanes with `d.x == NaN` are inactive. Each lane ignores the start cell (like the
/// reference) and stops at `tfar`.
#[inline(always)]
pub(crate) fn trace_packet<const N: usize>(grid: &Grid, cam: IVec3, o: Vec3, dx: &[f32; N], dy: &[f32; N], dz: &[f32; N], max_t: f32, bmin: IVec3, bmax: IVec3, out: &mut [Option<Hit>; N]) -> u32 {
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

impl Grid {
    /// Block id at absolute cell `abs` (0 = air / not loaded).
    #[inline]
    pub(crate) fn block(&self, abs: IVec3) -> u16 {
        let idx = self.chunk(abs);
        if idx == 0 {
            return 0;
        }
        match &self.entries[idx as usize - 1] {
            Entry::Uniform(b) => *b,
            Entry::Dense { blocks, .. } => blocks.0[cell_index(abs & 31)].0,
        }
    }

    /// Loaded region relative to camera block `cam`.
    pub(crate) fn bounds_rel(&self, cam: IVec3) -> (IVec3, IVec3) {
        if self.cells.is_empty() {
            return (IVec3::ONE, IVec3::ZERO);
        }
        (self.min * 32 - cam, (self.min + self.dims) * 32 - cam)
    }

    /// Single-ray trace (packet of one) from camera-relative `o`.
    #[inline]
    pub(crate) fn trace(&self, cam: IVec3, o: Vec3, d: Vec3, bmin: IVec3, bmax: IVec3) -> Option<Hit> {
        let mut out = [None];
        trace_packet::<1>(self, cam, o, &[d.x], &[d.y], &[d.z], 1e7, bmin, bmax, &mut out);
        out[0]
    }
}

/// Chunk map mirrored from the update stream; produces immutable grid snapshots that
/// background discovery jobs can hold while the render thread applies new updates.
#[derive(Default)]
pub(crate) struct CpuWorld {
    chunks: HashMap<IVec3, Entry>,
    pub grid: Arc<Grid>,
    dirty: bool,
}

impl CpuWorld {
    pub(crate) fn apply(&mut self, updates: &[WorldUpdate]) {
        for u in updates {
            match u {
                WorldUpdate::ChunkLoad { key, data } if key.lod == 0 => {
                    match data {
                        ChunkData::Uniform(b) if b.is_air() => {
                            self.chunks.remove(&key.pos);
                        }
                        ChunkData::Uniform(b) => {
                            self.chunks.insert(key.pos, Entry::Uniform(b.0));
                        }
                        ChunkData::Dense(d) => {
                            let bricks = bricks_of(d);
                            self.chunks.insert(key.pos, Entry::Dense { blocks: d.clone(), bricks });
                        }
                    }
                    self.dirty = true;
                }
                WorldUpdate::ChunkUnload { key } if key.lod == 0 => {
                    self.chunks.remove(&key.pos);
                    self.dirty = true;
                }
                WorldUpdate::SetBlocks(edits) => {
                    let mut touched: HashMap<IVec3, ChunkData> = HashMap::new();
                    for e in edits {
                        let pos = ChunkKey::of_block(e.pos).pos;
                        let data = match touched.get_mut(&pos) {
                            Some(d) => d,
                            None => {
                                let Some(entry) = self.chunks.get(&pos) else { continue };
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
                                self.chunks.remove(&pos);
                                continue;
                            }
                            ChunkData::Uniform(b) => Entry::Uniform(b.0),
                            ChunkData::Dense(d) => {
                                let bricks = bricks_of(&d);
                                Entry::Dense { blocks: d, bricks }
                            }
                        };
                        self.chunks.insert(pos, entry);
                    }
                    self.dirty = true;
                }
                _ => {}
            }
        }
    }

    /// Current grid snapshot (rebuilt if the world changed since the last call).
    pub(crate) fn snapshot(&mut self) -> Arc<Grid> {
        if self.dirty {
            self.dirty = false;
            let mut it = self.chunks.keys();
            self.grid = Arc::new(match it.next() {
                None => Grid::default(),
                Some(&first) => {
                    let (lo, hi) = it.fold((first, first), |(lo, hi), &k| (lo.min(k), hi.max(k)));
                    let dims = hi - lo + 1;
                    let mut cells = vec![0u32; (dims.x * dims.y * dims.z) as usize];
                    let mut entries = Vec::with_capacity(self.chunks.len());
                    for (pos, e) in &self.chunks {
                        entries.push(e.clone());
                        let c = *pos - lo;
                        cells[((c.z * dims.y + c.y) * dims.x + c.x) as usize] = entries.len() as u32;
                    }
                    Grid { min: lo, dims, cells, entries }
                }
            });
        }
        self.grid.clone()
    }

    pub(crate) fn memory(&self) -> u64 {
        let dense = self.chunks.values().filter(|e| matches!(e, Entry::Dense { .. })).count() as u64;
        dense * (CHUNK_VOLUME as u64 * 2 + 64) + self.grid.cells.len() as u64 * 4
    }
}
