//! CPU implementation of the `hier` storage component.

use std::sync::Arc;

use glam::IVec3;
#[allow(unused_imports)]
use mr_core::chunk::{cell_index, cell_of_index};
use mr_core::*;

#[allow(unused_imports)]
use crate::storage::cpu::{Repr, bit_in_brick, brick_index, brick_set, bricks_of, occupancy_of};


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
