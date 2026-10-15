// Traversal `rayquery` (gpu-rt back end): hardware ray queries against the acceleration
// structure built from the geometry component, which supplies `g_rt_trace`. Primary,
// bounce and shadow rays all go through it. Ray queries cannot sit in a shader module (naga
// cannot write them back), so this kernel is part of the root: it overrides the ray
// services of the program (`rt::rays`, virtual functions; PLAN step 9b) that lighting and
// shading call.

fn ctrace(o: vec3<f32>, d: vec3<f32>, max_t: f32) -> Hit {
    return g_rt_trace(o, d, max_t, false);
}

override fn rt::rays::occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool {
    return g_rt_trace(p, d, max_t, true).hit;
}

override fn rt::rays::strace(o: vec3<f32>, d: vec3<f32>, max_t: f32) -> Hit { return ctrace(o, d, max_t); }
