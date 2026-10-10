// gpu-dda: one compute thread per pixel, ray cast through the shared voxel world.

struct Camera {
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, mode: u32,      // 0 unlit, 1 lit
    origin: vec3<f32>, sky: u32,   // camera position relative to vp.cam_block; packed sRGB sky
    width: u32, height: u32, _p0: u32, _p1: u32,
}

@group(0) @binding(0) var<uniform> cam: Camera;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width || gid.y >= cam.height) { return; }
    let sx = (2.0 * (f32(gid.x) + 0.5) / f32(cam.width) - 1.0) * cam.tan_half_fov * cam.aspect;
    let sy = (1.0 - 2.0 * (f32(gid.y) + 0.5) / f32(cam.height)) * cam.tan_half_fov;
    let d = normalize(cam.forward + sx * cam.right + sy * cam.up);
    let o = cam.origin;
    // Mesh mode: rays end at the mesh surface; a voxel in front of it still wins.
    let mt = mesh_t(gid.xy, d, cam.forward);
    let h = trace(o, d, min(vp.max_dist, mt));
    var color = unpack4x8unorm(cam.sky);
    if (!h.hit && mt < 1e29) { color = mesh_color(gid.xy); }
    if (h.hit) {
        let face = face_entered(h.axis, d[h.axis] > 0.0);
        let p = o + d * h.t;
        // LOD cells span several blocks; textures still tile once per block.
        var local = p - vec3<f32>(h.cell);
        if (h.lod > 0u) { local = p - floor(p); }
        let texel = surface_texel(h.block, face, local);
        // A branch, not `select`: select evaluates both sides, so unlit frames paid for
        // lighting and shadow rays (unlit and lit GPU times were identical).
        color = texel;
        if (cam.mode == 1u) { color = shade_lit(texel, p, face_normal(face)); }
    }
    textureStore(out_tex, vec2<i32>(gid.xy), color);
}
