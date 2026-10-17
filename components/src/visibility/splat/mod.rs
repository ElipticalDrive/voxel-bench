//! Visibility `splat` (point splatting in compute; PLAN step 13d: was gpu-splat's code): each
//! point of geometry `points` (`crate::geometry::points`) is projected and `atomicMin`s its
//! view depth over a square footprint sized from its projected size (capped at `max_splat`
//! px), a second pass writes the winning point id, and the common deferred pass is the
//! resolve: it decodes the id (ray-boxes the pixel against the winning block) and shades.
//! GPU only: [`GPU_WGSL`] (the root: entries `clear`, `splat_depth`, `splat_id`, and the
//! record encoding of the deferred pass, entry `shade`). Its `Camera` uniform (group 0,
//! binding 0) is the GPU executor's with `count` and `max_splat` as the last two words; its
//! buffers at group 0, bindings 8-10 (points, depth, ids).

pub const GPU_WGSL: &str = include_str!("gpu.wgsl");

/// The entry points in dispatch order: clear (per pixel), the two splats (per point), the
/// resolve (per pixel).
pub const ENTRIES: [&str; 4] = ["clear", "splat_depth", "splat_id", "shade"];
