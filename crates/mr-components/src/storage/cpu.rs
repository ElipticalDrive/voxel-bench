//! CPU storage interface: per-chunk voxel representations, each a storage
//! component (`storage/<name>/cpu.rs`). All answer the same two questions:
//! `probe(local)` → (non-zero if solid, step size for the DDA) and `block(local)` → id.
//! Only `probe` is used inside the traversal loop; `block` runs once per primary hit.

use std::sync::Arc;

use glam::IVec3;
use mr_core::chunk::cell_of_index;
use mr_core::*;

pub trait Repr: Clone + Send + Sync + 'static {
    fn build(d: &Arc<DenseChunk>) -> Self;
    /// (non-zero if solid, step size) at chunk-local cell `l` (0..32 per axis).
    fn probe(&self, l: IVec3) -> (u32, i32);
    /// Levels `k` (bit k set) at which `occupied` answers directly for aligned 2^k cubes.
    /// Always includes 0; the chunk level (5) is answered by the chunk grid instead.
    const LEVELS: u32;
    /// The capabilities this CPU storage provides (the registry reads them):
    /// every `Repr` answers the voxel queries; none has LOD, distances or rows yet.
    const CAPS: &'static [crate::Cap] = &[crate::Cap::Voxels];
    /// Whether the aligned 2^`level` cube at level coordinates `c` (chunk-local,
    /// `0..32 >> level` per axis) holds a solid voxel. Exact; only called for `LEVELS`.
    fn occupied(&self, level: u32, c: IVec3) -> bool;
    /// Block id at chunk-local cell `l`.
    fn block(&self, l: IVec3) -> u16;
    /// Bytes touched by traversal (occupancy structures).
    fn traversal_bytes(&self) -> usize;
    /// Bytes of everything this representation keeps alive (traversal + ids).
    fn total_bytes(&self) -> usize;
    /// Decodes the whole chunk (used to apply edits).
    fn to_dense(&self) -> DenseChunk {
        let mut d = DenseChunk(Box::new([BlockId::AIR; CHUNK_VOLUME]));
        for (i, b) in d.0.iter_mut().enumerate() {
            *b = BlockId(self.block(cell_of_index(i)));
        }
        d
    }
}

/// Brick index (0..512) of a chunk-local cell: same order as cpu-simd's brick mask.
#[inline(always)]
pub fn brick_index(l: IVec3) -> u32 {
    let b: IVec3 = l >> 2;
    (b.x | (b.z << 3) | (b.y << 6)) as u32
}

/// Bit within a brick's u64 (brick-major layout).
#[inline(always)]
pub fn bit_in_brick(l: IVec3) -> u32 {
    ((l.x & 3) | ((l.z & 3) << 2) | ((l.y & 3) << 4)) as u32
}

pub fn bricks_of(d: &DenseChunk) -> [u64; 8] {
    let mut bits = [0u64; 8];
    for (i, b) in d.0.iter().enumerate() {
        if b.is_solid() {
            let bi = brick_index(cell_of_index(i)) as usize;
            bits[bi >> 6] |= 1 << (bi & 63);
        }
    }
    bits
}

pub fn occupancy_of(d: &DenseChunk) -> Box<[u64; 512]> {
    let mut occ = Box::new([0u64; 512]);
    for (i, b) in d.0.iter().enumerate() {
        if b.is_solid() {
            let l = cell_of_index(i);
            occ[brick_index(l) as usize] |= 1 << bit_in_brick(l);
        }
    }
    occ
}

#[inline(always)]
pub fn brick_set(bricks: &[u64; 8], bi: u32) -> bool {
    bricks[(bi >> 6) as usize] >> (bi & 63) & 1 != 0
}


// ------------------------------------------------------------------------- world

#[derive(Clone)]
pub enum Entry<R> {
    Uniform(u16),
    Dense(R),
}

/// Dense grid of chunk entries over the loaded region; index 0 = air.
pub struct Grid<R> {
    pub min: IVec3,
    pub dims: IVec3,
    pub cells: Vec<u32>,
    pub entries: Vec<Entry<R>>,
}

impl<R> Default for Grid<R> {
    fn default() -> Self {
        Self { min: IVec3::ZERO, dims: IVec3::ZERO, cells: Vec::new(), entries: Vec::new() }
    }
}

impl<R: Repr> Grid<R> {
    /// Entry index (0 = air or outside) of the chunk containing absolute cell `abs`.
    #[inline(always)]
    pub fn chunk(&self, abs: IVec3) -> u32 {
        let c: IVec3 = (abs >> 5) - self.min;
        if c.x < 0 || c.y < 0 || c.z < 0 || c.x >= self.dims.x || c.y >= self.dims.y || c.z >= self.dims.z {
            return 0;
        }
        self.cells[((c.z * self.dims.y + c.y) * self.dims.x + c.x) as usize]
    }

    /// Entry index (0 = air or outside) of the chunk at chunk coordinates `ch`.
    #[inline(always)]
    pub fn chunk_at(&self, ch: IVec3) -> u32 {
        self.chunk(ch << 5)
    }

    /// (non-zero if solid, step size) at absolute cell `abs` inside chunk entry `idx`.
    #[inline(always)]
    pub fn probe_in(&self, idx: u32, abs: IVec3) -> (u32, i32) {
        if idx == 0 {
            return (0, 32);
        }
        match &self.entries[idx as usize - 1] {
            Entry::Uniform(b) => (*b as u32, 1),
            Entry::Dense(r) => r.probe(abs & 31),
        }
    }

    /// Block id at absolute cell `abs` (called once per hit).
    pub fn block_at(&self, abs: IVec3) -> u16 {
        match self.chunk(abs) {
            0 => 0,
            idx => match &self.entries[idx as usize - 1] {
                Entry::Uniform(b) => *b,
                Entry::Dense(r) => r.block(abs & 31),
            },
        }
    }
}


/// `floor` without a libm call (the baseline x86-64 target has no SSE4.1 `roundss`, so
/// `f32::floor` is a function call — it dominated the step loop).
#[inline(always)]
pub fn ffloor(x: f32) -> i32 {
    let i = x as i32;
    i - (x < i as f32) as i32
}

