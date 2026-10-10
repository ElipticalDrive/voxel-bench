//! CPU builder of the `prefix` storage's per-slot words (layout in `store_prefix.wgsl`).

use mr_core::DenseChunk;

/// u32 words per slot: 512 masks as (lo, hi) pairs, then 513 u16 prefixes in pairs.
pub const PREFIX_WORDS: usize = 1024 + 257;

/// Coordinate `axis` (0 = x, 1 = y, 2 = z) of a 3-bit-per-axis Morton index.
const fn unmorton3(m: usize, axis: usize) -> usize {
    ((m >> axis) & 1) | ((m >> (axis + 3)) & 1) << 1 | ((m >> (axis + 6)) & 1) << 2
}

/// Dense index (`x + 32 z + 1024 y`) of the origin of the brick with Morton index `m`.
const BRICK_BASE: [usize; 512] = {
    let mut t = [0; 512];
    let mut m = 0;
    while m < 512 {
        t[m] = 4 * unmorton3(m, 0) + 128 * unmorton3(m, 2) + 4096 * unmorton3(m, 1);
        m += 1;
    }
    t
};

/// Dense index offset of bit `k` (Morton order) within a brick.
const BIT_OFFSET: [usize; 64] = {
    let mut t = [0; 64];
    let mut k = 0;
    while k < 64 {
        t[k] = unmorton3(k, 0) + 32 * unmorton3(k, 2) + 1024 * unmorton3(k, 1);
        k += 1;
    }
    t
};

/// One pass in Morton order: masks and prefixes come out together (as in cpu-simd-mem).
pub fn build(d: &DenseChunk) -> Vec<u32> {
    let mut words = vec![0u32; PREFIX_WORDS];
    let mut sum = 0u32;
    for (m, &base) in BRICK_BASE.iter().enumerate() {
        let mut w = 0u64;
        for (k, &off) in BIT_OFFSET.iter().enumerate() {
            w |= (d.0[base + off].is_solid() as u64) << k;
        }
        words[2 * m] = w as u32;
        words[2 * m + 1] = (w >> 32) as u32;
        words[1024 + m / 2] |= sum << ((m & 1) * 16);
        sum += w.count_ones();
    }
    words[1024 + 256] |= sum;
    words
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::IVec3;
    use mr_core::chunk::{cell_index, cell_of_index};
    use mr_core::{BlockId, CHUNK_VOLUME};

    /// Morton index of up to 3 bits per axis, x lowest (matches `morton3` in the WGSL).
    fn morton3(x: u32, y: u32, z: u32) -> u32 {
        fn spread(v: u32) -> u32 {
            (v & 1) | (v & 2) << 2 | (v & 4) << 4
        }
        spread(x) | spread(y) << 1 | spread(z) << 2
    }

    fn prefix(w: &[u32], i: usize) -> u32 {
        (w[1024 + (i >> 1)] >> ((i & 1) * 16)) & 0xffff
    }

    /// Every aligned 2^k cube's emptiness read the way the shader reads it (bit, byte,
    /// word, prefix ranges of 8 and 64 bricks) matches brute force.
    #[test]
    fn layout_matches_brute_force() {
        let mut s = 0x9E37_79B9u64;
        let mut d = DenseChunk(Box::new([BlockId::AIR; CHUNK_VOLUME]));
        for (i, b) in d.0.iter_mut().enumerate() {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let l = cell_of_index(i);
            // Sparse: whole 8^3 and 16^3 regions empty, dense elsewhere.
            if (l.x >> 4) % 2 == 0 && (l.y >> 3) % 2 == 0 && s % 5 == 0 {
                *b = BlockId(1);
            }
        }
        let w = build(&d);
        assert_eq!(prefix(&w, 512) as usize, d.0.iter().filter(|b| b.is_solid()).count());
        for level in 0..5 {
            let (n, size) = (32 >> level, 1i32 << level);
            for i in 0..n * n * n {
                let c = IVec3::new(i % n, i / n % n, i / (n * n));
                let base = c * size;
                let solid = (0..size).any(|z| (0..size).any(|y| (0..size).any(|x| d.0[cell_index(base + IVec3::new(x, y, z))].is_solid())));
                let l = base.as_uvec3();
                let m = morton3(l.x >> 2, l.y >> 2, l.z >> 2) as usize;
                let k = morton3(l.x & 3, l.y & 3, l.z & 3);
                let mask = w[2 * m] as u64 | (w[2 * m + 1] as u64) << 32;
                let occ = match level {
                    0 => mask >> k & 1 != 0,
                    1 => mask >> k & 0xff != 0,
                    2 => mask != 0,
                    3 => prefix(&w, m & !7) != prefix(&w, (m & !7) + 8),
                    _ => prefix(&w, m & !63) != prefix(&w, (m & !63) + 64),
                };
                assert_eq!(occ, solid, "level {level} at {c}");
            }
        }
    }
}
