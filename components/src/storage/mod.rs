//! Storage components: how the world is held and which queries it answers.
//!
//! GPU interface (WGSL; see IDEAS §12). Walks work in absolute cell coordinates and reach
//! the storage only through:
//! * the top level: `S_TOP` (log2 edge of the storage's top cell: 5 for 32^3 chunks, 6 for
//!   tree64's 64^3 regions) and `s_top(key) -> handle` for the top cell `abs >> S_TOP`:
//!   `EMPTY`, `UNIFORM_BIT | block` (uniform solid) or a handle for the queries below.
//!   Pool storages get both from `pool_top.wgsl`, region storages from `region_top.wgsl`.
//! * required queries, `l` local to the top cell: `S_LEVELS` (levels k whose aligned 2^k
//!   cubes `s_occupied` answers), `s_occupied(h, level, c)` (may say occupied for an empty
//!   cube, never the reverse; exact at level 0), `s_probe(h, l) -> (block or 0, empty
//!   aligned cube size)`, `s_block(h, l)`;
//! * optional fast paths (`OPTIONAL_QUERIES`): `s_dist(h, l)` (Chebyshev distance to the
//!   nearest solid voxel), `s_row(h, x0, y, z)` (occupancy of the 32 cells from local x0
//!   along x). A storage lists the ones it implements; the assembler appends the generic
//!   fallbacks for the rest (`fallbacks.wgsl`), so every storage works with every
//!   visibility component.
//!
//! Host side (`Kind`, managed by `gpu_data::StorageData`): pool storages read the shared
//! voxel pool plus `aux_words` u32 of side data per pool slot; region storages own a blob
//! of words per region in `aux`, found through the `regions` table.

pub mod bits;
pub mod bricks;
pub mod cpu;
#[cfg(test)]
mod cpu_tests;
pub mod distance;
pub mod gpu_data;
pub mod hier;
pub mod palette;
pub mod prefix;
pub mod rows;
pub mod tree64;
pub mod u8;

use mr_core::DenseChunk;

/// Optional storage queries; each has a fallback in `fallbacks.wgsl` (`//@name`).
pub const OPTIONAL_QUERIES: &[&str] = &["s_dist", "s_row"];

/// What a region storage builds for one region (`get` reads its blocks, local coords).
pub enum RegionBlob {
    Empty,
    Uniform(u16),
    Words(Vec<u32>),
}

/// How a GPU storage's data is held (see the module docs).
#[derive(Copy, Clone)]
pub enum Kind {
    Pool { aux_words: usize, build: fn(&DenseChunk) -> Vec<u32> },
    Region { shift: u32, build: fn(&dyn Fn(glam::IVec3) -> mr_core::BlockId) -> RegionBlob },
}

/// Implementation details a GPU back end needs for a storage.
#[derive(Copy, Clone)]
pub struct GpuStorage {
    pub wgsl: &'static str,
    pub provides: &'static [&'static str],
    pub kind: Kind,
}

impl GpuStorage {
    /// The storage's WGSL with its top level and the fallbacks it needs. Expects the back
    /// end to bind `aux` and `regions` (group 2, `gpu_data::layout`).
    pub fn source(&self) -> String {
        let (top, aux_words) = match self.kind {
            Kind::Pool { aux_words, .. } => (include_str!("pool_top.wgsl"), aux_words),
            Kind::Region { .. } => (include_str!("region_top.wgsl"), 0),
        };
        format!(
            "@group(2) @binding(0) var<storage, read> aux: array<u32>;\n\
             @group(2) @binding(1) var<storage, read> regions: array<vec4<u32>>;\n\
             @group(2) @binding(2) var<storage, read> aux4: array<vec4<u32>>;\n\
             const AUX_WORDS: u32 = {}u;\n{top}\n{}\n{}",
            aux_words.max(1),
            self.wgsl,
            fallbacks(self.provides)
        )
    }
}

fn pool(wgsl: &'static str, provides: &'static [&'static str], aux_words: usize, build: fn(&DenseChunk) -> Vec<u32>) -> GpuStorage {
    GpuStorage { wgsl, provides, kind: Kind::Pool { aux_words, build } }
}

pub fn gpu(name: &str) -> Option<GpuStorage> {
    Some(match name {
        "bricks" => pool(bricks::GPU_WGSL, &[], bricks::AUX_WORDS, bricks::build),
        "hier" => pool(hier::GPU_WGSL, &[], hier::AUX_WORDS, hier::build),
        "prefix" => pool(prefix::GPU_WGSL, &[], prefix::AUX_WORDS, prefix::build),
        "distance" => pool(distance::GPU_WGSL, &["s_dist"], distance::AUX_WORDS, distance::build),
        "rows" => pool(rows::GPU_WGSL, &["s_row"], rows::AUX_WORDS, rows::build),
        "tree64" => GpuStorage { wgsl: tree64::GPU_WGSL, provides: &[], kind: Kind::Region { shift: 6, build: tree64::build } },
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
