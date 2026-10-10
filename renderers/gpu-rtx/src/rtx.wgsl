// gpu-rtx: hardware ray queries against a two-level acceleration structure (one BLAS
// per chunk, one TLAS instance per chunk translated relative to the camera block).
// The geometry-specific part (`primary`, `occluded`) comes from tri.wgsl or bricks.wgsl.

struct Camera {
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, mode: u32,      // 0 unlit, 1 lit
    origin: vec3<f32>, sky: u32,   // camera position relative to vp.cam_block; packed sRGB sky
    width: u32, height: u32, ray_flags: u32, _p0: u32,
}

@group(0) @binding(0) var<uniform> cam: Camera;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var acc: acceleration_structure;
@group(0) @binding(3) var<storage, read> geo: array<u32>;          // quads (tri) or brick lists (bricks)
@group(0) @binding(4) var<storage, read> chunk_info: array<vec4<u32>>; // per instance slot

struct Surface { hit: bool, t: f32, block: u32, face: u32, local: vec3<f32> }

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width || gid.y >= cam.height) { return; }
    let sx = (2.0 * (f32(gid.x) + 0.5) / f32(cam.width) - 1.0) * cam.tan_half_fov * cam.aspect;
    let sy = (1.0 - 2.0 * (f32(gid.y) + 0.5) / f32(cam.height)) * cam.tan_half_fov;
    let d = normalize(cam.forward + sx * cam.right + sy * cam.up);
    let o = cam.origin;
    let s = primary(o, d);
    var color = unpack4x8unorm(cam.sky);
    if (s.hit) {
        let texel = surface_texel(s.block, s.face, s.local);
        color = select(texel, shade_lit(texel, o + d * s.t, face_normal(s.face)), cam.mode == 1u);
    }
    textureStore(out_tex, vec2<i32>(gid.xy), color);
}
