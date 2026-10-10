//! CPU implementation of the `bits` storage component.

use std::sync::Arc;

use glam::IVec3;
#[allow(unused_imports)]
use mr_core::chunk::{cell_index, cell_of_index};
use mr_core::*;

#[allow(unused_imports)]
use crate::storage::cpu::{Repr, bit_in_brick, brick_index, brick_set, bricks_of, occupancy_of};


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
