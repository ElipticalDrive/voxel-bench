//! CPU implementation of the `bricks` storage component.

use std::sync::Arc;

use glam::IVec3;
#[allow(unused_imports)]
use mr_core::chunk::{cell_index, cell_of_index};
use mr_core::*;

#[allow(unused_imports)]
use crate::storage::cpu::{Repr, bit_in_brick, brick_index, brick_set, bricks_of, occupancy_of};


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
