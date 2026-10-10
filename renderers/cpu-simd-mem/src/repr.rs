//! Per-chunk voxel representations. All answer the same two questions:
//! `probe(local)` → (non-zero if solid, step size for the DDA) and `block(local)` → id.
//! Only `probe` is used inside the traversal loop; `block` runs once per primary hit.

use std::sync::Arc;

use glam::IVec3;
use mr_core::chunk::{cell_index, cell_of_index};
use mr_core::*;

pub trait Repr: Clone + Send + Sync + 'static {
    fn build(d: &Arc<DenseChunk>) -> Self;
    /// (non-zero if solid, step size) at chunk-local cell `l` (0..32 per axis).
    fn probe(&self, l: IVec3) -> (u32, i32);
    /// Levels `k` (bit k set) at which `occupied` answers directly for aligned 2^k cubes.
    /// Always includes 0; the chunk level (5) is answered by the chunk grid instead.
    const LEVELS: u32;
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
fn brick_index(l: IVec3) -> u32 {
    let b: IVec3 = l >> 2;
    (b.x | (b.z << 3) | (b.y << 6)) as u32
}

/// Bit within a brick's u64 (brick-major layout).
#[inline(always)]
fn bit_in_brick(l: IVec3) -> u32 {
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

fn occupancy_of(d: &DenseChunk) -> Box<[u64; 512]> {
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
fn brick_set(bricks: &[u64; 8], bi: u32) -> bool {
    bricks[(bi >> 6) as usize] >> (bi & 63) & 1 != 0
}

// --------------------------------------------------------------------- u16 (control)

/// cpu-simd's representation: 512-bit brick mask + u16 per voxel (64 KiB).
#[derive(Clone)]
pub struct U16 {
    bricks: [u64; 8],
    blocks: Arc<DenseChunk>,
}

impl Repr for U16 {
    fn build(d: &Arc<DenseChunk>) -> Self {
        Self { bricks: bricks_of(d), blocks: d.clone() }
    }
    #[inline(always)]
    fn probe(&self, l: IVec3) -> (u32, i32) {
        if !brick_set(&self.bricks, brick_index(l)) {
            return (0, 4);
        }
        (self.blocks.0[cell_index(l)].0 as u32, 1)
    }
    const LEVELS: u32 = 0b101;
    #[inline(always)]
    fn occupied(&self, level: u32, c: IVec3) -> bool {
        match level {
            0 => brick_set(&self.bricks, brick_index(c)) && self.blocks.0[cell_index(c)].is_solid(),
            _ => brick_set(&self.bricks, brick_index(c << 2)),
        }
    }
    #[inline(always)]
    fn block(&self, l: IVec3) -> u16 {
        self.blocks.0[cell_index(l)].0
    }
    fn traversal_bytes(&self) -> usize {
        64 + CHUNK_VOLUME * 2
    }
    fn total_bytes(&self) -> usize {
        64 + CHUNK_VOLUME * 2
    }
}

// --------------------------------------------------------------------- u8

/// Brick mask + u8 per voxel (32 KiB); only valid with ≤ 256 block types.
#[derive(Clone)]
pub struct U8 {
    bricks: [u64; 8],
    ids: Arc<[u8]>,
}

impl Repr for U8 {
    fn build(d: &Arc<DenseChunk>) -> Self {
        Self { bricks: bricks_of(d), ids: d.0.iter().map(|b| b.0 as u8).collect() }
    }
    #[inline(always)]
    fn probe(&self, l: IVec3) -> (u32, i32) {
        if !brick_set(&self.bricks, brick_index(l)) {
            return (0, 4);
        }
        (self.ids[cell_index(l)] as u32, 1)
    }
    const LEVELS: u32 = 0b101;
    #[inline(always)]
    fn occupied(&self, level: u32, c: IVec3) -> bool {
        match level {
            0 => brick_set(&self.bricks, brick_index(c)) && self.ids[cell_index(c)] != 0,
            _ => brick_set(&self.bricks, brick_index(c << 2)),
        }
    }
    #[inline(always)]
    fn block(&self, l: IVec3) -> u16 {
        self.ids[cell_index(l)] as u16
    }
    fn traversal_bytes(&self) -> usize {
        64 + CHUNK_VOLUME
    }
    fn total_bytes(&self) -> usize {
        64 + CHUNK_VOLUME
    }
}

// --------------------------------------------------------------------- bits

/// 1 bit per voxel, brick-major (one u64 per 4^3 brick, so the brick test and the voxel
/// test read the same word): 4 KiB per chunk. Ids stay in the shared u16 chunk and are
/// read only on the final hit.
#[derive(Clone)]
pub struct Bits {
    occ: Arc<[u64; 512]>,
    ids: Arc<DenseChunk>,
}

impl Repr for Bits {
    fn build(d: &Arc<DenseChunk>) -> Self {
        Self { occ: occupancy_of(d).into(), ids: d.clone() }
    }
    #[inline(always)]
    fn probe(&self, l: IVec3) -> (u32, i32) {
        let w = self.occ[brick_index(l) as usize];
        if w == 0 {
            return (0, 4);
        }
        ((w >> bit_in_brick(l)) as u32 & 1, 1)
    }
    const LEVELS: u32 = 0b101;
    #[inline(always)]
    fn occupied(&self, level: u32, c: IVec3) -> bool {
        match level {
            0 => self.occ[brick_index(c) as usize] >> bit_in_brick(c) & 1 != 0,
            _ => self.occ[brick_index(c << 2) as usize] != 0,
        }
    }
    #[inline(always)]
    fn block(&self, l: IVec3) -> u16 {
        self.ids.0[cell_index(l)].0
    }
    fn traversal_bytes(&self) -> usize {
        4096
    }
    fn total_bytes(&self) -> usize {
        4096 + CHUNK_VOLUME * 2
    }
}

// --------------------------------------------------------------------- hier

/// `Bits` plus a 64-bit summary, one bit per 8^3 region, so empty regions skip 8 cells.
#[derive(Clone)]
pub struct Hier {
    summary: u64,
    occ: Arc<[u64; 512]>,
    ids: Arc<DenseChunk>,
}

#[inline(always)]
fn region_index(l: IVec3) -> u32 {
    let r: IVec3 = l >> 3;
    (r.x | (r.z << 2) | (r.y << 4)) as u32
}

impl Repr for Hier {
    fn build(d: &Arc<DenseChunk>) -> Self {
        let occ = occupancy_of(d);
        let mut summary = 0u64;
        for (bi, w) in occ.iter().enumerate() {
            if *w != 0 {
                let b = IVec3::new(bi as i32 & 7, bi as i32 >> 6, (bi as i32 >> 3) & 7);
                summary |= 1 << region_index(b * 4);
            }
        }
        Self { summary, occ: occ.into(), ids: d.clone() }
    }
    #[inline(always)]
    fn probe(&self, l: IVec3) -> (u32, i32) {
        if self.summary >> region_index(l) & 1 == 0 {
            return (0, 8);
        }
        let w = self.occ[brick_index(l) as usize];
        if w == 0 {
            return (0, 4);
        }
        ((w >> bit_in_brick(l)) as u32 & 1, 1)
    }
    const LEVELS: u32 = 0b1101;
    #[inline(always)]
    fn occupied(&self, level: u32, c: IVec3) -> bool {
        match level {
            0 => self.occ[brick_index(c) as usize] >> bit_in_brick(c) & 1 != 0,
            2 => self.occ[brick_index(c << 2) as usize] != 0,
            _ => self.summary >> region_index(c << 3) & 1 != 0,
        }
    }
    #[inline(always)]
    fn block(&self, l: IVec3) -> u16 {
        self.ids.0[cell_index(l)].0
    }
    fn traversal_bytes(&self) -> usize {
        8 + 4096
    }
    fn total_bytes(&self) -> usize {
        8 + 4096 + CHUNK_VOLUME * 2
    }
}

// --------------------------------------------------------------------- prefix

/// Interleaves the low 3 bits of `x`, `y`, `z` (Morton order, x lowest).
#[inline(always)]
fn morton3(x: u32, y: u32, z: u32) -> u32 {
    #[inline(always)]
    fn spread(v: u32) -> u32 {
        (v & 1) | (v & 2) << 2 | (v & 4) << 4
    }
    spread(x) | spread(y) << 1 | spread(z) << 2
}

/// Morton index (0..512) of the 4^3 brick containing chunk-local cell `l`.
#[inline(always)]
fn brick_morton(l: IVec3) -> u32 {
    let b = (l >> 2i32).as_uvec3();
    morton3(b.x, b.y, b.z)
}

/// Morton index (0..64) of `l` within its brick: the two Morton levels inside one u64, so
/// each byte is a 2^3 sub-cube.
#[inline(always)]
fn bit_morton(l: IVec3) -> u32 {
    let c = (l & 3i32).as_uvec3();
    morton3(c.x, c.y, c.z)
}

/// The user's prefix-popcount hierarchy. One u64 occupancy mask per 4^3 brick with bricks
/// *and* the bits inside each mask in Morton order, plus `prefix[i]` = total popcount of
/// masks `0..i`. Every aligned 2^k cube of bricks is a contiguous Morton range `[a, a+n)`,
/// so it is empty exactly when `prefix[a] == prefix[a + n]`: one compare per level, every
/// level from the same array. The prefix is also a rank: a solid voxel's id lives at
/// `prefix[brick] + popcount(mask & below(bit))` in `ids`, which holds solid voxels only.
#[derive(Clone)]
pub struct Prefix {
    /// Masks and prefixes in one allocation: one pointer to chase per probe.
    h: Arc<PrefixData>,
    ids: Arc<[u16]>,
}

/// Dense index of the origin of the brick with Morton index `m` (dense layout is linear:
/// `x + 32 z + 1024 y`, so offsets add).
const BRICK_BASE: [usize; 512] = {
    let mut t = [0usize; 512];
    let mut m = 0;
    while m < 512 {
        let (x, y, z) = (unmorton3(m, 0), unmorton3(m, 1), unmorton3(m, 2));
        t[m] = 4 * x + 32 * 4 * z + 1024 * 4 * y;
        m += 1;
    }
    t
};

/// Dense index offset of bit `k` (Morton order) within a brick.
const BIT_OFFSET: [u16; 64] = {
    let mut t = [0u16; 64];
    let mut k = 0;
    while k < 64 {
        let (x, y, z) = (unmorton3(k, 0), unmorton3(k, 1), unmorton3(k, 2));
        t[k] = (x + 32 * z + 1024 * y) as u16;
        k += 1;
    }
    t
};

/// Coordinate `axis` (0 = x, 1 = y, 2 = z) of a 3-bit-per-axis Morton index.
const fn unmorton3(m: usize, axis: usize) -> usize {
    ((m >> axis) & 1) | ((m >> (axis + 3)) & 1) << 1 | ((m >> (axis + 6)) & 1) << 2
}

#[repr(C, align(64))]
struct PrefixData {
    occ: [u64; 512],
    prefix: [u16; 513],
}

impl Prefix {
    #[inline(always)]
    fn rank(&self, l: IVec3) -> usize {
        let m = brick_morton(l) as usize;
        let below = (1u64 << bit_morton(l)) - 1;
        self.h.prefix[m] as usize + (self.h.occ[m] & below).count_ones() as usize
    }

    /// Whether the aligned cube of `n` bricks (a power of 8) containing brick `m` is empty.
    #[inline(always)]
    fn empty_range(&self, m: u32, n: u32) -> bool {
        let a = (m & !(n - 1)) as usize;
        self.h.prefix[a] == self.h.prefix[a + n as usize]
    }
}

impl Repr for Prefix {
    /// One pass in Morton order (bricks, then bits within a brick), which is also the rank
    /// order of the compressed ids, so masks, prefixes and ids come out together.
    fn build(d: &Arc<DenseChunk>) -> Self {
        let mut h = Box::new(PrefixData { occ: [0; 512], prefix: [0; 513] });
        let mut ids = Vec::with_capacity(CHUNK_VOLUME / 4);
        for m in 0..512 {
            let base = BRICK_BASE[m];
            let mut w = 0u64;
            for (k, off) in BIT_OFFSET.iter().enumerate() {
                let b = d.0[base + *off as usize];
                if b.is_solid() {
                    w |= 1 << k;
                    ids.push(b.0);
                }
            }
            h.occ[m] = w;
            h.prefix[m + 1] = h.prefix[m] + w.count_ones() as u16;
        }
        Self { h: h.into(), ids: ids.into() }
    }
    /// Bottom-up: the cell itself, then the largest empty aligned cube containing it
    /// (emptiness is monotone, so stop at the first occupied level).
    #[inline(always)]
    fn probe(&self, l: IVec3) -> (u32, i32) {
        let m = brick_morton(l);
        let w = self.h.occ[m as usize];
        let bit = bit_morton(l);
        if w >> bit & 1 != 0 {
            return (1, 1);
        }
        if w >> (bit & !7) & 0xff != 0 {
            return (0, 1);
        }
        if w != 0 {
            return (0, 2);
        }
        if !self.empty_range(m, 8) {
            return (0, 4);
        }
        if !self.empty_range(m, 64) {
            return (0, 8);
        }
        (0, 16)
    }
    /// Top-down use of the same hierarchy: bit, byte (2^3), word (4^3), prefix ranges of
    /// 8 and 64 bricks (8^3, 16^3). Each level is a single test.
    const LEVELS: u32 = 0b11111;
    #[inline(always)]
    fn occupied(&self, level: u32, c: IVec3) -> bool {
        let l = c << level as i32;
        let m = brick_morton(l);
        match level {
            0 => self.h.occ[m as usize] >> bit_morton(l) & 1 != 0,
            1 => self.h.occ[m as usize] >> bit_morton(l) & 0xff != 0,
            2 => self.h.occ[m as usize] != 0,
            3 => !self.empty_range(m, 8),
            _ => !self.empty_range(m, 64),
        }
    }
    #[inline(always)]
    fn block(&self, l: IVec3) -> u16 {
        let m = brick_morton(l) as usize;
        if self.h.occ[m] >> bit_morton(l) & 1 == 0 {
            return 0;
        }
        self.ids[self.rank(l)]
    }
    /// The inverse of `build`: walking in Morton order, ids come out in rank order.
    fn to_dense(&self) -> DenseChunk {
        let mut d = DenseChunk(Box::new([BlockId::AIR; CHUNK_VOLUME]));
        let mut next = self.ids.iter();
        for m in 0..512 {
            let mut w = self.h.occ[m];
            while w != 0 {
                let k = w.trailing_zeros() as usize;
                d.0[BRICK_BASE[m] + BIT_OFFSET[k] as usize] = BlockId(*next.next().unwrap());
                w &= w - 1;
            }
        }
        d
    }
    fn traversal_bytes(&self) -> usize {
        4096 + 513 * 2
    }
    fn total_bytes(&self) -> usize {
        4096 + 513 * 2 + self.ids.len() * 2
    }
}

// --------------------------------------------------------------------- palette

/// Per-chunk palette (index 0 is always air) with indices packed at 1/2/4/8/16 bits in
/// brick-major voxel order (a brick's 64 indices are contiguous: ≤ 1 cache line up to
/// 8 bits). Traversal reads packed indices; occupancy = index != 0.
#[derive(Clone)]
pub struct Palette {
    bricks: [u64; 8],
    /// log2 of bits per index (0..=4).
    shift: u32,
    palette: Arc<[u16]>,
    data: Arc<[u64]>,
}

impl Palette {
    #[inline(always)]
    fn index(&self, l: IVec3) -> u32 {
        let v = brick_index(l) * 64 + bit_in_brick(l);
        let bit = v << self.shift;
        let mask = (1u64 << (1u32 << self.shift)) - 1;
        (self.data[(bit >> 6) as usize] >> (bit & 63) & mask) as u32
    }
}

impl Repr for Palette {
    fn build(d: &Arc<DenseChunk>) -> Self {
        let mut palette: Vec<u16> = vec![0];
        let mut lut = vec![u16::MAX; 65536];
        lut[0] = 0;
        for b in d.0.iter() {
            if lut[b.0 as usize] == u16::MAX {
                lut[b.0 as usize] = palette.len() as u16;
                palette.push(b.0);
            }
        }
        let shift = match palette.len() {
            0..=2 => 0,
            3..=4 => 1,
            5..=16 => 2,
            17..=256 => 3,
            _ => 4,
        };
        let mut data = vec![0u64; (CHUNK_VOLUME << shift) / 64];
        for (i, b) in d.0.iter().enumerate() {
            let l = cell_of_index(i);
            let v = brick_index(l) * 64 + bit_in_brick(l);
            let bit = v << shift;
            data[(bit >> 6) as usize] |= (lut[b.0 as usize] as u64) << (bit & 63);
        }
        Self { bricks: bricks_of(d), shift, palette: palette.into(), data: data.into() }
    }
    #[inline(always)]
    fn probe(&self, l: IVec3) -> (u32, i32) {
        if !brick_set(&self.bricks, brick_index(l)) {
            return (0, 4);
        }
        (self.index(l), 1)
    }
    const LEVELS: u32 = 0b101;
    #[inline(always)]
    fn occupied(&self, level: u32, c: IVec3) -> bool {
        match level {
            0 => brick_set(&self.bricks, brick_index(c)) && self.index(c) != 0,
            _ => brick_set(&self.bricks, brick_index(c << 2)),
        }
    }
    #[inline(always)]
    fn block(&self, l: IVec3) -> u16 {
        self.palette[self.index(l) as usize]
    }
    fn traversal_bytes(&self) -> usize {
        64 + self.data.len() * 8
    }
    fn total_bytes(&self) -> usize {
        64 + self.data.len() * 8 + self.palette.len() * 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// xorshift
    fn rng(s: &mut u64) -> u64 {
        *s ^= *s << 13;
        *s ^= *s >> 7;
        *s ^= *s << 17;
        *s
    }

    fn random_chunk(seed: u64, distinct: u64, air_pct: u64) -> Arc<DenseChunk> {
        let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut d = DenseChunk(Box::new([BlockId::AIR; CHUNK_VOLUME]));
        // Some chunks: sparse bricks (whole 4^3 / 8^3 regions empty) to exercise skips.
        let sparse = seed % 3 == 0;
        for (i, b) in d.0.iter_mut().enumerate() {
            let l = cell_of_index(i);
            if sparse && (l.x >> 3) % 2 == 1 {
                continue;
            }
            if rng(&mut s) % 100 >= air_pct {
                *b = BlockId(1 + (rng(&mut s) % distinct) as u16);
            }
        }
        Arc::new(d)
    }

    fn check<R: Repr>(d: &Arc<DenseChunk>) {
        let r = R::build(d);
        let bricks = bricks_of(d);
        for i in 0..CHUNK_VOLUME {
            let l = cell_of_index(i);
            let id = d.0[i].0;
            assert_eq!(r.block(l), id, "block at {l}");
            let (solid, step) = r.probe(l);
            let bi = brick_index(l);
            if brick_set(&bricks, bi) {
                assert_eq!(step, 1, "step at {l}");
                assert_eq!(solid != 0, id != 0, "solid at {l}");
            } else {
                assert_eq!(solid, 0);
                assert!(step >= 4, "empty brick must skip at least the brick at {l}");
            }
            assert_empty_cube(d, l, step, solid);
        }
    }

    /// A step larger than 1 claims the aligned cube of that size around `l` is empty.
    fn assert_empty_cube(d: &DenseChunk, l: IVec3, step: i32, solid: u32) {
        assert!(step.count_ones() == 1 && step <= 32, "step {step}");
        if step == 1 {
            assert_eq!(solid != 0, d.0[cell_index(l)].is_solid(), "solid at {l}");
            return;
        }
        let base = l & !(step - 1);
        for z in 0..step {
            for y in 0..step {
                for x in 0..step {
                    assert!(d.0[cell_index(base + IVec3::new(x, y, z))].is_air(), "step {step} at {l} skips a solid cell");
                }
            }
        }
    }

    /// The prefix hierarchy's steps are exact: every claimed empty cube is empty, and no
    /// larger aligned cube (up to 16) is empty — the probe never under-skips.
    fn check_prefix(d: &Arc<DenseChunk>) {
        let r = Prefix::build(d);
        assert!(r.to_dense().0.iter().zip(d.0.iter()).all(|(a, b)| a == b), "to_dense round trip");
        let empty = |base: IVec3, s: i32| (0..s).all(|z| (0..s).all(|y| (0..s).all(|x| d.0[cell_index(base + IVec3::new(x, y, z))].is_air())));
        for i in 0..CHUNK_VOLUME {
            let l = cell_of_index(i);
            assert_eq!(r.block(l), d.0[i].0, "block at {l}");
            let (solid, step) = r.probe(l);
            assert_empty_cube(d, l, step, solid);
            if solid == 0 && step < 16 {
                assert!(!empty(l & !(2 * step - 1), 2 * step), "under-skip at {l}: step {step}");
            }
        }
    }

    #[test]
    fn prefix_hierarchy_is_exact() {
        for (seed, distinct, air) in [(1, 1, 50), (2, 3, 90), (3, 15, 30), (4, 200, 70), (5, 1000, 50), (6, 1, 0), (7, 2, 100), (9, 5, 99), (11, 4, 97), (12, 7, 100)] {
            check_prefix(&random_chunk(seed, distinct, air));
        }
        // Structured: one solid voxel, and a half-filled chunk.
        let mut d = DenseChunk(Box::new([BlockId::AIR; CHUNK_VOLUME]));
        d.0[cell_index(IVec3::new(17, 3, 29))] = BlockId(5);
        check_prefix(&Arc::new(d));
        let mut d = DenseChunk(Box::new([BlockId::AIR; CHUNK_VOLUME]));
        for i in 0..CHUNK_VOLUME {
            if cell_of_index(i).y < 16 {
                d.0[i] = BlockId(2);
            }
        }
        check_prefix(&Arc::new(d));
    }

    /// `occupied` is exact at every level a representation claims in `LEVELS`.
    fn check_occupied<R: Repr>(d: &Arc<DenseChunk>) {
        let r = R::build(d);
        assert!(R::LEVELS & 1 != 0 && R::LEVELS >> 5 == 0);
        for level in (0..5).filter(|k| R::LEVELS >> k & 1 != 0) {
            let (n, s) = (32 >> level, 1 << level);
            for i in 0..n * n * n {
                let c = IVec3::new(i % n, i / n % n, i / (n * n));
                let base = c * s;
                let solid = (0..s).any(|z| (0..s).any(|y| (0..s).any(|x| d.0[cell_index(base + IVec3::new(x, y, z))].is_solid())));
                assert_eq!(r.occupied(level as u32, c), solid, "level {level} at {c}");
            }
        }
    }

    #[test]
    fn occupied_is_exact() {
        for (seed, distinct, air) in [(1, 1, 50), (2, 3, 90), (3, 15, 30), (6, 1, 0), (7, 2, 100), (9, 5, 99), (11, 4, 97), (12, 7, 100)] {
            let d = random_chunk(seed, distinct, air);
            check_occupied::<U16>(&d);
            check_occupied::<U8>(&d);
            check_occupied::<Bits>(&d);
            check_occupied::<Hier>(&d);
            check_occupied::<Prefix>(&d);
            check_occupied::<Palette>(&d);
        }
    }

    #[test]
    fn representations_agree() {
        for (seed, distinct, air) in [(1, 1, 50), (2, 3, 90), (3, 15, 30), (4, 200, 70), (5, 1000, 50), (6, 1, 0), (7, 2, 100), (9, 5, 99)] {
            let d = random_chunk(seed, distinct, air);
            check::<U16>(&d);
            check::<Bits>(&d);
            check::<Hier>(&d);
            check::<Palette>(&d);
            if distinct < 255 {
                check::<U8>(&d);
            }
        }
    }

    #[test]
    fn palette_bit_widths() {
        let sizes: Vec<usize> = [(1u64, 1u64), (3, 3), (12, 12), (200, 200), (1000, 1000)]
            .iter()
            .map(|&(seed, n)| Palette::build(&random_chunk(seed + 10, n, 0)).traversal_bytes())
            .collect();
        assert_eq!(sizes, vec![64 + 4096, 64 + 8192, 64 + 16384, 64 + 32768, 64 + 65536]);
    }
}
