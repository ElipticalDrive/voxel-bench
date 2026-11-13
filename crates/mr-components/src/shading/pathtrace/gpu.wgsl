// Shading `pathtrace` (from gpu-pathtrace): unlit = the surface's albedo; lit = diffuse path
// tracing: next-event estimation for sun and point lights with shadow rays (`occluded`),
// a constant-ambient sky, `sp.bounces` cosine-weighted bounces traced with the composed
// secondary walk (`strace`), `sp.spp` paths per pixel per frame, and progressive accumulation in a
// linear buffer (`accum`, `sp.prev_count` samples so far). `sp` and `accum` are the
// shading state (`shading::state`), bound by the back end. `bounces = 0` reproduces
// `direct`'s lit image.

var<private> pt_rng: u32;

fn pt_pcg(v: u32) -> u32 {
    let s = v * 747796405u + 2891336453u;
    let w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (w >> 22u) ^ w;
}

fn pt_rand() -> f32 {
    pt_rng = pt_pcg(pt_rng);
    return f32(pt_rng >> 8u) * (1.0 / 16777216.0);
}

fn pt_cosine_dir(n: vec3<f32>) -> vec3<f32> {
    let u1 = pt_rand();
    let u2 = pt_rand();
    let r = sqrt(u1);
    let phi = 6.2831853 * u2;
    let a = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), abs(n.x) > 0.5);
    let t = normalize(cross(n, a));
    let b = cross(n, t);
    return normalize(t * (r * cos(phi)) + b * (r * sin(phi)) + n * sqrt(max(0.0, 1.0 - u1)));
}

fn pt_linear(c: vec4<f32>) -> vec3<f32> {
    return vec3<f32>(srgb_to_linear(c.r), srgb_to_linear(c.g), srgb_to_linear(c.b));
}

fn pt_albedo(s: Surface) -> vec3<f32> {
    return pt_linear(surface_texel(s.block, s.face, s.local));
}

// Linear radiance (pre-exposure) of one path starting at the primary surface point `p0`
// with normal `n0` and linear albedo `albedo0` (a voxel face or a mesh); bounces land on
// voxel surfaces (`strace`, `surface_of`).
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
        let d = pt_cosine_dir(n);
        let h = strace(po, d, vp.max_dist);
        if (!h.hit) {
            radiance += thr * vp.ambient;
            break;
        }
        let s = surface_of(h, po, d);
        albedo = pt_albedo(s);
        p = s.p;
        n = s.n;
    }
    return radiance;
}

fn shade_point(albedo: vec4<f32>, p: vec3<f32>, n: vec3<f32>, d: vec3<f32>, lit: bool, px: vec2<u32>) -> vec4<f32> {
    if (!lit) { return albedo; }
    let idx = px.y * sp.width + px.x;
    pt_rng = pt_pcg(idx ^ pt_pcg(sp.frame * 0x9E3779B9u + 1u));
    let a = pt_linear(albedo);
    var sum = vec3<f32>(0.0);
    for (var i = 0u; i < sp.spp; i++) { sum += pt_path(a, p, n); }
    var mean = sum / f32(sp.spp);
    if (sp.prev_count > 0u) {
        mean = (accum[idx].rgb * f32(sp.prev_count) + sum) / f32(sp.prev_count + sp.spp);
    }
    accum[idx] = vec4<f32>(mean, 1.0);
    return display(mean * vp.exposure);
}

fn shade_surface(s: Surface, d: vec3<f32>, lit: bool, px: vec2<u32>) -> vec4<f32> {
    return shade_point(surface_texel(s.block, s.face, s.local), s.p, s.n, d, lit, px);
}
