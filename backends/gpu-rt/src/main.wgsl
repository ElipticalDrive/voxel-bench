// GPU-RT back end: one compute thread per pixel casts a hardware ray query (`ctrace` from
// the visibility component over the geometry component's acceleration structure) and
// colours the hit with the shading component (`shade_hit`).

struct Camera {
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, mode: u32,      // 0 unlit, 1 lit
    origin: vec3<f32>, sky: u32,   // camera position relative to vp.cam_block; packed sRGB sky
    width: u32, height: u32, frame: u32, prev_count: u32,   // frame counter; samples accumulated so far
    bounces: u32, spp: u32, ray_flags: u32, _p0: u32,       // pathtrace shading; primary-ray culling
}

@group(0) @binding(0) var<uniform> cam: Camera;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var acc: acceleration_structure;
@group(0) @binding(3) var<storage, read> geo: array<u32>;              // quads or brick lists
@group(0) @binding(4) var<storage, read> chunk_info: array<vec4<u32>>; // per instance slot
// Progressive accumulation for shading components that need it (pathtrace).
@group(0) @binding(5) var<storage, read_write> accum: array<vec4<f32>>;
var<private> ct_iters: u32;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width || gid.y >= cam.height) { return; }
    let sx = (2.0 * (f32(gid.x) + 0.5) / f32(cam.width) - 1.0) * cam.tan_half_fov * cam.aspect;
    let sy = (1.0 - 2.0 * (f32(gid.y) + 0.5) / f32(cam.height)) * cam.tan_half_fov;
    let d = normalize(cam.forward + sx * cam.right + sy * cam.up);
    let o = cam.origin;
    let h = ctrace(o, d, vp.max_dist);
    var color = unpack4x8unorm(cam.sky);
    if (h.hit) { color = shade_hit(h, o, d, cam.mode == 1u, gid.xy); }
    textureStore(out_tex, vec2<i32>(gid.xy), color);
}
