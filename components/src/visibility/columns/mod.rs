//! Visibility `columns` (Voxel Space, Comanche 1992; PLAN step 13d: was the voxelspace back
//! ends' code): per screen column, a march front to back over the heightmap storage
//! (`crate::storage::heightmap`), drawing spans with a y-buffer. Shading inline in the march
//! or deferred (`placement`: the march writes hit records, the common pass shades them).
//! CPU: [`cpu`]. GPU: [`GPU_WGSL`] (the root: the march, entry `main`, with its [`Camera`]
//! uniform) and, with shader def `DEFERRED`, [`DEFERRED_WGSL`] (the records and the view the
//! common deferred pass decodes them with).

use bytemuck::{Pod, Zeroable};

pub mod cpu;

pub const GPU_WGSL: &str = include_str!("gpu.wgsl");
pub const DEFERRED_WGSL: &str = include_str!("deferred.wgsl");

/// The GPU march's uniform (`Camera` in [`GPU_WGSL`], group 0 binding 0).
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
    pub _p0: u32,
    pub max_cells: u32,
}
