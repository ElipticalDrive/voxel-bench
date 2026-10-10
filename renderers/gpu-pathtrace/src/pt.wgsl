// gpu-pathtrace: one compute thread per pixel; unlit = primary-hit texel, lit = diffuse
// path tracing (NEE sun/point lights with shadow rays, constant-ambient sky, cosine-weighted
// bounces) with progressive accumulation in a linear HDR buffer.

struct Camera {
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, mode: u32,      // 0 unlit, 1 lit
    origin: vec3<f32>, sky: u32,   // camera position relative to vp.cam_block; packed sRGB sky
    width: u32, height: u32, frame: u32, prev_count: u32,
    bounces: u32, spp: u32, _p0: u32, _p1: u32,
}

@group(0) @binding(0) var<uniform> cam: Camera;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<storage, read_write> accum: array<vec4<f32>>;

var<private> rng: u32;

fn pcg(v: u32) -> u32 {
    let s = v * 747796405u + 2891336453u;
    let w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (w >> 22u) ^ w;
}
fn rand() -> f32 {
    rng = pcg(rng);
    return f32(rng >> 8u) * (1.0 / 16777216.0);
}

fn cosine_dir(n: vec3<f32>) -> vec3<f32> {
    let u1 = rand();
    let u2 = rand();
    let r = sqrt(u1);
    let phi = 6.2831853 * u2;
    let a = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), abs(n.x) > 0.5);
    let t = normalize(cross(n, a));
    let b = cross(n, t);
    return normalize(t * (r * cos(phi)) + b * (r * sin(phi)) + n * sqrt(max(0.0, 1.0 - u1)));
}

fn albedo_at(h: Hit, o: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    let face = face_entered(h.axis, d[h.axis] > 0.0);
    let p = o + d * h.t;
    var local = p - vec3<f32>(h.cell);
    if (h.lod > 0u) { local = p - floor(p); }
    let texel = surface_texel(h.block, face, local);
    return vec3<f32>(srgb_to_linear(texel.r), srgb_to_linear(texel.g), srgb_to_linear(texel.b));
}

// Linear radiance (pre-exposure) of one path starting at primary hit `h0`.
fn path(o0: vec3<f32>, d0: vec3<f32>, h0: Hit) -> vec3<f32> {
    var o = o0;
    var d = d0;
    var h = h0;
    var thr = vec3<f32>(1.0);
    var radiance = vec3<f32>(0.0);
    for (var depth = 0u; depth <= cam.bounces; depth++) {
        let n = face_normal(face_entered(h.axis, d[h.axis] > 0.0));
        let p = o + d * h.t + n * 1e-3;
        thr *= albedo_at(h, o, d);
        var light = direct_light(p, n);
        // Ambient stands in for sky light: gathered by escaping bounce rays instead,
        // except at the last vertex where it terminates the path.
        if (depth < cam.bounces) { light -= vp.ambient; }
        radiance += thr * light;
        if (depth == cam.bounces) { break; }
        d = cosine_dir(n);
        o = p;
        h = trace(o, d, vp.max_dist);
        if (!h.hit) {
            radiance += thr * vp.ambient;
            break;
        }
    }
    return radiance;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width || gid.y >= cam.height) { return; }
    let sx = (2.0 * (f32(gid.x) + 0.5) / f32(cam.width) - 1.0) * cam.tan_half_fov * cam.aspect;
    let sy = (1.0 - 2.0 * (f32(gid.y) + 0.5) / f32(cam.height)) * cam.tan_half_fov;
    let d = normalize(cam.forward + sx * cam.right + sy * cam.up);
    let o = cam.origin;
    let h = trace(o, d, vp.max_dist);
    var color = unpack4x8unorm(cam.sky);
    if (h.hit) {
        if (cam.mode == 1u) {
            let idx = gid.y * cam.width + gid.x;
            rng = pcg(idx ^ pcg(cam.frame * 0x9E3779B9u + 1u));
            var sum = vec3<f32>(0.0);
            for (var s = 0u; s < cam.spp; s++) { sum += path(o, d, h); }
            var mean = sum / f32(cam.spp);
            if (cam.prev_count > 0u) {
                let total = f32(cam.prev_count + cam.spp);
                mean = (accum[idx].rgb * f32(cam.prev_count) + sum) / total;
            }
            accum[idx] = vec4<f32>(mean, 1.0);
            color = encode_srgb(mean * vp.exposure);
        } else {
            let face = face_entered(h.axis, d[h.axis] > 0.0);
            let p = o + d * h.t;
            var local = p - vec3<f32>(h.cell);
            if (h.lod > 0u) { local = p - floor(p); }
            color = surface_texel(h.block, face, local);
        }
    }
    textureStore(out_tex, vec2<i32>(gid.xy), color);
}
