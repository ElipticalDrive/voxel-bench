//! Tests of the CPU visibility components.

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use glam::{IVec3, Vec3};
    use mr_core::*;

    use crate::storage::bits::cpu::Bits;
    use crate::storage::bricks::cpu::U16;
    use crate::storage::cpu::{Entry, Grid, Repr};
    use crate::storage::hier::cpu::Hier;
    use crate::storage::palette::cpu::Palette;
    use crate::storage::prefix::cpu::Prefix;
    use crate::visibility::incremental::cpu::{level_above, level_below, trace_incremental};
    use crate::visibility::refloor::cpu::trace_packet;

    fn rng(s: &mut u64) -> u64 {
        *s ^= *s << 13;
        *s ^= *s >> 7;
        *s ^= *s << 17;
        *s
    }

    fn unit(s: &mut u64) -> f32 {
        (rng(s) >> 40) as f32 / (1u64 << 24) as f32
    }

    /// A 3×2×3-chunk world at negative and positive coordinates: rolling terrain with
    /// floating debris, one missing (air) chunk and one uniform stone chunk.
    fn world<R: Repr>(seed: u64) -> Grid<R> {
        let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut scene_chunks: HashMap<IVec3, Entry<R>> = HashMap::new();
        for cz in -1..=1 {
            for cy in -1..=0 {
                for cx in -1..=1 {
                    let pos = IVec3::new(cx, cy, cz);
                    if pos == IVec3::new(1, 0, -1) {
                        continue;
                    }
                    if pos == IVec3::new(-1, -1, 1) {
                        scene_chunks.insert(pos, Entry::Uniform(3));
                        continue;
                    }
                    let mut d = DenseChunk(Box::new([BlockId::AIR; CHUNK_VOLUME]));
                    for i in 0..CHUNK_VOLUME {
                        let w = pos * 32 + cell_of(i);
                        let h = (8.0 * ((w.x as f32 * 0.21).sin() + (w.z as f32 * 0.17).cos())) as i32 - 12;
                        if w.y < h || rng(&mut s) % 400 == 0 {
                            d.0[i] = BlockId(1 + (rng(&mut s) % 4) as u16);
                        }
                    }
                    scene_chunks.insert(pos, Entry::Dense(R::build(&Arc::new(d))));
                }
            }
        }
        let lo = IVec3::new(-1, -1, -1);
        let dims = IVec3::new(3, 2, 3);
        let mut cells = vec![0u32; 18];
        let mut entries = Vec::new();
        for (pos, e) in scene_chunks {
            entries.push(e);
            let c = pos - lo;
            cells[((c.z * dims.y + c.y) * dims.x + c.x) as usize] = entries.len() as u32;
        }
        Grid { min: lo, dims, cells, entries }
    }

    fn cell_of(i: usize) -> IVec3 {
        mr_core::chunk::cell_of_index(i)
    }

    fn solid<R: Repr>(g: &Grid<R>, abs: IVec3) -> bool {
        g.block_at(abs) != 0
    }

    /// The incremental traversal finds the same first hit as the re-flooring one, for every
    /// representation. Disagreements are allowed only where the ray grazes an edge (both
    /// hits at nearly the same t), and every reported hit must be a solid cell.
    fn agree<R: Repr, const EXACT: bool, const LEVELS: u32>(seed: u64) {
        let g = world::<R>(seed);
        let (bmin_abs, bmax_abs) = (g.min * 32, (g.min + g.dims) * 32);
        let mut s = seed | 1;
        let (mut n, mut differ) = (0, 0);
        for _ in 0..4000 {
            // Camera block somewhere in the world, origin fraction within it.
            let cam = IVec3::new(
                bmin_abs.x + (rng(&mut s) % 96) as i32,
                bmin_abs.y + 20 + (rng(&mut s) % 40) as i32,
                bmin_abs.z + (rng(&mut s) % 96) as i32,
            );
            let o = Vec3::new(unit(&mut s), unit(&mut s), unit(&mut s));
            let mut d = Vec3::new(unit(&mut s) - 0.5, unit(&mut s) - 0.7, unit(&mut s) - 0.5);
            // Some axis-aligned and diagonal rays.
            match rng(&mut s) % 8 {
                0 => d.x = 0.0,
                1 => (d.x, d.z) = (0.0, 0.0),
                2 => d = Vec3::new(1.0, -1.0, 1.0),
                _ => {}
            }
            let d = d.normalize();
            let (bmin, bmax) = (bmin_abs - cam, bmax_abs - cam);
            let mut out = [None];
            trace_packet::<R, 1>(&g, cam, o, &[d.x], &[d.y], &[d.z], 1e7, bmin, bmax, &mut out);
            let (inc, _) = trace_incremental::<R, EXACT>(&g, LEVELS, cam, o, d, 1e7, bmin, bmax);
            if let Some(h) = inc {
                assert!(solid(&g, h.cell + cam), "incremental hit an air cell");
                assert!(h.cell != o.floor().as_ivec3(), "hit the start cell");
            }
            n += 1;
            match (out[0], inc) {
                (Some(a), Some(b)) if a.cell == b.cell => {}
                (None, None) => {}
                (Some(a), Some(b)) => {
                    assert!((a.t - b.t).abs() < 1e-3 * a.t.max(1.0), "hits differ: {:?}/{} vs {:?}/{}", a.cell, a.t, b.cell, b.t);
                    differ += 1;
                }
                (a, b) => panic!("hit vs miss: {:?} vs {:?} (cam {cam}, o {o}, d {d})", a.map(|h| (h.cell, h.t)), b.map(|h| (h.cell, h.t))),
            }
        }
        assert!(differ * 1000 <= n, "{differ}/{n} rays differ");
    }

    #[test]
    fn incremental_matches_refloor() {
        for seed in [1, 2, 3] {
            agree::<U16, false, 0b111111>(seed);
            agree::<Bits, false, 0b111111>(seed);
            agree::<Hier, false, 0b111111>(seed);
            agree::<Prefix, false, 0b111111>(seed);
            agree::<Palette, false, 0b111111>(seed);
            agree::<crate::storage::morton::Cpu, true, 0b111111>(seed);
            agree::<Hier, true, 0b111111>(seed);
            agree::<Prefix, true, 0b111111>(seed);
            // Level subsets (the `skip*` knobs).
            agree::<Prefix, true, 0b101101>(seed);
            agree::<Prefix, true, 0b100001>(seed);
            agree::<Prefix, false, 0b110011>(seed);
        }
    }

    #[test]
    fn level_masks() {
        assert_eq!(level_above(0b100101, 0), 2);
        assert_eq!(level_above(0b100101, 2), 5);
        assert_eq!(level_below(0b100101, 5), 2);
        assert_eq!(level_below(0b111111, 1), 0);
    }
}
