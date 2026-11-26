//! CPU implementation of the `hashed` storage component: voxel hashing (Niessner et al.
//! 2013) at brick granularity inside a chunk. Only the 4^3 bricks holding a block are kept,
//! each its 64 ids contiguous in a heap; an open-addressed table (linear probing, at most
//! half full) maps a brick's index to its place in the heap. The brick mask answers
//! emptiness, as in `bricks`; the table is asked only at bricks the mask says are solid.

use std::sync::Arc;

use glam::IVec3;
use mr_core::chunk::cell_of_index;
use mr_core::*;

use crate::storage::cpu::{Repr, bit_in_brick, brick_index, brick_set, bricks_of};

/// An empty table entry.
const EMPTY: u32 = u32::MAX;

#[derive(Clone)]
pub struct Hashed {
    /// One bit per brick holding a solid voxel (the emptiness test).
    bricks: [u64; 8],
    /// Entries `brick << 16 | heap index`, `EMPTY` where none; a power of two long.
    table: Box<[u32]>,
    /// The kept bricks' ids, 64 per brick in `bit_in_brick` order.
    heap: Box<[u16]>,
}

impl Hashed {
    #[inline(always)]
    fn home(&self, brick: u32) -> usize {
        (brick.wrapping_mul(0x9E37_79B1) >> 16) as usize & (self.table.len() - 1)
    }

    /// The heap index of `brick`'s ids, if it is kept.
    #[inline(always)]
    fn find(&self, brick: u32) -> Option<usize> {
        let mask = self.table.len() - 1;
        let mut i = self.home(brick);
        loop {
            let e = self.table[i];
            if e == EMPTY {
                return None;
            }
            if e >> 16 == brick {
                return Some((e & 0xffff) as usize);
            }
            i = (i + 1) & mask;
        }
    }
}

impl Repr for Hashed {
    fn build(d: &Arc<DenseChunk>) -> Self {
        // Bricks holding any block, see-through ones too: `block` answers every cell.
        let mut kept = [false; 512];
        for (i, b) in d.0.iter().enumerate() {
            if *b != BlockId::AIR {
                kept[brick_index(cell_of_index(i)) as usize] = true;
            }
        }
        let count = kept.iter().filter(|k| **k).count();
        let mut s = Self {
            bricks: bricks_of(d),
            table: vec![EMPTY; (count * 2).next_power_of_two().max(8)].into_boxed_slice(),
            heap: vec![0u16; count * 64].into_boxed_slice(),
        };
        let mask = s.table.len() - 1;
        for (next, brick) in (0..512u32).filter(|b| kept[*b as usize]).enumerate() {
            let mut i = s.home(brick);
            while s.table[i] != EMPTY {
                i = (i + 1) & mask;
            }
            s.table[i] = brick << 16 | next as u32;
        }
        for (i, b) in d.0.iter().enumerate() {
            if *b != BlockId::AIR {
                let l = cell_of_index(i);
                let at = s.find(brick_index(l)).expect("a kept brick");
                s.heap[at * 64 + bit_in_brick(l) as usize] = b.0;
            }
        }
        s
    }
    #[inline(always)]
    fn probe(&self, l: IVec3) -> (u32, i32) {
        if !brick_set(&self.bricks, brick_index(l)) {
            return (0, 4);
        }
        (self.block(l) as u32, 1)
    }
    const LEVELS: u32 = 0b101;
    #[inline(always)]
    fn occupied(&self, level: u32, c: IVec3) -> bool {
        match level {
            0 => brick_set(&self.bricks, brick_index(c)) && BlockId(self.block(c)).is_solid(),
            _ => brick_set(&self.bricks, brick_index(c << 2)),
        }
    }
    #[inline(always)]
    fn block(&self, l: IVec3) -> u16 {
        match self.find(brick_index(l)) {
            Some(at) => self.heap[at * 64 + bit_in_brick(l) as usize],
            None => 0,
        }
    }
    fn traversal_bytes(&self) -> usize {
        64 + self.table.len() * 4
    }
    fn total_bytes(&self) -> usize {
        64 + self.table.len() * 4 + self.heap.len() * 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mr_core::chunk::cell_index;

    #[test]
    fn empty_and_full_chunks_round_trip() {
        let air = Arc::new(DenseChunk(Box::new([BlockId::AIR; CHUNK_VOLUME])));
        let h = Hashed::build(&air);
        assert_eq!((h.heap.len(), h.table.len()), (0, 8));
        assert_eq!(h.block(IVec3::new(3, 4, 5)), 0);
        let full = Arc::new(DenseChunk(Box::new([BlockId(7); CHUNK_VOLUME])));
        let h = Hashed::build(&full);
        assert_eq!((h.heap.len(), h.table.len()), (512 * 64, 1024));
        assert!((0..CHUNK_VOLUME).all(|i| h.block(cell_of_index(i)) == 7 && cell_index(cell_of_index(i)) == i));
    }
}
