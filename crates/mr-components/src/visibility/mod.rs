//! Visibility strategies: how primary visibility is organised. `raycast` casts one ray per
//! pixel through the traversal kernel (`crate::traversal`); `faces` hands every face of the
//! geometry component to the draw stage (`crate::draw`); discovery (`quadtree`, `cluster`)
//! finds the visible faces with few rays and hands that set to the draw stage; `splat`,
//! `slices` (plane sweeping) and `columns` fill their own buffers. Discovery's code is here ([`discover`],
//! [`quadtree`], [`cluster`]; its hole detection in `crate::holes`), and splat's and columns'
//! ([`splat`] over `crate::geometry::points`, [`columns`]).
//!
//! Every strategy's product is one hit record per pixel: [`record`] on the
//! CPU, `pack_hit` / `record_surface` (`mr::voxels`) on the GPU, with the same layout; or an
//! id its source decodes (`visbuffer`'s face ids, `splat`'s point ids, raster-cpu's quad
//! ids). The common deferred pass (`crate::shading`) shades either; placement (inline or
//! deferred) is a choice wherever a strategy can do both: ray casting and columns
//! (`placement`), the face drawers (`raster` or `visbuffer` / `gbuffer`), raster-cpu
//! (`placement`, forward or deferred); splatting settles visibility only after every point,
//! so it is deferred by construction.

pub mod cluster;
pub mod columns;
pub mod discover;
pub mod quadtree;
pub mod record;
pub mod slices;
pub mod splat;

/// The layer walk on the GPU (transparency): the surfaces along a ray front to back through a
/// kernel's `ctrace` and its storage's `s_block_at` (`layer_walk`, `layer_next`). The
/// ray-casting root's primary rays and the shadow-ray service (`vox_transmit`) each carry it,
/// over their own kernel, compiled in only with `LAYERS`.
pub const LAYER_WALK_WGSL: &str = include_str!("layer_walk.wgsl");
