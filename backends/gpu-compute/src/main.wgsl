// GPU-compute back end: one thread per pixel (compute, or a fragment shader over a
// full-screen triangle with `dispatch = "fragment"`). `ctrace` comes from the chosen
// visibility component, which reaches the world only through the chosen storage
// component; `shade_surface` comes from the chosen shading component.

struct Camera {
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, mode: u32,      // 0 unlit, 1 lit
    origin: vec3<f32>, sky: u32,   // camera position relative to vp.cam_block; packed sRGB sky
    width: u32, height: u32, frame: u32, prev_count: u32,   // frame counter; samples accumulated so far
    bounces: u32, spp: u32, view_max: u32, _p1: u32,        // pathtrace shading; steps view scale
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

// The colour of pixel `px` (or its step count with `view = "steps"`).
fn pixel_color(px: vec2<u32>) -> vec4<f32> {
    let sx = (2.0 * (f32(px.x) + 0.5) / f32(cam.width) - 1.0) * cam.tan_half_fov * cam.aspect;
    let sy = (1.0 - 2.0 * (f32(px.y) + 0.5) / f32(cam.height)) * cam.tan_half_fov;
    let d = normalize(cam.forward + sx * cam.right + sy * cam.up);
    let o = cam.origin;
    ct_iters = 0u;
    // Mesh mode: rays end at the mesh surface; a voxel in front of it still wins.
    let mt = mesh_t(px, d, cam.forward);
    // The prepass may let the ray start further out (its tile's cone is empty up to
    // there); the walk then starts at that point, and the hit's t is shifted back.
    let t0 = start_t(px);
    var h: Hit;
    if (t0 > 0.0) {
        h = ctrace(o + d * t0, d, min(vp.max_dist, mt) - t0);
        h.t += t0;
    } else {
        h = ctrace(o, d, min(vp.max_dist, mt));
    }
    if (VIEW_STEPS) {
        // Primary-ray loop iterations on the shared colour scale (log; white above the top).
        return ramp_value(f32(ct_iters), f32(cam.view_max), true);
    }
    var color = unpack4x8unorm(cam.sky);
    if (!h.hit && mt < 1e29) { color = mesh_color(px); }
    if (h.hit) { color = shade_surface(surface_of(h, o, d), d, cam.mode == 1u, px); }
    return color;
}

// `dispatch = "compute"`: one compute thread per pixel.
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width || gid.y >= cam.height) { return; }
    textureStore(out_tex, vec2<i32>(gid.xy), pixel_color(gid.xy));
}

// `dispatch = "fragment"` (was gpu-frag-dda): the same per pixel, as a fragment shader over
// a full-screen triangle (render pipeline instead of compute).
@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_full(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    // frag.xy is the pixel centre (x + 0.5, y + 0.5), y down: the same pixel as gid.
    return pixel_color(vec2<u32>(frag.xy));
}
