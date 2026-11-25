//! Renderer components: a renderer is a pipeline graph ([`graph`]) whose nodes are
//! component instances, each filling a role: `storage`, `geometry` (storage turned into
//! drawable faces), `prepass` (a stage before the walk, e.g. beams), `visibility` (the
//! strategy that organises primary visibility: ray casting, raster, discovery, splat,
//! columns), `holes`, `draw` (the faces a strategy hands on turned into the image: raster,
//! visibility buffer, G-buffer), `traversal` (the kernel that answers ray queries against a
//! storage, wherever rays are cast), `shading`, `meshes`, and the world's side around them.
//!
//! Lighting and meshes are stages *around* the walk, so storages and walks stay unaware of
//! them: the pass mode (lit / unlit, meshes on / off) is what the scene asks for, the
//! shading component realises lit and unlit (with the lighting knobs, e.g. `shadows`), and
//! the meshes component decides how meshes are drawn.
//!
//! This crate is the single source of truth for what exists and what combines: component
//! metadata ([`COMPONENTS`]), capabilities and adapters ([`caps`]), the graph model with
//! its validation and what each executor builds ([`graph`]), and the component
//! implementations themselves, one directory per piece (`storage/prefix/gpu.wgsl`, ...). The
//! executors (`crates/mr-exec-*`) assemble a working renderer from an accepted graph.

pub use caps::Cap;
pub use graph::Role;
pub use impls::{Api, ApiChoice, Impl, Kind, Place};
use impls::{ISAS, SHADER};
use storage::cpu::Repr;
use traversal::cpu::Kernel;

pub mod caps;
pub mod draw;
pub mod geometry;
pub mod graph;
pub mod holes;
pub mod impls;
pub mod post;
pub mod prepass;
pub mod shaders;
pub mod shading;
pub mod storage;
pub mod traversal;
pub mod visibility;

pub struct ComponentInfo {
    pub role: Role,
    pub name: &'static str,
    pub description: &'static str,
    /// Its implementations, per device kind and API, each with the optional capabilities
    /// it provides there ([`caps`]).
    pub impls: &'static [Impl],
    /// Capabilities it needs from the other stages: met natively, through an adapter, or the
    /// combination is invalid.
    pub needs: &'static [(Role, Cap)],
}

impl ComponentInfo {
    /// Its implementation that runs at `p`, if any.
    pub fn at(&self, p: &Place) -> Option<&'static Impl> {
        self.impls.iter().find(|i| p.takes(i))
    }

    /// Its implementation on device kind `k` (the first, for capabilities).
    pub fn on(&self, k: Kind) -> Option<&'static Impl> {
        self.impls.iter().find(|i| i.kind == k)
    }

    /// `gpu: compute/raster/rayquery; cpu: scalar/avx2/avx512`.
    pub fn impls_label(&self) -> String {
        self.impls
            .iter()
            .map(|i| i.label())
            .collect::<Vec<_>>()
            .join("; ")
    }
}

pub const COMPONENTS: &[ComponentInfo] = &[
    // Storage.
    ComponentInfo {
        role: Role::Storage,
        name: "bricks",
        description: "The shared pool: a u16 id per voxel and one bit per 4^3 brick (levels 1, 4). gpu-dda's data.",
        impls: &[
            Impl {
                kind: Kind::Gpu,
                apis: SHADER,
                provides: &[Cap::Voxels, Cap::Lod],
            },
            Impl {
                kind: Kind::Cpu,
                apis: ISAS,
                provides: <storage::bricks::cpu::U16 as Repr>::CAPS,
            },
        ],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Storage,
        name: "hier",
        description: "Bricks plus one bit per 8^3 region (levels 1, 4, 8), as cpu-simd-hier.",
        impls: &[
            Impl {
                kind: Kind::Gpu,
                apis: SHADER,
                provides: &[Cap::Voxels, Cap::Lod],
            },
            Impl {
                kind: Kind::Cpu,
                apis: ISAS,
                provides: <storage::hier::cpu::Hier as Repr>::CAPS,
            },
        ],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Storage,
        name: "prefix",
        description: "The user's structure: Morton-ordered 4^3 masks plus a popcount prefix; any aligned 2..16 cube's emptiness is one compare (levels 1-16).",
        impls: &[
            Impl {
                kind: Kind::Gpu,
                apis: SHADER,
                provides: &[Cap::Voxels, Cap::Lod],
            },
            Impl {
                kind: Kind::Cpu,
                apis: ISAS,
                provides: <storage::prefix::cpu::Prefix as Repr>::CAPS,
            },
        ],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Storage,
        name: "distance",
        description: "Per-voxel Chebyshev distance to the nearest solid voxel (from gpu-sdf). Fast `s_dist`.",
        impls: &[Impl {
            kind: Kind::Gpu,
            apis: SHADER,
            provides: &[Cap::Voxels, Cap::Dist, Cap::Lod],
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Storage,
        name: "rows",
        description: "Bricks plus 32x32 occupancy rows per chunk (from gpu-bitmask). Fast `s_row`.",
        impls: &[Impl {
            kind: Kind::Gpu,
            apis: SHADER,
            provides: &[Cap::Voxels, Cap::Row, Cap::Lod],
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Storage,
        name: "tree64",
        description: "Sparse 64-tree per 64^3 region (from gpu-tree64): 4x4x4 child masks, popcount-packed children, compressed leaves, uniform subtrees collapsed (levels 0, 2, 4, 6). Owns its data: no voxel pool, ~15% of its VRAM.",
        impls: &[Impl {
            kind: Kind::Gpu,
            apis: SHADER,
            provides: &[Cap::Voxels],
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Storage,
        name: "svo_dag",
        description: "Sparse voxel DAG (from gpu-svo-dag; Kampe et al. 2013): per-chunk binary octrees, identical subtrees shared across all chunks (~3x dedup on terrain). No voxel pool: the smallest GPU footprint.",
        impls: &[Impl {
            kind: Kind::Gpu,
            apis: SHADER,
            provides: &[Cap::Voxels],
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Storage,
        name: "clipmap",
        description: "Camera-centred toroidal 3D textures (from gpu-clipmap): block ids, 4^3 brick occupancy and slot tags, read with textureLoad only; no hash table, no voxel pool.",
        impls: &[Impl {
            kind: Kind::Gpu,
            apis: SHADER,
            provides: &[Cap::Voxels],
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Storage,
        name: "brickmajor",
        description: "bricks with each 4^3 brick's 64 ids contiguous (128 bytes; from cpu-simd-brickmajor): fewer L1/L2 misses per ray.",
        impls: &[Impl {
            kind: Kind::Cpu,
            apis: ISAS,
            provides: <storage::brickmajor::Cpu as Repr>::CAPS,
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Storage,
        name: "morton",
        description: "bricks with ids in Morton (Z-)order over the chunk (from cpu-simd-morton): bricks contiguous and neighbouring bricks close.",
        impls: &[Impl {
            kind: Kind::Cpu,
            apis: ISAS,
            provides: <storage::morton::Cpu as Repr>::CAPS,
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Storage,
        name: "heightmap",
        description: "Top solid block height and id per (x, z) column, in a ring around the camera (from voxelspace): 2.5D, no overhangs or caves.",
        impls: &[
            Impl {
                kind: Kind::Gpu,
                apis: &[Api::Compute],
                provides: &[Cap::Heights],
            },
            Impl {
                kind: Kind::Cpu,
                apis: &[Api::Scalar],
                provides: &[Cap::Heights],
            },
        ],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Storage,
        name: "u8",
        description: "Brick mask plus a u8 id per voxel: half of bricks' footprint (32 KiB per chunk; at most 256 block types).",
        impls: &[Impl {
            kind: Kind::Cpu,
            apis: ISAS,
            provides: <storage::u8::cpu::U8 as Repr>::CAPS,
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Storage,
        name: "bits",
        description: "One occupancy bit per voxel, brick-major (4 KiB per chunk); block ids read only at the hit.",
        impls: &[Impl {
            kind: Kind::Cpu,
            apis: ISAS,
            provides: <storage::bits::cpu::Bits as Repr>::CAPS,
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Storage,
        name: "palette",
        description: "Per-chunk palette with indices packed at 1/2/4/8/16 bits (7x smaller than bricks on terrain).",
        impls: &[Impl {
            kind: Kind::Cpu,
            apis: ISAS,
            provides: <storage::palette::cpu::Palette as Repr>::CAPS,
        }],
        needs: &[],
    },
    // Visibility.
    ComponentInfo {
        role: Role::Visibility,
        name: "raycast",
        description: "Ray casting: one primary ray per pixel through the traversal kernel, shaded at its hit. Walks (not ray queries): past see-through blocks the ray continues (layers composited front to back).",
        impls: &[
            Impl {
                kind: Kind::Gpu,
                apis: &[Api::Compute, Api::Raster],
                provides: &[Cap::Layers],
            },
            Impl {
                kind: Kind::Gpu,
                apis: &[Api::RayQuery],
                provides: &[],
            },
            Impl {
                kind: Kind::Cpu,
                apis: ISAS,
                provides: &[Cap::Layers],
            },
        ],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Visibility,
        name: "quadtree",
        description: "Discovery (the user's idea): an adaptive screen-space quadtree of corner rays; coherent tiles add every exposed face of the hit plane in their footprint to a face set kept across frames, which is rasterised. Knobs: tile, criterion, incremental, events, merge, order, ...; writes the post views tiles/rays/churn/overdraw/quads",
        impls: &[
            Impl {
                kind: Kind::Gpu,
                apis: &[Api::Compute],
                provides: &[],
            },
            Impl {
                kind: Kind::Cpu,
                apis: &[Api::Scalar],
                provides: &[],
            },
        ],
        needs: &[(Role::Storage, Cap::Voxels)],
    },
    ComponentInfo {
        role: Role::Visibility,
        name: "cluster",
        description: "Face-cluster discovery (the user's design; was cpu-cluster): seed rays on a coarse-to-fine pixel lattice; from each hit a flood fill over coplanar exposed faces confirmed by corner rays; confirmed faces cover their screen area. Knobs: seed_step, sky, views clusters/kinds.",
        impls: &[Impl {
            kind: Kind::Cpu,
            apis: &[Api::Scalar],
            provides: &[],
        }],
        needs: &[(Role::Storage, Cap::Voxels)],
    },
    ComponentInfo {
        role: Role::Visibility,
        name: "splat",
        description: "Point splatting in compute (from splat-gpu): each point atomicMin's, over its block's screen rectangle, the depth where each pixel's ray enters the block (points under 2 px also cover the pixels they miss), a second pass writes the winning id, a resolve pass ray-boxes the block. Points over max_splat px on a side are splatted by a workgroup each. Knob: max_splat.",
        impls: &[Impl {
            kind: Kind::Gpu,
            apis: &[Api::Compute],
            provides: &[],
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Visibility,
        name: "columns",
        description: "Voxel Space column march (Comanche, 1992): per screen column, front to back over the heightmap, drawing spans with a y-buffer.",
        impls: &[
            Impl {
                kind: Kind::Gpu,
                apis: &[Api::Compute],
                provides: &[],
            },
            Impl {
                kind: Kind::Cpu,
                apis: &[Api::Scalar],
                provides: &[],
            },
        ],
        needs: &[(Role::Storage, Cap::Heights)],
    },
    ComponentInfo {
        role: Role::Visibility,
        name: "faces",
        description: "Every face of the geometry component goes to the draw stage, which depth-tests them (rasterisation).",
        impls: &[
            Impl {
                kind: Kind::Gpu,
                apis: &[Api::Raster],
                provides: &[],
            },
            Impl {
                kind: Kind::Cpu,
                apis: &[Api::Scalar],
                provides: &[],
            },
        ],
        needs: &[],
    },
    // Holes (discovery's hole detection).
    ComponentInfo {
        role: Role::Holes,
        name: "none",
        description: "No hole detection: the sampler looks at the whole screen every frame (jittered lattice).",
        impls: &[Impl {
            kind: Kind::Gpu,
            apis: &[Api::Compute],
            provides: &[],
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Holes,
        name: "coverage",
        description: "Incremental discovery (was `incremental = true`): the drawer first draws last frame's faces at this camera; the sampler then looks only at tiles with uncovered pixels (not known sky), occlusion edges the camera's translation can reveal, edited blocks or changed chunks, and a round-robin refresh; faces covering a pixel count as seen. In the graph a loop: the drawer's faces iterate twice. Knobs: keep_unseen, refresh, parallax_px, reveal_tiles, events, still_holes.",
        impls: &[Impl {
            kind: Kind::Gpu,
            apis: &[Api::Compute],
            provides: &[],
        }],
        needs: &[],
    },
    // Draw (the face drawer).
    ComponentInfo {
        role: Role::Draw,
        name: "raster",
        description: "Depth-tested rasterisation of the faces; each fragment's pixel ray is intersected with the face plane and shaded. raster-gpu: knob depth_prepass (lit: shade once per pixel); discovery: knob order (depth or painter). raster-cpu: edge functions into a visibility buffer, then deferred shading; knobs tile_width, band_height.",
        impls: &[
            Impl {
                kind: Kind::Gpu,
                apis: &[Api::Raster],
                provides: &[],
            },
            Impl {
                kind: Kind::Cpu,
                apis: &[Api::Scalar],
                provides: &[],
            },
        ],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Draw,
        name: "gbuffer",
        description: "G-buffer (from gpu-hybrid): the raster pass writes the common hit record (distance, block, face); the common deferred pass rebuilds the surface and shades once per pixel. 8 bytes per pixel against visbuffer's 4, but no refetch.",
        impls: &[Impl {
            kind: Kind::Gpu,
            apis: &[Api::Raster],
            provides: &[Cap::MeshRays, Cap::Records],
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Draw,
        name: "visbuffer",
        description: "Visibility buffer: the raster pass writes only a face id and depth; a compute pass reconstructs the hit and shades once per pixel, independent of overdraw.",
        impls: &[Impl {
            kind: Kind::Gpu,
            apis: &[Api::Raster],
            provides: &[Cap::MeshRays, Cap::Records],
        }],
        needs: &[],
    },
    // Traversal kernels.
    ComponentInfo {
        role: Role::Traversal,
        name: "refloor",
        description: "Per-pixel DDA: ask the storage for the largest empty cube, step out of it, re-floor (gpu-dda's walk).",
        impls: &[
            Impl {
                kind: Kind::Gpu,
                apis: SHADER,
                provides: &[Cap::Lod, Cap::Steps, Cap::Layers],
            },
            Impl {
                kind: Kind::Cpu,
                apis: ISAS,
                provides: <traversal::refloor::cpu::Refloor as Kernel>::CAPS,
            },
        ],
        needs: &[(Role::Storage, Cap::Voxels)],
    },
    ComponentInfo {
        role: Role::Traversal,
        name: "incremental",
        description: "Hierarchical DDA keeping per-level exit times, moving between levels incrementally. Knobs: exact, skip2..16.",
        impls: &[
            Impl {
                kind: Kind::Gpu,
                apis: SHADER,
                provides: &[Cap::Steps, Cap::Layers],
            },
            Impl {
                kind: Kind::Cpu,
                apis: ISAS,
                provides: <traversal::incremental::cpu::Incremental as Kernel>::CAPS,
            },
        ],
        needs: &[(Role::Storage, Cap::Voxels)],
    },
    ComponentInfo {
        role: Role::Traversal,
        name: "sphere",
        description: "Chebyshev sphere tracing: leave the whole empty box around the cell in one step (from gpu-sdf). Fast with `distance`.",
        impls: &[Impl {
            kind: Kind::Gpu,
            apis: SHADER,
            provides: &[Cap::Steps, Cap::Layers],
        }],
        needs: &[(Role::Storage, Cap::Voxels), (Role::Storage, Cap::Dist)],
    },
    ComponentInfo {
        role: Role::Traversal,
        name: "rowrun",
        description: "Refloor plus bit-scan jumps across empty row runs when the ray runs along x (from gpu-bitmask). Fast with `rows`.",
        impls: &[Impl {
            kind: Kind::Gpu,
            apis: SHADER,
            provides: &[Cap::Steps, Cap::Layers],
        }],
        needs: &[(Role::Storage, Cap::Voxels), (Role::Storage, Cap::Row)],
    },
    ComponentInfo {
        role: Role::Traversal,
        name: "rayquery",
        description: "Hardware ray queries against the acceleration structure (RT cores) for primary, shadow and bounce rays. Knob: cull (triangle facing culled for primary rays).",
        impls: &[Impl {
            kind: Kind::Gpu,
            apis: &[Api::RayQuery],
            provides: &[],
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Traversal,
        name: "heightfield",
        description: "The heightmap's own ray march (shadow and bounce rays of the columns pipelines): a 2D DDA over its columns, chunk columns the ray passes above skipped in one step.",
        impls: &[
            Impl {
                kind: Kind::Gpu,
                apis: &[Api::Compute],
                provides: &[],
            },
            Impl {
                kind: Kind::Cpu,
                apis: &[Api::Scalar],
                provides: &[],
            },
        ],
        needs: &[(Role::Storage, Cap::Heights)],
    },
    // Prepass.
    ComponentInfo {
        role: Role::Prepass,
        name: "none",
        description: "No prepass: every ray walks from the camera.",
        impls: &[Impl {
            kind: Kind::Gpu,
            apis: &[Api::Compute],
            provides: &[Cap::Lod],
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Prepass,
        name: "beam",
        description: "Conservative per-tile beam (from gpu-beam): one cone per tile marches top cells; rays start where their tile's cone is provably empty. Exact. Knob: tile (pixels, 1 = off).",
        impls: &[Impl {
            kind: Kind::Gpu,
            apis: &[Api::Compute],
            provides: &[],
        }],
        needs: &[(Role::Storage, Cap::Voxels)],
    },
    // Geometry.
    ComponentInfo {
        role: Role::Geometry,
        name: "greedy",
        description: "Greedy quads (CPU, per chunk, incremental): coplanar exposed faces merged into rectangles (on rayquery-gpu: two triangles each). Knob: merge = block (same block only) or exposure (any block, looked up per fragment; raster only).",
        impls: &[Impl {
            kind: Kind::Cpu,
            apis: &[Api::Scalar],
            provides: &[],
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Geometry,
        name: "naive",
        description: "One quad per exposed block face, no merging (\"Minecraft-style\"). Knob: cull = false emits all six faces of every solid block.",
        impls: &[Impl {
            kind: Kind::Cpu,
            apis: &[Api::Scalar],
            provides: &[],
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Geometry,
        name: "points",
        description: "One point per block with an exposed face (from splat-gpu), rebuilt per chunk.",
        impls: &[Impl {
            kind: Kind::Cpu,
            apis: &[Api::Scalar],
            provides: &[],
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Geometry,
        name: "boxes",
        description: "One AABB per occupied 4^3 brick (from gpu-rtx-bricks): the RT cores find candidate boxes, a small software DDA through the voxel pool finds the voxel.",
        impls: &[Impl {
            kind: Kind::Cpu,
            apis: &[Api::Scalar],
            provides: &[],
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Geometry,
        name: "cubes",
        description: "One 36-vertex cube instance per surface block; the hardware back-face culls the hidden half. Knob: cull = false instances every solid block.",
        impls: &[Impl {
            kind: Kind::Cpu,
            apis: &[Api::Scalar],
            provides: &[],
        }],
        needs: &[],
    },
    // Shading.
    ComponentInfo {
        role: Role::Shading,
        name: "pathtrace",
        description: "Diffuse path tracing (from gpu-pathtrace): NEE for sun and point lights, cosine-weighted bounces through the composed walk, progressive accumulation while the view is still. bounces = 0 reproduces direct. Knobs: bounces, spp, accumulate.",
        impls: &[Impl {
            kind: Kind::Gpu,
            apis: SHADER,
            provides: &[Cap::Paths],
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Shading,
        name: "direct",
        description: "The reference lighting model: texel (unlit); ambient, sun and point lights with shadow rays through the same walk (lit).",
        impls: &[
            Impl {
                kind: Kind::Gpu,
                apis: SHADER,
                provides: &[Cap::Layers],
            },
            Impl {
                kind: Kind::Cpu,
                apis: ISAS,
                provides: &[Cap::Layers],
            },
        ],
        needs: &[],
    },
    // Meshes (mesh mode): how polygon meshes join the voxel image.
    ComponentInfo {
        role: Role::Meshes,
        name: "none",
        description: "Meshes are not drawn: voxels only. Benchmarks skip the composition in +meshes passes; the GUI labels it \"(no meshes)\".",
        impls: &[Impl {
            kind: Kind::Gpu,
            apis: &[Api::Raster],
            provides: &[],
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Meshes,
        name: "composite",
        description: "Meshes as a surface source: their hit records (the record's mesh capability) rasterised first into a record and depth target; each primary ray is cast only up to the mesh depth at its pixel, and where no voxel is nearer the pixel's surface is the mesh's, shaded where the walk shades (inline, or the deferred pass; voxel shadows in lit mode). Exact for primary visibility; capped rays trace less (mr_gpu::meshes::MeshRecords).",
        impls: &[Impl {
            kind: Kind::Gpu,
            apis: &[Api::Raster],
            provides: &[],
        }],
        needs: &[],
    },
    ComponentInfo {
        role: Role::Meshes,
        name: "depth",
        description: "Meshes drawn into the same depth buffer as the voxel faces: shaded forward with `raster`, written into the ids or records with `visibility` / `gbuffer` and shaded by the deferred pass (voxels shadow meshes). Exact (mr_gpu::meshes::GpuMeshes).",
        impls: &[Impl {
            kind: Kind::Gpu,
            apis: &[Api::Raster],
            provides: &[],
        }],
        needs: &[],
    },
];

/// A component of `role`.
/// Device kinds a component is refused on by its nature, each with the reason (shown by
/// `accept` and the audit). A kind without an implementation and not listed here is a gap
/// not yet filled.
pub const REFUSED: &[(Role, &str, Kind, &str)] = &[(
    Role::Traversal,
    "rayquery",
    Kind::Cpu,
    "it is the GPU's ray-tracing API (RT cores walking a hardware acceleration structure); a CPU BVH walk would be a traversal of its own",
)];

/// Why component `name` of `role` is refused on device kind `k`, if it is by nature.
pub fn refused(role: Role, name: &str, k: Kind) -> Option<&'static str> {
    REFUSED
        .iter()
        .find(|(r, n, kind, _)| *r == role && *n == name && *kind == k)
        .map(|(.., why)| *why)
}

pub fn component(role: Role, name: &str) -> Option<&'static ComponentInfo> {
    registry().find(|c| c.role == role && c.name == name)
}

pub fn components(role: Role) -> impl Iterator<Item = &'static ComponentInfo> {
    registry().filter(move |c| c.role == role)
}

/// Every registered component: [`COMPONENTS`], and with the `test-hooks` feature those a
/// test registered on its own thread (`register_on_this_thread`).
fn registry() -> impl Iterator<Item = &'static ComponentInfo> {
    #[cfg(feature = "test-hooks")]
    let added = ADDED.with(std::cell::Cell::get);
    #[cfg(not(feature = "test-hooks"))]
    let added: &[ComponentInfo] = &[];
    COMPONENTS.iter().chain(added)
}

#[cfg(feature = "test-hooks")]
thread_local! {
    static ADDED: std::cell::Cell<&'static [ComponentInfo]> = const { std::cell::Cell::new(&[]) };
}

/// Adds `components` to the registry for the calling thread only (other threads, and so
/// other tests, never see them): how a test checks that registering a component is all it
/// takes for every listing to offer it. Test builds only (the `test-hooks` feature, which
/// the host enables as a dev-dependency).
#[cfg(feature = "test-hooks")]
pub fn register_on_this_thread(components: &'static [ComponentInfo]) {
    ADDED.with(|a| a.set(components));
}

/// Parses a placement value: `true` for deferred.
pub fn parse_placement(s: &str) -> anyhow::Result<bool> {
    match s {
        "inline" => Ok(false),
        "deferred" => Ok(true),
        _ => anyhow::bail!("placement must be `inline` or `deferred`, got `{s}`"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Metadata and implementations agree: every component a node of a shipped graph may be
    /// (`Graph::choices`) has code where that graph's executor runs it, every storage's
    /// `provides` matches its implementation, and every component runs in some graph.
    #[test]
    fn registry_is_consistent() {
        use graph::{SHIPPED, shipped};
        let mut seen = std::collections::BTreeSet::new();
        let mut checked = 0;
        for (file, _) in SHIPPED {
            let g = shipped(file).unwrap();
            let region = g.region().unwrap().id;
            let ck = g.check().unwrap();
            for (id, n) in g.nodes.iter().filter(|(_, n)| n.role.registered()) {
                let choices = g.choices(id);
                assert!(
                    choices.contains(&n.component.as_str()),
                    "{file} `{id}`: may be what it is"
                );
                let gpu = ck.devices[id.as_str()].kind() == Kind::Gpu;
                for c in choices {
                    seen.insert((n.role, c));
                    checked += 1;
                    use graph::RegionId::*;
                    let ok = match (region, n.role) {
                        (GpuWalk, Role::Storage) => storage::gpu(c).is_some_and(|g| {
                            g.has_lod() == matches!(g.kind, storage::Kind::Pool { .. })
                        }),
                        (GpuWalk | GpuRayQuery, Role::Visibility) => c == "raycast",
                        (GpuWalk, Role::Traversal) | (GpuMeshing | GpuSampler, Role::Traversal) => {
                            traversal::gpu_wgsl(c).is_some()
                        }
                        (GpuWalk, Role::Prepass) => prepass::gpu_wgsl(c).is_some(),
                        (GpuWalk, Role::Meshes) => c == "composite",
                        (GpuMeshing, Role::Meshes) => c == "depth",
                        (GpuMeshing | GpuSampler, Role::Storage) => storage::gpu(c).is_some(),
                        (GpuMeshing, Role::Geometry) => geometry::gpu(c).is_some(),
                        (GpuMeshing | CpuRaster, Role::Visibility) => c == "faces",
                        (GpuMeshing | GpuSampler | CpuSampler, Role::Draw) => {
                            draw::gpu_wgsl(c).is_some()
                        }
                        (GpuSampler, Role::Visibility) => c == "quadtree",
                        (GpuSampler, Role::Holes) => c == "coverage",
                        // Every CPU storage (checked by `grid::CpuWorld::new`); its discovery rays walk with refloor.
                        (CpuSampler, Role::Storage) => gpu && storage::gpu(c).is_some() || !gpu,
                        (CpuSampler, Role::Traversal) if gpu => {
                            traversal::gpu_wgsl(c).is_some() || traversal::rt_wgsl(c).is_some()
                        }
                        (CpuSampler, Role::Visibility) => ["quadtree", "cluster"].contains(&c),
                        (CpuSampler, Role::Traversal) => c == "refloor",
                        (CpuRaster, Role::Geometry) => geometry::gpu(c).is_some_and(|g| g.per_face),
                        (CpuRaster, Role::Draw) => c == "raster",
                        (CpuRaster | CpuRaycast | CpuColumns, Role::Shading) => c == "direct",
                        (GpuRayQuery, Role::Geometry) => geometry::rt(c).is_some(),
                        (GpuRayQuery, Role::Traversal) => traversal::rt_wgsl(c).is_some(),
                        (_, Role::Shading) => shading::gpu_wgsl(c).is_some(),
                        (CpuRaycast | CpuRaster, Role::Storage) => component(Role::Storage, c)
                            .and_then(|x| x.on(Kind::Cpu))
                            .is_some(),
                        (CpuRaycast | CpuRaster, Role::Traversal) => {
                            ["refloor", "incremental"].contains(&c)
                        }
                        (CpuRaycast, Role::Visibility) => c == "raycast",
                        (GpuSplat, Role::Geometry) => c == "points",
                        (GpuSplat, Role::Visibility) => c == "splat",
                        (GpuSplat, Role::Storage) => storage::gpu(c).is_some(),
                        (GpuSplat, Role::Traversal) => traversal::gpu_wgsl(c).is_some(),
                        (GpuColumns | CpuColumns, Role::Storage) => c == "heightmap",
                        (GpuColumns | CpuColumns, Role::Traversal) => c == "heightfield",
                        (GpuColumns | CpuColumns, Role::Visibility) => c == "columns",
                        _ => false,
                    };
                    assert!(ok, "{file} `{id}` ({}) `{c}`", n.role.name());
                }
            }
        }
        assert!(checked > 100, "{checked}");
        // Every component runs in some shipped graph's node, and declares implementations;
        // prepass and holes are nodes the files leave out (`none` = no node).
        for c in COMPONENTS
            .iter()
            .filter(|c| c.name != "none" && !matches!(c.role, Role::Prepass | Role::Holes))
        {
            assert!(
                !c.impls.is_empty() && seen.contains(&(c.role, c.name)),
                "{} `{}`",
                c.role.name(),
                c.name
            );
        }
        // CPU implementations' capabilities are their associated consts.
        assert_eq!(
            component(Role::Storage, "prefix")
                .unwrap()
                .on(Kind::Cpu)
                .unwrap()
                .provides,
            <storage::prefix::cpu::Prefix as Repr>::CAPS
        );
        assert_eq!(
            component(Role::Traversal, "refloor")
                .unwrap()
                .on(Kind::Cpu)
                .unwrap()
                .provides,
            &[Cap::Steps, Cap::Layers]
        );
        assert!(
            !caps::provides(Kind::Cpu, Role::Storage, "bricks", Cap::Lod)
                && caps::provides(Kind::Gpu, Role::Storage, "bricks", Cap::Lod),
            "LOD is the GPU implementation's"
        );
        // Hit record fields: every walk counts steps; ray queries do not.
        assert!(!caps::provides(
            Kind::Gpu,
            Role::Traversal,
            "rayquery",
            Cap::Layers
        ));
        for v in graph::GPU_WALK
            .declared(Role::Traversal)
            .into_iter()
            .filter(|k| *k != "rayquery")
        {
            assert!(
                caps::provides(Kind::Gpu, Role::Traversal, v, Cap::Steps),
                "{v}"
            );
            // Every walk resumes from any cell (layers); ray queries stop at the first surface.
            assert!(
                caps::provides(Kind::Gpu, Role::Traversal, v, Cap::Layers),
                "{v}"
            );
        }
        assert!(!caps::provides(
            Kind::Gpu,
            Role::Traversal,
            "rayquery",
            Cap::Steps
        ));
    }
}
