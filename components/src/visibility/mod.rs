//! Visibility strategies: how primary visibility is organised. `raycast` casts one ray per
//! pixel through the traversal kernel (`crate::traversal`); `faces` hands every face of the
//! geometry component to the draw stage (`crate::draw`); discovery (`quadtree`, `cluster`)
//! finds the visible faces with few rays and hands that set to the draw stage; `splat` and
//! `columns` fill their own buffers. The strategies' code lives in their back ends for now.
//!
//! Every strategy's product is one hit record per pixel (PLAN step 12): [`record`] on the
//! CPU, `pack_hit` / `record_surface` (`mr::voxels`) on the GPU, with the same layout; or an
//! id its source decodes (`visbuffer`'s face ids, `splat`'s point ids, cpu-raster's quad
//! ids). The common deferred pass (`crate::shading`) shades either; placement (inline or
//! deferred) is a choice wherever a strategy can do both: ray casting and columns
//! (`placement`), the face drawers (`raster` or `visbuffer` / `gbuffer`), cpu-raster
//! (`placement`, forward or deferred); splatting settles visibility only after every point,
//! so it is deferred by construction.

pub mod record;
