//! Storage components: how the world is held and which queries it answers.
//!
//! GPU interface (WGSL; see IDEAS §12):
//! * required: `S_LEVELS` (levels k whose aligned 2^k cubes `s_occupied` answers),
//!   `s_occupied(slot, level, c)` (may say occupied for an empty cube, never the reverse;
//!   exact at level 0), `s_probe(slot, l) -> (block or 0, empty aligned cube size)`,
//!   `s_block(slot, l)`;
//! * optional fast paths (`OPTIONAL_QUERIES`): `s_dist(slot, l)` (Chebyshev distance to the
//!   nearest solid voxel), `s_row(slot, y, z)` (occupancy row along x). A storage lists the
//!   ones it implements; the assembler appends the generic fallbacks for the rest
//!   (`fallbacks.wgsl`), so every storage works with every visibility component.
//! * side data: `aux_words` u32 per pool slot in the `aux` buffer, built per dense chunk.

pub mod bits;
pub mod bricks;
pub mod cpu;
#[cfg(test)]
mod cpu_tests;
pub mod distance;
pub mod hier;
pub mod palette;
pub mod prefix;
pub mod rows;
pub mod u8;

use mr_core::DenseChunk;

/// Optional storage queries; each has a fallback in `fallbacks.wgsl` (`//@name`).
pub const OPTIONAL_QUERIES: &[&str] = &["s_dist", "s_row"];

/// Implementation details a GPU back end needs for a storage.
pub struct GpuStorage {
    pub wgsl: &'static str,
    pub provides: &'static [&'static str],
    pub aux_words: usize,
    pub build: fn(&DenseChunk) -> Vec<u32>,
}

pub fn gpu(name: &str) -> Option<GpuStorage> {
    Some(match name {
        "bricks" => GpuStorage { wgsl: bricks::GPU_WGSL, provides: &[], aux_words: bricks::AUX_WORDS, build: bricks::build },
        "hier" => GpuStorage { wgsl: hier::GPU_WGSL, provides: &[], aux_words: hier::AUX_WORDS, build: hier::build },
        "prefix" => GpuStorage { wgsl: prefix::GPU_WGSL, provides: &[], aux_words: prefix::AUX_WORDS, build: prefix::build },
        "distance" => GpuStorage { wgsl: distance::GPU_WGSL, provides: &["s_dist"], aux_words: distance::AUX_WORDS, build: distance::build },
        "rows" => GpuStorage { wgsl: rows::GPU_WGSL, provides: &["s_row"], aux_words: rows::AUX_WORDS, build: rows::build },
        _ => return None,
    })
}

/// The fallback WGSL of every optional query `provides` lacks.
pub fn fallbacks(provides: &[&str]) -> String {
    let src = include_str!("fallbacks.wgsl");
    let mut out = String::new();
    for q in OPTIONAL_QUERIES.iter().filter(|q| !provides.contains(q)) {
        let start = src.find(&format!("//@{q}\n")).expect("fallback missing");
        let rest = &src[start..];
        let end = rest[3..].find("//@").map_or(rest.len(), |e| e + 3);
        out += &rest[..end];
    }
    out
}
