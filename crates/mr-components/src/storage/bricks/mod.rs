//! Storage `bricks`: the shared GPU world as is: a u16 id per voxel and one occupancy bit
//! per 4^3 brick (gpu-dda's data). No side data.

pub mod cpu;

pub const GPU_WGSL: &str = include_str!("gpu.wgsl");
pub const AUX_WORDS: usize = 0;

pub fn build(_: &mr_core::DenseChunk) -> Vec<u32> {
    Vec::new()
}
