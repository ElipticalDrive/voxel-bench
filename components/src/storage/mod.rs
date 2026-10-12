//! Storage components: how the world is held and which queries it answers.
//!
//! GPU interface (WGSL; see IDEAS §12). Walks work in absolute cell coordinates and reach
//! the storage only through:
//! * the top level: `S_TOP` (log2 edge of the storage's top cell: 5 for 32^3 chunks, 6 for
//!   tree64's 64^3 regions) and `s_top(key) -> handle` for the top cell `abs >> S_TOP`:
//!   `EMPTY`, `UNIFORM_BIT | block` (uniform solid) or a handle for the queries below.
//!   Pool storages get both from `pool_top.wgsl`, region storages from `region_top.wgsl`.
//!   LOD (`s_top_lod(key, lod)`, cells 2^lod blocks wide) exists for pool storages only
//!   (`S_HAS_LOD`); the others answer level 0.
//! * required queries, `l` local to the top cell: `S_LEVELS` (levels k whose aligned 2^k
//!   cubes `s_occupied` answers), `s_occupied(h, level, c)` (may say occupied for an empty
//!   cube, never the reverse; exact at level 0), `s_probe(h, l) -> (block or 0, empty
//!   aligned cube size)`, `s_block(h, l)`;
//! * optional capabilities (`crate::caps`): `s_dist(h, l)` (Chebyshev distance to the
//!   nearest solid voxel), `s_row(h, x0, y, z)` (occupancy of the 32 cells from local x0
//!   along x), LOD. A storage's registry entry lists the ones it implements; the assembler
//!   appends the adapters for the rest (`adapters.wgsl`), so every storage works with every
//!   visibility component.
//!
//! Host side (`Kind`, managed by `gpu_data::StorageData`): pool storages read the shared
//! voxel pool plus `aux_words` u32 of side data per pool slot; region storages own a blob
//! of words per region in `aux`, found through the `regions` table; the DAG storage keeps
//! one global hash-consed node pool in `aux` with chunk roots in the `regions` table; the
//! clipmap keeps toroidal 3D textures (group 2 bindings 4-7) and its own `s_top`.

pub mod bits;
pub mod brickmajor;
pub mod bricks;
pub mod clipmap;
pub mod cpu;
#[cfg(test)]
mod cpu_tests;
pub mod distance;
pub mod gpu_data;
pub mod heightmap;
pub mod hier;
pub mod layout;
pub mod morton;
pub mod palette;
pub mod prefix;
pub mod rows;
pub mod svo_dag;
pub mod tree64;
pub mod u8;

use mr_core::DenseChunk;

use crate::Cap;

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
    /// `svo_dag`: one global DAG shared by all chunks (top cell = chunk).
    Dag,
    /// `clipmap`: toroidal 3D textures around the camera (own `s_top`, bindings 4-7).
    Clipmap,
}

/// Implementation details a GPU back end needs for a storage.
#[derive(Copy, Clone)]
pub struct GpuStorage {
    pub wgsl: &'static str,
    /// Its capabilities, from the registry (`ComponentInfo::provides`).
    pub provides: &'static [Cap],
    pub kind: Kind,
}

impl GpuStorage {
    /// Whether the storage holds LOD chunks (pool storages).
    pub fn has_lod(&self) -> bool {
        self.provides.contains(&Cap::Lod)
    }

    /// The storage's WGSL with its top level and the adapters it needs. Expects the back
    /// end to bind `aux` and `regions` (group 2, `gpu_data::layout`).
    pub fn source(&self) -> String {
        const NO_LOD: &str = "const S_HAS_LOD: bool = false;\nfn s_top_lod(key: vec3<i32>, lod: u32) -> u32 { return s_top(key); }\n";
        let (top, aux_words) = match self.kind {
            Kind::Pool { aux_words, .. } => (include_str!("pool_top.wgsl").to_string(), aux_words),
            Kind::Region { .. } | Kind::Dag => (format!("{}{NO_LOD}", include_str!("region_top.wgsl")), 0),
            Kind::Clipmap => (NO_LOD.to_string(), 0),
        };
        format!(
            "@group(2) @binding(0) var<storage, read> aux: array<u32>;\n\
             @group(2) @binding(1) var<storage, read> regions: array<vec4<u32>>;\n\
             @group(2) @binding(2) var<storage, read> aux4: array<vec4<u32>>;\n\
             @group(2) @binding(3) var<uniform> region_head: vec4<u32>;\n\
             const AUX_WORDS: u32 = {}u;\n{top}\n{}\n{}\n{BLOCK_AT}",
            aux_words.max(1),
            self.wgsl,
            adapters(self.provides)
        )
    }
}

fn pool(wgsl: &'static str, aux_words: usize, build: fn(&DenseChunk) -> Vec<u32>) -> (&'static str, Kind) {
    (wgsl, Kind::Pool { aux_words, build })
}

pub fn gpu(name: &str) -> Option<GpuStorage> {
    let (wgsl, kind) = match name {
        "bricks" => pool(bricks::GPU_WGSL, bricks::AUX_WORDS, bricks::build),
        "hier" => pool(hier::GPU_WGSL, hier::AUX_WORDS, hier::build),
        "prefix" => pool(prefix::GPU_WGSL, prefix::AUX_WORDS, prefix::build),
        "distance" => pool(distance::GPU_WGSL, distance::AUX_WORDS, distance::build),
        "rows" => pool(rows::GPU_WGSL, rows::AUX_WORDS, rows::build),
        "clipmap" => (clipmap::GPU_WGSL, Kind::Clipmap),
        "svo_dag" => (svo_dag::GPU_WGSL, Kind::Dag),
        "tree64" => (tree64::GPU_WGSL, Kind::Region { shift: 6, build: tree64::build }),
        _ => return None,
    };
    let provides = crate::component(crate::Slot::Storage, name).map_or(&[][..], |c| c.provides);
    Some(GpuStorage { wgsl, provides, kind })
}

/// Derived from the required interface, for every storage: the block at absolute cell `a`
/// (0 = air or not loaded).
const BLOCK_AT: &str = "fn s_block_at(a: vec3<i32>) -> u32 {
    let h = s_top(a >> vec3<u32>(S_TOP));
    if (h == EMPTY) { return 0u; }
    if ((h & UNIFORM_BIT) != 0u) { return h & 0xffffu; }
    return s_block(h, a & vec3<i32>((1 << S_TOP) - 1));
}
";

/// The adapter WGSL of every storage capability `provides` lacks.
pub fn adapters(provides: &[Cap]) -> String {
    let src = include_str!("adapters.wgsl");
    let mut out = String::new();
    for a in crate::caps::ADAPTERS.iter().filter(|a| a.slot == crate::Slot::Storage && !provides.contains(&a.cap)) {
        let q = a.cap.name();
        let start = src.find(&format!("//@{q}\n")).expect("adapter missing");
        let rest = &src[start..];
        let end = rest[3..].find("//@").map_or(rest.len(), |e| e + 3);
        out += &rest[..end];
    }
    out
}
