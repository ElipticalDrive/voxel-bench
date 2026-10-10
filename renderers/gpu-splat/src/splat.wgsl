// gpu-splat: software point rasterisation. Each face-exposed block is a point; passes:
// clear, depth splat (atomicMin), id splat (atomicMax where depth matches), resolve.

struct Camera {
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, mode: u32,
    origin: vec3<f32>, sky: u32,
    width: u32, height: u32, count: u32, max_splat: u32,
}
struct Point { pos: vec3<i32>, info: u32 } // info: block | face_mask << 16

@group(0) @binding(0) var<uniform> cam: Camera;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<storage, read> points: array<Point>;
@group(0) @binding(3) var<storage, read_write> zbuf: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> idbuf: array<atomic<u32>>;

@compute @workgroup_size(8, 8)
fn clear(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width || gid.y >= cam.height) { return; }
    let i = gid.y * cam.width + gid.x;
    atomicStore(&zbuf[i], 0xffffffffu);
    atomicStore(&idbuf[i], 0u);
}

struct Splat { ok: bool, x0: i32, y0: i32, x1: i32, y1: i32, z: u32 }

fn project(id: u32) -> Splat {
    var s: Splat;
    s.ok = false;
    let pt = points[id];
    let c = vec3<f32>(pt.pos - vp.cam_block) + vec3<f32>(0.5) - cam.origin;
    let z = dot(c, cam.forward);
    if (z < 0.05 || z > vp.max_dist) { return s; }
    // Back-face style cull: skip if no exposed face looks toward the camera.
    let mask = pt.info >> 16u;
    var vis = false;
    for (var f = 0u; f < 6u; f++) {
        if ((mask & (1u << f)) != 0u && dot(face_normal(f), -c) > -0.5) { vis = true; }
    }
    if (!vis) { return s; }
    let sx = dot(c, cam.right) / (z * cam.tan_half_fov * cam.aspect);
    let sy = dot(c, cam.up) / (z * cam.tan_half_fov);
    let px = (sx * 0.5 + 0.5) * f32(cam.width);
    let py = (0.5 - sy * 0.5) * f32(cam.height);
    let size = 1.6 / (z * cam.tan_half_fov) * 0.5 * f32(cam.height);
    let half = min(max(size * 0.5, 0.5), f32(cam.max_splat) * 0.5);
    s.x0 = max(i32(floor(px - half + 0.5)), 0);
    s.y0 = max(i32(floor(py - half + 0.5)), 0);
    s.x1 = min(i32(floor(px + half + 0.5)), i32(cam.width) - 1);
    s.y1 = min(i32(floor(py + half + 0.5)), i32(cam.height) - 1);
    if (s.x0 > s.x1 || s.y0 > s.y1) { return s; }
    s.z = bitcast<u32>(z);
    s.ok = true;
    return s;
}

@compute @workgroup_size(64)
fn splat_depth(@builtin(global_invocation_id) gid: vec3<u32>) {
    let id = gid.x + gid.y * 65535u * 64u;
    if (id >= cam.count) { return; }
    let s = project(id);
    if (!s.ok) { return; }
    for (var y = s.y0; y <= s.y1; y++) {
        for (var x = s.x0; x <= s.x1; x++) { atomicMin(&zbuf[u32(y) * cam.width + u32(x)], s.z); }
    }
}

@compute @workgroup_size(64)
fn splat_id(@builtin(global_invocation_id) gid: vec3<u32>) {
    let id = gid.x + gid.y * 65535u * 64u;
    if (id >= cam.count) { return; }
    let s = project(id);
    if (!s.ok) { return; }
    for (var y = s.y0; y <= s.y1; y++) {
        for (var x = s.x0; x <= s.x1; x++) {
            let i = u32(y) * cam.width + u32(x);
            if (atomicLoad(&zbuf[i]) == s.z) { atomicMax(&idbuf[i], id + 1u); }
        }
    }
}

@compute @workgroup_size(8, 8)
fn resolve(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width || gid.y >= cam.height) { return; }
    let id = atomicLoad(&idbuf[gid.y * cam.width + gid.x]);
    var color = unpack4x8unorm(cam.sky);
    if (id != 0u) {
        let pt = points[id - 1u];
        let sx = (2.0 * (f32(gid.x) + 0.5) / f32(cam.width) - 1.0) * cam.tan_half_fov * cam.aspect;
        let sy = (1.0 - 2.0 * (f32(gid.y) + 0.5) / f32(cam.height)) * cam.tan_half_fov;
        let d = normalize(cam.forward + sx * cam.right + sy * cam.up);
        let o = cam.origin;
        let bmin = vec3<f32>(pt.pos - vp.cam_block);
        let inv = 1.0 / d;
        let ta = (bmin - o) * inv;
        let tb = (bmin + vec3<f32>(1.0) - o) * inv;
        let tlo = min(ta, tb);
        let thi = max(ta, tb);
        var axis = 0u;
        if (tlo.y > tlo[axis]) { axis = 1u; }
        if (tlo.z > tlo[axis]) { axis = 2u; }
        let tn = tlo[axis];
        let tf = min(thi.x, min(thi.y, thi.z));
        var p: vec3<f32>;
        var face: u32;
        if (tn <= tf && tf > 0.0) {
            p = o + d * max(tn, 0.0);
            face = face_entered(axis, d[axis] > 0.0);
        } else {
            // Splat pixel outside the block's silhouette: clamp to the box.
            let ctr = bmin + vec3<f32>(0.5);
            p = clamp(o + d * dot(ctr - o, d), bmin, bmin + vec3<f32>(1.0));
            let q = abs(p - ctr);
            var a = 0u;
            if (q.y > q[a]) { a = 1u; }
            if (q.z > q[a]) { a = 2u; }
            face = face_entered(a, (p - ctr)[a] < 0.0);
        }
        let local = clamp(p - bmin, vec3<f32>(0.0), vec3<f32>(0.9999));
        let texel = surface_texel(pt.info & 0xffffu, face, local);
        color = select(texel, shade_lit(texel, p, face_normal(face)), cam.mode == 1u);
    }
    textureStore(out_tex, vec2<i32>(gid.xy), color);
}
