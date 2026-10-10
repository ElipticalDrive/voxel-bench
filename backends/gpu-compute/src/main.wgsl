// GPU-compute back end: one compute thread per pixel. `ctrace` comes from the chosen
// visibility component, which reaches the world only through the chosen storage
// component; `shade_hit` comes from the chosen shading component.

struct Camera {
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, mode: u32,      // 0 unlit, 1 lit
    origin: vec3<f32>, sky: u32,   // camera position relative to vp.cam_block; packed sRGB sky
    width: u32, height: u32, frame: u32, prev_count: u32,   // frame counter; samples accumulated so far
    bounces: u32, spp: u32, _p0: u32, _p1: u32,             // pathtrace shading
}

@group(0) @binding(0) var<uniform> cam: Camera;
// Loop iterations of the current ray (traversals increment it), for `view = "steps"`.
var<private> ct_iters: u32;

@group(0) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;
// Prepass output (one start distance per tile) and statistics; see `prepass` components.
@group(0) @binding(2) var<storage, read_write> beam: array<f32>;
@group(0) @binding(3) var<storage, read_write> beam_stats: array<atomic<u32>, 2>;
// Progressive accumulation for shading components that need it (pathtrace).
@group(0) @binding(4) var<storage, read_write> accum: array<vec4<f32>>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width || gid.y >= cam.height) { return; }
    let sx = (2.0 * (f32(gid.x) + 0.5) / f32(cam.width) - 1.0) * cam.tan_half_fov * cam.aspect;
    let sy = (1.0 - 2.0 * (f32(gid.y) + 0.5) / f32(cam.height)) * cam.tan_half_fov;
    let d = normalize(cam.forward + sx * cam.right + sy * cam.up);
    let o = cam.origin;
    ct_iters = 0u;
    // Mesh mode: rays end at the mesh surface; a voxel in front of it still wins.
    let mt = mesh_t(gid.xy, d, cam.forward);
    // The prepass may let the ray start further out (its tile's cone is empty up to
    // there); the walk then starts at that point, and the hit's t is shifted back.
    let t0 = start_t(gid.xy);
    var h: Hit;
    if (t0 > 0.0) {
        h = ctrace(o + d * t0, d, min(vp.max_dist, mt) - t0);
        h.t += t0;
    } else {
        h = ctrace(o, d, min(vp.max_dist, mt));
    }
    if (VIEW_STEPS) {
        // Primary-ray loop iterations: red = low 8 bits, green = high bits (exact counts
        // for analysis), blue = a log-scale brightness for viewing.
        let n = ct_iters;
        let v = log2(f32(n) + 1.0) / 11.0;
        textureStore(out_tex, vec2<i32>(gid.xy), vec4<f32>(f32(n & 255u) / 255.0, f32(min(n >> 8u, 255u)) / 255.0, v, 1.0));
        return;
    }
    var color = unpack4x8unorm(cam.sky);
    if (!h.hit && mt < 1e29) { color = mesh_color(gid.xy); }
    if (h.hit) { color = shade_hit(h, o, d, cam.mode == 1u, gid.xy); }
    textureStore(out_tex, vec2<i32>(gid.xy), color);
}
