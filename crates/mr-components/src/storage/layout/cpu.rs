//! u16 block ids in a reordered layout inside a chunk (from cpu-simd-loc): the `bricks`
//! storage with a different memory order, so that the voxels a ray touches share cache
//! lines. `L` picks the order: brick-major (each 4^3 brick's 64 ids contiguous, 128 B)
//! or Morton (Z-order over the chunk; also brick-contiguous, and neighbouring bricks are
//! close too).

use std::sync::Arc;

use glam::IVec3;
use mr_core::*;

use crate::storage::cpu::{Repr, brick_index, brick_set, bricks_of};

pub const BRICK: u8 = 1;
pub const MORTON: u8 = 2;

/// Spreads 5 bits to every third bit.
const fn spread5(v: u32) -> u16 {
    let mut r = 0u32;
    let mut i = 0;
    while i < 5 {
        r |= ((v >> i) & 1) << (3 * i);
        i += 1;
    }
    r as u16
}

const MORTON_TABLE: [u16; 32] = {
    let mut t = [0u16; 32];
    let mut i = 0;
    while i < 32 {
        t[i] = spread5(i as u32);
        i += 1;
    }
    t
};

/// Index of chunk-local cell `l` in layout `L`. Masked to the chunk so the compiler can
/// drop the bounds check of the id lookup.
#[inline(always)]
pub fn index<const L: u8>(l: IVec3) -> usize {
    (match L {
        BRICK => {
            let inner = (l.x & 3) | ((l.z & 3) << 2) | ((l.y & 3) << 4);
            ((brick_index(l) << 6) | inner as u32) as usize
        }
        _ => (MORTON_TABLE[(l.x & 31) as usize] | (MORTON_TABLE[(l.z & 31) as usize] << 1) | (MORTON_TABLE[(l.y & 31) as usize] << 2)) as usize,
    }) & (CHUNK_VOLUME - 1)
}

/// Brick mask plus the chunk's u16 ids in layout `L` (64 KiB, like `bricks`).
#[derive(Clone)]
pub struct Laid<const L: u8> {
    bricks: [u64; 8],
    ids: Arc<[u16; CHUNK_VOLUME]>,
}

impl<const L: u8> Repr for Laid<L> {
    fn build(d: &Arc<DenseChunk>) -> Self {
        let mut ids: Box<[u16; CHUNK_VOLUME]> = vec![0u16; CHUNK_VOLUME].into_boxed_slice().try_into().unwrap();
        // Source order: x fastest, then z, then y.
        let mut i = 0;
        for y in 0..32 {
            for z in 0..32 {
                for x in 0..32 {
                    ids[index::<L>(IVec3::new(x, y, z))] = d.0[i].0;
                    i += 1;
                }
            }
        }
        Self { bricks: bricks_of(d), ids: ids.into() }
    }
    #[inline(always)]
    fn probe(&self, l: IVec3) -> (u32, i32) {
        if !brick_set(&self.bricks, brick_index(l)) {
            return (0, 4);
        }
        (self.ids[index::<L>(l)] as u32, 1)
    }
    const LEVELS: u32 = 0b101;
    #[inline(always)]
    fn occupied(&self, level: u32, c: IVec3) -> bool {
        match level {
            0 => brick_set(&self.bricks, brick_index(c)) && self.ids[index::<L>(c)] != 0,
            _ => brick_set(&self.bricks, brick_index(c << 2)),
        }
    }
    #[inline(always)]
    fn block(&self, l: IVec3) -> u16 {
        self.ids[index::<L>(l)]
    }
    fn traversal_bytes(&self) -> usize {
        64 + CHUNK_VOLUME * 2
    }
    fn total_bytes(&self) -> usize {
        64 + CHUNK_VOLUME * 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mr_core::chunk::cell_of_index;

    #[test]
    fn layouts_are_bijections_and_brick_contiguous() {
        for f in [index::<BRICK> as fn(IVec3) -> usize, index::<MORTON>] {
            let mut seen = vec![false; CHUNK_VOLUME];
            for i in 0..CHUNK_VOLUME {
                let l = cell_of_index(i);
                let j = f(l);
                assert!(j < CHUNK_VOLUME && !seen[j]);
                seen[j] = true;
                assert_eq!(j >> 6, f(l & !3) >> 6);
            }
        }
    }
}
