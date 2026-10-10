// gpu-compose: one compute thread per pixel. `ctrace` comes from the chosen traversal
// module, which reaches the world only through the chosen storage module.

struct Camera {
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, mode: u32,      // 0 unlit, 1 lit
    origin: vec3<f32>, sky: u32,   // camera position relative to vp.cam_block; packed sRGB sky
    width: u32, height: u32, _p0: u32, _p1: u32,
}

@group(0) @binding(0) var<uniform> cam: Camera;
// Loop iterations of the current ray (traversals increment it), for `view = "steps"`.
var<private> ct_iters: u32;

@group(0) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width || gid.y >= cam.height) { return; }
    let sx = (2.0 * (f32(gid.x) + 0.5) / f32(cam.width) - 1.0) * cam.tan_half_fov * cam.aspect;
    let sy = (1.0 - 2.0 * (f32(gid.y) + 0.5) / f32(cam.height)) * cam.tan_half_fov;
    let d = normalize(cam.forward + sx * cam.right + sy * cam.up);
    let o = cam.origin;
    ct_iters = 0u;
    let h = ctrace(o, d, vp.max_dist);
    if (VIEW_STEPS) {
        // Primary-ray loop iterations: red = low 8 bits, green = high bits (exact counts
        // for analysis), blue = a log-scale brightness for viewing.
        let n = ct_iters;
        let v = log2(f32(n) + 1.0) / 11.0;
        textureStore(out_tex, vec2<i32>(gid.xy), vec4<f32>(f32(n & 255u) / 255.0, f32(min(n >> 8u, 255u)) / 255.0, v, 1.0));
        return;
    }
    var color = unpack4x8unorm(cam.sky);
    if (h.hit) {
        let face = face_entered(h.axis, d[h.axis] > 0.0);
        let p = o + d * h.t;
        let texel = surface_texel(h.block, face, p - vec3<f32>(h.cell));
        // A branch, not `select`: select evaluates both sides, so unlit pixels would pay for
        // lighting and shadow rays whenever the compiler doesn't turn it into a branch.
        color = texel;
        if (cam.mode == 1u) { color = shade_lit(texel, p, face_normal(face)); }
    }
    textureStore(out_tex, vec2<i32>(gid.xy), color);
}
