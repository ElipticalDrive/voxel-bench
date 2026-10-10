// Visibility `rayquery` (gpu-rt back end): hardware ray queries against the acceleration
// structure built from the geometry component, which supplies `g_rt_trace`. Primary,
// bounce and shadow rays all go through it.

fn ctrace(o: vec3<f32>, d: vec3<f32>, max_t: f32) -> Hit {
    ct_iters += 1u;
    return g_rt_trace(o, d, max_t, false);
}

fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool {
    return g_rt_trace(p, d, max_t, true).hit;
}
