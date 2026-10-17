//! Visibility `columns` (Voxel Space, Comanche 1992; PLAN step 13d: was the voxelspace back
//! ends' code): per screen column, a march front to back over the heightmap storage
//! (`crate::storage::heightmap`), drawing spans with a y-buffer. Shading inline in the march
//! or deferred (`placement`: the march writes hit records, the common pass shades them).
//! CPU: [`cpu`]. GPU: [`GPU_WGSL`] (the root: the march, entry `main`; its `Camera` uniform,
//! group 0 binding 0, is the GPU executor's with `max_cells` as the last word) and, with
//! shader def `DEFERRED`, [`DEFERRED_WGSL`] (the records at group 0 binding 4 and the view
//! the common deferred pass decodes them with).

pub mod cpu;

pub const GPU_WGSL: &str = include_str!("gpu.wgsl");
pub const DEFERRED_WGSL: &str = include_str!("deferred.wgsl");
