// Traversal `rayquery`: hardware ray queries against the acceleration structures the
// executor builds from the geometry node this kernel reads, through the geometry
// component's `g_rt_trace` (any GPU ray-casting site may be one). Ray
// queries cannot sit in a shader module (naga cannot write them back), so this kernel is
// root code: the primary rays' `ctrace` here, the ray services of other sites
// (`occluded`, `strace`) virtual functions the root overrides (`Walks::services`).

fn ctrace(o: vec3<f32>, d: vec3<f32>, max_t: f32) -> Hit {
    return g_rt_trace(o, d, max_t, false);
}
