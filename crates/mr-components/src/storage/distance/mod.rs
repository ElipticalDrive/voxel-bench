//! Storage `distance` (from gpu-sdf): per-voxel Chebyshev distance to the nearest solid
//! voxel of the chunk (u8, 0 = solid), four per u32 word in dense order. Fast path for the
//! `s_dist` query (sphere tracing).

use glam::IVec3;
use mr_core::chunk::cell_index;
use mr_core::{CHUNK_VOLUME, DenseChunk};

pub const GPU_WGSL: &str = include_str!("gpu.wgsl");
pub const AUX_WORDS: usize = CHUNK_VOLUME / 4;

/// Per-slot side data: the distance field packed four u8 per word.
pub fn build(d: &DenseChunk) -> Vec<u32> {
    bytemuck::cast_slice(&chebyshev_df(d)).to_vec()
}

/// Chebyshev distance (in voxels, capped at 32) from every cell to the nearest solid cell
/// of the chunk; 0 for solid cells. Separable: rows, then max-min passes along z and y.
pub fn chebyshev_df(chunk: &DenseChunk) -> Vec<u8> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use mr_core::BlockId;

    #[test]
    fn distance_field_matches_brute_force() {
        let mut c = DenseChunk::filled(BlockId::AIR);
        for p in [IVec3::new(3, 4, 5), IVec3::new(20, 30, 1), IVec3::new(31, 0, 31), IVec3::new(10, 10, 10)] {
            c.set(p, BlockId(1));
        }
        let solids: Vec<IVec3> = (0..CHUNK_VOLUME).map(mr_core::chunk::cell_of_index).filter(|p| c.get(*p).is_solid()).collect();
        let df = chebyshev_df(&c);
        for i in (0..CHUNK_VOLUME).step_by(37) {
            let p = mr_core::chunk::cell_of_index(i);
            let want = solids.iter().map(|s| (*s - p).abs().max_element()).min().unwrap().min(32);
            assert_eq!(df[i] as i32, want, "{p}");
        }
    }
}
