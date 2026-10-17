//! The quadtree sampler (the user's discovery, IDEAS §3): rays at the corners of screen
//! tiles; a coherent tile adds the exposed faces of its plane in its footprint to the face
//! set, an incoherent one splits. CPU: [`cpu`]. GPU: [`GPU_WGSL`] (the sampler's root:
//! survivors, rays, classification, footprints, audit, painter sort, row merge) over
//! [`SET_WGSL`] (module [`SET`]: the face set, the lattice and the buffers it shares with
//! hole detection, `crate::holes`).

pub mod cpu;

/// Module path of the GPU face set and discovery buffers.
pub const SET: &str = "quadtree::set";
pub const SET_WGSL: &str = include_str!("set.wgsl");
pub const GPU_WGSL: &str = include_str!("gpu.wgsl");
