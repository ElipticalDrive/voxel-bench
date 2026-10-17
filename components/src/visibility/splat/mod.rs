//! Visibility `splat` (point splatting in compute; PLAN step 13d: was gpu-splat's code): each
//! point of geometry `points` (`crate::geometry::points`) is projected and `atomicMin`s its
//! view depth over a square footprint sized from its projected size (capped at `max_splat`
//! px), a second pass writes the winning point id, and the common deferred pass is the
//! resolve: it decodes the id (ray-boxes the pixel against the winning block) and shades.
//! GPU only: [`GPU_WGSL`] (the root: entries `clear`, `splat_depth`, `splat_id`, and the
//! record encoding of the deferred pass, entry `shade`) with its [`Camera`] uniform.

use bytemuck::{Pod, Zeroable};

pub const GPU_WGSL: &str = include_str!("gpu.wgsl");

/// The entry points in dispatch order: clear (per pixel), the two splats (per point), the
/// resolve (per pixel).
pub const ENTRIES: [&str; 4] = ["clear", "splat_depth", "splat_id", "shade"];

/// The uniform (`Camera` in [`GPU_WGSL`], group 0 binding 0).
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Camera {
    pub forward: [f32; 3],
    pub tan_half_fov: f32,
    pub right: [f32; 3],
    pub aspect: f32,
    pub up: [f32; 3],
    pub mode: u32,
    pub origin: [f32; 3],
    pub sky: u32,
    pub width: u32,
    pub height: u32,
    pub count: u32,
    pub max_splat: u32,
}
