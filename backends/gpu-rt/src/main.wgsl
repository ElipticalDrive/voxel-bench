// GPU-RT back end: one compute thread per pixel casts a hardware ray query (`ctrace` from
// the visibility component over the geometry component's acceleration structure) and
// colours the hit with the shading component (`shade_surface`).

struct Camera {
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, mode: u32,      // 0 unlit, 1 lit
    origin: vec3<f32>, sky: u32,   // camera position relative to vp.cam_block; packed sRGB sky
    width: u32, height: u32, ray_flags: u32, _p0: u32,      // primary-ray culling
}

@group(0) @binding(0) var<uniform> cam: Camera;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var acc: acceleration_structure;
@group(0) @binding(3) var<storage, read> geo: array<u32>;              // quads or brick lists
@group(0) @binding(4) var<storage, read> chunk_info: array<vec4<u32>>; // per instance slot
// Bindings 5 and 6: the shading state (`sp`, `accum`; shading::state).
var<private> ct_iters: u32;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width || gid.y >= cam.height) { return; }
    let d = camera_ray(cam.forward, cam.right, cam.up, cam.tan_half_fov, cam.aspect, vec2<f32>(gid.xy) + 0.5, vec2<f32>(f32(cam.width), f32(cam.height)));
    let o = cam.origin;
    let h = ctrace(o, d, vp.max_dist);
    var color = unpack4x8unorm(cam.sky);
    if (h.hit) { color = shade_surface(surface_of(h, o, d), d, cam.mode == 1u, gid.xy); }
    textureStore(out_tex, vec2<i32>(gid.xy), color);
}
