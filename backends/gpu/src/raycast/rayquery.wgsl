// The ray-query lane (gpu-rt): the primary ray is a hardware ray query (`ctrace` from the
// traversal kernel `rayquery` over the geometry component's acceleration structure).

@group(0) @binding(8) var acc: acceleration_structure;
@group(0) @binding(9) var<storage, read> geo: array<u32>;              // quads or brick lists
@group(0) @binding(10) var<storage, read> chunk_info: array<vec4<u32>>; // per instance slot

// What the primary ray `d` through pixel `px` hits (no meshes stage: `mt` is 1e30).
fn pixel_hit(px: vec2<u32>, d: vec3<f32>, mt: f32) -> Hit {
    return ctrace(cam.origin, d, vp.max_dist);
}
