//! Storage `rows` (from gpu-bitmask): bricks plus 32 x 32 u32 occupancy rows per chunk
//! (row y * 32 + z, bit x). Fast path for the `s_row` query (row-run jumps).

pub const GPU_WGSL: &str = include_str!("gpu.wgsl");
pub const AUX_WORDS: usize = 1024;

pub fn build(d: &mr_core::DenseChunk) -> Vec<u32> {
    let mut rows = vec![0u32; 1024];
    for (i, b) in d.0.iter().enumerate() {
        rows[i >> 5] |= (b.is_solid() as u32) << (i & 31);
    }
    rows
}
