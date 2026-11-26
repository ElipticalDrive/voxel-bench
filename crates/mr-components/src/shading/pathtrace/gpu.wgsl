// Shading `pathtrace` (from gpu-pathtrace): unlit = the surface's albedo; lit = diffuse path
// tracing: next-event estimation for sun and point lights with shadow rays (`occluded`),
// a constant-ambient sky, `sp.bounces` cosine-weighted bounces traced with the composed
// secondary walk (`strace`), `sp.spp` paths per pixel per frame, and progressive accumulation in a
// linear buffer (`accum`, `sp.prev_count` samples so far). `sp` and `accum` are the
// shading state (`shading::state`), bound by the back end. `bounces = 0` reproduces
// `direct`'s lit image.

fn pt_linear(c: vec4<f32>) -> vec3<f32> {
    return vec3<f32>(srgb_to_linear(c.r), srgb_to_linear(c.g), srgb_to_linear(c.b));
}

// Linear radiance (pre-exposure) of one path starting at the primary surface point `p0`
// with normal `n0` and linear albedo `albedo0` (a voxel face or a mesh); bounces land
// where the bounce rays' `btrace` says (voxel faces, and meshes where the program has them).
fn pt_path(albedo0: vec3<f32>, p0: vec3<f32>, n0: vec3<f32>) -> vec3<f32> {
    var albedo = albedo0;
    var p = p0;
    var n = n0;
    var thr = vec3<f32>(1.0);
    var radiance = vec3<f32>(0.0);
    for (var depth = 0u; depth <= sp.bounces; depth++) {
        let po = p + n * 1e-3;
        thr *= albedo;
        var light = direct_light(po, n);
        // Ambient stands in for sky light: gathered by escaping bounce rays instead,
        // except at the last vertex where it terminates the path.
        if (depth < sp.bounces) { light -= vp.ambient; }
        radiance += thr * light;
        if (depth == sp.bounces) { break; }
        let d = rng_cosine_dir(n);
        let s = btrace(po, d, vp.max_dist);
        if (!s.hit) {
            radiance += thr * vp.ambient;
            break;
        }
        albedo = pt_linear(s.albedo);
        p = s.p;
        n = s.n;
    }
    return radiance;
}

fn shade_point(albedo: vec4<f32>, p: vec3<f32>, n: vec3<f32>, d: vec3<f32>, lit: bool, px: vec2<u32>) -> vec4<f32> {
    if (!lit) { return albedo; }
    let idx = px.y * sp.width + px.x;
    rng_state = rng_pcg(idx ^ rng_pcg(sp.frame * 0x9E3779B9u + 1u));
    let a = pt_linear(albedo);
    var sum = vec3<f32>(0.0);
    for (var i = 0u; i < sp.spp; i++) { sum += pt_path(a, p, n); }
    // Per pixel: the mean so far (bits of linear rgb) and in w the primary surface's
    // signature (high bits) with the pixel's own sample count (low bits). A pixel whose
    // surface changed (a face set that changed under a still camera) starts again alone;
    // `prev_count == 0` (view, mode or world changed) restarts every pixel.
    let sig = surface_sig(p, n);
    let old = accum[idx];
    var count = 0u;
    if (sp.prev_count > 0u && (old.w & ACC_SIG_MASK) == sig) { count = old.w & ACC_COUNT_MASK; }
    var mean = sum / f32(sp.spp);
    if (count > 0u) {
        mean = (bitcast<vec3<f32>>(old.xyz) * f32(count) + sum) / f32(count + sp.spp);
    }
    accum[idx] = vec4<u32>(bitcast<vec3<u32>>(mean), sig | min(count + sp.spp, ACC_COUNT_MASK));
    return display(mean * vp.exposure);
}

fn shade_surface(s: Surface, d: vec3<f32>, lit: bool, px: vec2<u32>) -> vec4<f32> {
    return shade_point(surface_texel(s.block, s.face, s.local), s.p, s.n, d, lit, px);
}
