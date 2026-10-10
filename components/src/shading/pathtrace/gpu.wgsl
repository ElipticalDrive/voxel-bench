// Shading `pathtrace` (from gpu-pathtrace): unlit = the face texel; lit = diffuse path
// tracing: next-event estimation for sun and point lights with shadow rays (`occluded`),
// a constant-ambient sky, `cam.bounces` cosine-weighted bounces traced with the composed
// walk (`ctrace`), `cam.spp` paths per pixel per frame, and progressive accumulation in a
// linear buffer (`accum`, `cam.prev_count` samples so far; the back end resets it when the
// view or the world changes). `bounces = 0` reproduces `direct`'s lit image.

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

// The face texel at hit `h` (LOD cells: textures tile once per block).
fn pt_texel(h: Hit, o: vec3<f32>, d: vec3<f32>) -> vec4<f32> {
    let face = face_entered(h.axis, d[h.axis] > 0.0);
    let p = o + d * h.t;
    var local = p - vec3<f32>(h.cell);
    if (h.lod > 0u) { local = p - floor(p); }
    return surface_texel(h.block, face, local);
}

fn pt_albedo(h: Hit, o: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    let texel = pt_texel(h, o, d);
    return vec3<f32>(srgb_to_linear(texel.r), srgb_to_linear(texel.g), srgb_to_linear(texel.b));
}

// Linear radiance (pre-exposure) of one path starting at primary hit `h0`.
fn pt_path(o0: vec3<f32>, d0: vec3<f32>, h0: Hit) -> vec3<f32> {
    var o = o0;
    var d = d0;
    var h = h0;
    var thr = vec3<f32>(1.0);
    var radiance = vec3<f32>(0.0);
    for (var depth = 0u; depth <= cam.bounces; depth++) {
        let n = face_normal(face_entered(h.axis, d[h.axis] > 0.0));
        let p = o + d * h.t + n * 1e-3;
        thr *= pt_albedo(h, o, d);
        var light = direct_light(p, n);
        // Ambient stands in for sky light: gathered by escaping bounce rays instead,
        // except at the last vertex where it terminates the path.
        if (depth < cam.bounces) { light -= vp.ambient; }
        radiance += thr * light;
        if (depth == cam.bounces) { break; }
        d = pt_cosine_dir(n);
        o = p;
        h = ctrace(o, d, vp.max_dist);
        if (!h.hit) {
            radiance += thr * vp.ambient;
            break;
        }
    }
    return radiance;
}

fn shade_hit(h: Hit, o: vec3<f32>, d: vec3<f32>, lit: bool, px: vec2<u32>) -> vec4<f32> {
    if (!lit) { return pt_texel(h, o, d); }
    let idx = px.y * cam.width + px.x;
    pt_rng = pt_pcg(idx ^ pt_pcg(cam.frame * 0x9E3779B9u + 1u));
    var sum = vec3<f32>(0.0);
    for (var s = 0u; s < cam.spp; s++) { sum += pt_path(o, d, h); }
    var mean = sum / f32(cam.spp);
    if (cam.prev_count > 0u) {
        mean = (accum[idx].rgb * f32(cam.prev_count) + sum) / f32(cam.prev_count + cam.spp);
    }
    accum[idx] = vec4<f32>(mean, 1.0);
    return encode_srgb(mean * vp.exposure);
}
