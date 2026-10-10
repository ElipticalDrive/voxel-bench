// gpu-frag-dda: the gpu-dda ray caster run as a fragment shader over a full-screen triangle.

struct Camera {
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, mode: u32,      // 0 unlit, 1 lit
    origin: vec3<f32>, sky: u32,   // camera position relative to vp.cam_block; packed sRGB sky
    width: u32, height: u32, _p0: u32, _p1: u32,
}

@group(0) @binding(0) var<uniform> cam: Camera;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    // frag.xy is the pixel centre (x + 0.5, y + 0.5), y down: same as gid + 0.5 in gpu-dda.
    let sx = (2.0 * frag.x / f32(cam.width) - 1.0) * cam.tan_half_fov * cam.aspect;
    let sy = (1.0 - 2.0 * frag.y / f32(cam.height)) * cam.tan_half_fov;
    let d = normalize(cam.forward + sx * cam.right + sy * cam.up);
    let o = cam.origin;
    let h = trace(o, d, vp.max_dist);
    var color = unpack4x8unorm(cam.sky);
    if (h.hit) {
        let face = face_entered(h.axis, d[h.axis] > 0.0);
        let p = o + d * h.t;
        var local = p - vec3<f32>(h.cell);
        if (h.lod > 0u) { local = p - floor(p); }
        let texel = surface_texel(h.block, face, local);
        // A branch, not `select`: select evaluates both sides, so unlit frames paid for
        // lighting and shadow rays (unlit and lit GPU times were identical).
        color = texel;
        if (cam.mode == 1u) { color = shade_lit(texel, p, face_normal(face)); }
    }
    return color;
}
