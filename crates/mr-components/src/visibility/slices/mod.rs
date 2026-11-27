//! Visibility `slices` (plane sweeping on the GPU rasteriser): every integer plane of the
//! loaded region along each axis is drawn as one quad, nearest to the camera first; the
//! fragment shader keeps a plane's pixel only where it holds a block face looking at the
//! camera (two lookups in the storage the node reads) and writes that face's hit record
//! behind the depth test; the common deferred pass shades the records. No geometry, no
//! meshing, no walk: the cost is the planes' fill (about three times the region's width in
//! planes, each up to the whole screen), bounded by the depth test. GPU only:
//! [`GPU_WGSL`] (the root: entries `vs`, `fs` and the deferred pass's view, entry `shade`
//! once `shading/deferred.wgsl` is appended). Its `Camera` uniform (group 0, binding 0) is the
//! GPU executor's with the plane count as the first region word; the records it reads in
//! the deferred pass at group 0, binding 8.

pub const GPU_WGSL: &str = include_str!("gpu.wgsl");

/// The planes drawn for a loaded region from `lo` to `hi` (blocks, `hi` exclusive): every
/// integer plane along each axis, both ends included.
pub fn planes(lo: [i32; 3], hi: [i32; 3]) -> u32 {
    (0..3).map(|a| (hi[a] - lo[a] + 1).max(0) as u32).sum()
}

#[cfg(test)]
mod tests {
    #[test]
    fn planes_count_both_ends() {
        assert_eq!(super::planes([0, 0, 0], [32, 64, 32]), 33 + 65 + 33);
        assert_eq!(super::planes([1, 1, 1], [0, 0, 0]), 0);
    }
}
