//! CPU implementation of the `palette` storage component.

use std::sync::Arc;

use glam::IVec3;
#[allow(unused_imports)]
use mr_core::chunk::{cell_index, cell_of_index};
use mr_core::*;

#[allow(unused_imports)]
use crate::storage::cpu::{Repr, bit_in_brick, brick_index, brick_set, bricks_of, occupancy_of};


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
