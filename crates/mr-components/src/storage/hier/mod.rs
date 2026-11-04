//! Storage `hier`: bricks plus a 64-bit summary per chunk, one bit per 8^3 region (as
//! cpu-simd-hier).

pub mod cpu;

pub const GPU_WGSL: &str = include_str!("gpu.wgsl");
pub const AUX_WORDS: usize = 2;

/// One bit per 8^3 region (index `x | z << 2 | y << 4`). Dense order is x + 32 z + 1024 y,
/// so the region index is bits 3-4 of each coordinate; branch-free.
pub fn build(d: &mr_core::DenseChunk) -> Vec<u32> {
    let mut bits = 0u64;
    for (i, b) in d.0.iter().enumerate() {
        let r = (i >> 3) & 3 | ((i >> 8) & 3) << 2 | ((i >> 13) & 3) << 4;
        bits |= (b.is_solid() as u64) << r;
    }
    vec![bits as u32, (bits >> 32) as u32]
}
