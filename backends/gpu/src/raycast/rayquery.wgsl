// The ray-query lane (gpu-rt): the primary ray is a hardware ray query (`ctrace` from the
// traversal kernel `rayquery` over the core's acceleration structures, whose bindings and
// trace are the program's root code).

// What the primary ray `d` through pixel `px` hits (no meshes stage: `mt` is 1e30).
fn pixel_hit(px: vec2<u32>, d: vec3<f32>, mt: f32) -> Hit {
    return ctrace(cam.origin, d, vp.max_dist);
}
