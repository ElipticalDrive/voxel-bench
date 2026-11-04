//! CPU implementation of the `prefix` storage component.

use std::sync::Arc;

use glam::IVec3;
#[allow(unused_imports)]
use mr_core::chunk::{cell_index, cell_of_index};
use mr_core::*;

#[allow(unused_imports)]
use crate::storage::cpu::{Repr, bit_in_brick, brick_index, brick_set, bricks_of, occupancy_of};


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
        for (m, &base) in BRICK_BASE.iter().enumerate() {
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
        for (&base, &occ) in BRICK_BASE.iter().zip(&self.h.occ) {
            let mut w = occ;
            while w != 0 {
                let k = w.trailing_zeros() as usize;
                d.0[base + BIT_OFFSET[k] as usize] = BlockId(*next.next().unwrap());
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
