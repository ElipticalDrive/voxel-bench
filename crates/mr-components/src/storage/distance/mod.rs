//! Storage `distance` (from gpu-sdf): per-voxel Chebyshev distance to the nearest solid
//! voxel of the chunk (u8, 0 = solid), four per u32 word in dense order. Fast path for the
//! `s_dist` query (sphere tracing).

use mr_core::{CHUNK_VOLUME, DenseChunk};

pub const GPU_WGSL: &str = include_str!("gpu.wgsl");
pub const AUX_WORDS: usize = CHUNK_VOLUME / 4;

/// Per-slot side data: the distance field packed four u8 per word.
pub fn build(d: &DenseChunk) -> Vec<u32> {
    bytemuck::cast_slice(&chebyshev_df(d)).to_vec()
}

/// Chebyshev distance (in voxels, capped at 32) from every cell to the nearest solid cell
/// of the chunk; 0 for solid cells. Each row along x is a 32-bit mask; the solid set grows by
/// a 3x3x3 cube per step (shifts along x, ORs of neighbour rows along z and y), and a cell's
/// distance is the step that first covers it. At most 31 steps over 1024 words, stopping when
/// a step covers nothing new.
pub fn chebyshev_df(chunk: &DenseChunk) -> Vec<u8> {
    const ROWS: usize = CHUNK_VOLUME / 32; // row = z + 32 y (`cell_index` / 32), bit = x
    let mut cur = [0u32; ROWS];
    for (i, b) in chunk.0.iter().enumerate() {
        if b.is_solid() {
            cur[i >> 5] |= 1 << (i & 31);
        }
    }
    let mut out = vec![32u8; CHUNK_VOLUME];
    let mut mark = |fresh: &[u32; ROWS], k: u8| {
        for (r, &w) in fresh.iter().enumerate() {
            let mut w = w;
            while w != 0 {
                out[(r << 5) | w.trailing_zeros() as usize] = k;
                w &= w - 1;
            }
        }
    };
    mark(&cur, 0);
    let (mut a, mut b) = ([0u32; ROWS], [0u32; ROWS]);
    for k in 1..32u8 {
        for r in 0..ROWS {
            let c = cur[r];
            a[r] = c | (c << 1) | (c >> 1);
        }
        for r in 0..ROWS {
            let z = r & 31;
            let below = if z > 0 { a[r - 1] } else { 0 };
            let above = if z < 31 { a[r + 1] } else { 0 };
            b[r] = a[r] | below | above;
        }
        let mut any = false;
        for r in 0..ROWS {
            let below = if r >= 32 { b[r - 32] } else { 0 };
            let above = if r + 32 < ROWS { b[r + 32] } else { 0 };
            let grown = b[r] | below | above;
            a[r] = grown & !cur[r];
            any |= a[r] != 0;
            cur[r] = grown;
        }
        if !any {
            break;
        }
        mark(&a, k);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::IVec3;
    use mr_core::BlockId;
    use mr_core::chunk::cell_index;

    #[test]
    fn distance_field_matches_brute_force() {
        let mut c = DenseChunk::filled(BlockId::AIR);
        for p in [
            IVec3::new(3, 4, 5),
            IVec3::new(20, 30, 1),
            IVec3::new(31, 0, 31),
            IVec3::new(10, 10, 10),
        ] {
            c.set(p, BlockId(1));
        }
        let solids: Vec<IVec3> = (0..CHUNK_VOLUME)
            .map(mr_core::chunk::cell_of_index)
            .filter(|p| c.get(*p).is_solid())
            .collect();
        let df = chebyshev_df(&c);
        for i in (0..CHUNK_VOLUME).step_by(37) {
            let p = mr_core::chunk::cell_of_index(i);
            let want = solids
                .iter()
                .map(|s| (*s - p).abs().max_element())
                .min()
                .unwrap()
                .min(32);
            assert_eq!(df[i] as i32, want, "{p}");
        }
    }

    /// The former O(32^4) separable transform: rows, then max-min passes along z and y.
    fn separable(chunk: &DenseChunk) -> Vec<u8> {
        const INF: i32 = 64;
        let mut a = vec![INF; CHUNK_VOLUME];
        // x: 1D distance to the nearest solid in the row.
        for y in 0..32 {
            for z in 0..32 {
                let mut last = -INF;
                for x in 0..32 {
                    let i = cell_index(IVec3::new(x, y, z));
                    if chunk.0[i].is_solid() {
                        last = x;
                    }
                    a[i] = (x - last).min(INF);
                }
                let mut next = 2 * INF;
                for x in (0..32).rev() {
                    let i = cell_index(IVec3::new(x, y, z));
                    if chunk.0[i].is_solid() {
                        next = x;
                    }
                    a[i] = a[i].min(next - x);
                }
            }
        }
        // z, then y: d(c) = min over c' on the line of max(|c - c'|, d_prev(c')).
        let pass = |src: &Vec<i32>, axis: usize| -> Vec<i32> {
            let mut out = vec![INF; CHUNK_VOLUME];
            let mut line = [0i32; 32];
            for u in 0..32 {
                for v in 0..32 {
                    let at = |t: i32| -> usize {
                        let p = match axis {
                            2 => IVec3::new(u, v, t),
                            _ => IVec3::new(u, t, v),
                        };
                        cell_index(p)
                    };
                    for t in 0..32 {
                        line[t as usize] = src[at(t)];
                    }
                    for t in 0..32i32 {
                        let mut best = INF;
                        for s in 0..32i32 {
                            let cand = (t - s).abs().max(line[s as usize]);
                            best = best.min(cand);
                        }
                        out[at(t)] = best;
                    }
                }
            }
            out
        };
        let a = pass(&a, 2);
        let a = pass(&a, 1);
        a.into_iter().map(|d| d.min(32) as u8).collect()
    }

    #[test]
    fn matches_separable_transform() {
        // Random chunks from empty to nearly full, every cell compared.
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for density in [0u64, 1, 4, 30, 300, 3000, 30000, 65000] {
            let mut c = DenseChunk::filled(BlockId::AIR);
            for i in 0..CHUNK_VOLUME {
                if next() % 65536 < density {
                    c.0[i] = BlockId(1);
                }
            }
            assert_eq!(chebyshev_df(&c), separable(&c), "density {density}");
        }
        let mut c = DenseChunk::filled(BlockId(1));
        c.0[0] = BlockId::AIR;
        assert_eq!(chebyshev_df(&c), separable(&c), "full but one");
    }
}
