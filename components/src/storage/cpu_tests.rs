//! Tests of the CPU storages.

#[allow(unused_imports)]
use crate::storage::cpu::*;
use glam::IVec3;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{bits::cpu::Bits, bricks::cpu::U16, hier::cpu::Hier, palette::cpu::Palette, prefix::cpu::Prefix, u8::cpu::U8};
    use mr_core::chunk::{cell_index, cell_of_index};
    use mr_core::*;
    use std::sync::Arc;

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
