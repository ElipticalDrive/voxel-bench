//! CPU implementation of the `u8` storage component.

use std::sync::Arc;

use glam::IVec3;
#[allow(unused_imports)]
use mr_core::chunk::{cell_index, cell_of_index};
use mr_core::*;

#[allow(unused_imports)]
use crate::storage::cpu::{Repr, bit_in_brick, brick_index, brick_set, bricks_of, occupancy_of};


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
