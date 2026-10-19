// Visibility `splat` on the GPU (the root of gpu-splat's program): software point
// rasterisation. Each point of geometry `points` is a face-exposed block; passes: clear,
// depth splat (atomicMin of the depth where each pixel's ray enters the block), the same for
// points larger than `max_splat` (a workgroup each), id splat (atomicMax where depth
// matches), the same for the large points, then the common deferred pass over the ids (the
// resolve).

struct Camera {
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, mode: u32,
    origin: vec3<f32>, sky: u32,
    width: u32, height: u32, count: u32, max_splat: u32,
}
struct Point { pos: vec3<i32>, info: u32 } // info: block | face_mask << 16

@group(0) @binding(0) var<uniform> cam: Camera;
#ifdef HDR
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba16float, write>;
#else
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;
#endif
@group(0) @binding(8) var<storage, read> points: array<Point>;
@group(0) @binding(9) var<storage, read_write> zbuf: array<atomic<u32>>;
@group(0) @binding(10) var<storage, read_write> idbuf: array<atomic<u32>>;

@compute @workgroup_size(8, 8)
fn clear(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width || gid.y >= cam.height) { return; }
    if (gid.x == 0u && gid.y == 0u) { atomicStore(&big.n, 0u); }
    let i = gid.y * cam.width + gid.x;
    atomicStore(&zbuf[i], 0xffffffffu);
    atomicStore(&idbuf[i], 0u);
}

// Points whose footprint is larger than `max_splat` px on a side: a whole workgroup splats
// each of them (`splat_big_*`), so a near block covers all of its pixels.
struct Big { n: atomic<u32>, ids: array<u32> }
@group(0) @binding(11) var<storage, read_write> big: Big;

// A point's footprint: the screen rectangle of its block's eight corners (the whole screen
// if a corner is behind the camera), and its centre's view depth.
struct Splat { ok: bool, big: bool, tiny: bool, x0: i32, y0: i32, x1: i32, y1: i32, z: f32, bmin: vec3<f32> }

fn project(id: u32) -> Splat {
    var s: Splat;
    s.ok = false;
    let pt = points[id];
    s.bmin = vec3<f32>(pt.pos - vp.cam_block);
    let c = s.bmin + vec3<f32>(0.5) - cam.origin;
    let z = dot(c, cam.forward);
    if (z < -1.0 || z > vp.max_dist) { return s; }
    // Back-face style cull: skip if no exposed face looks toward the camera.
    let mask = pt.info >> 16u;
    var vis = false;
    for (var f = 0u; f < 6u; f++) {
        if ((mask & (1u << f)) != 0u && dot(face_normal(f), -c) > -0.5) { vis = true; }
    }
    if (!vis) { return s; }
    var lo = vec2<f32>(1e30);
    var hi = vec2<f32>(-1e30);
    var behind = false;
    for (var k = 0u; k < 8u; k++) {
        let v = s.bmin + vec3<f32>(f32(k & 1u), f32((k >> 1u) & 1u), f32(k >> 2u)) - cam.origin;
        let vz = dot(v, cam.forward);
        if (vz < 0.05) { behind = true; break; }
        let p = vec2<f32>(
            (dot(v, cam.right) / (vz * cam.tan_half_fov * cam.aspect) * 0.5 + 0.5) * f32(cam.width),
            (0.5 - dot(v, cam.up) / (vz * cam.tan_half_fov) * 0.5) * f32(cam.height));
        lo = min(lo, p);
        hi = max(hi, p);
    }
    if (behind) {
        // Its corners all lie within 1.74 of the camera plane, so a visible part lies within
        // that depth inside the frustum and its centre near the camera; farther, it is off
        // screen.
        let tx = cam.tan_half_fov * cam.aspect;
        if (length(c) > 1.74 * sqrt(1.0 + tx * tx + cam.tan_half_fov * cam.tan_half_fov) + 0.87) { return s; }
        lo = vec2<f32>(0.0);
        hi = vec2<f32>(f32(cam.width), f32(cam.height));
    }
    s.tiny = !behind && max(hi.x - lo.x, hi.y - lo.y) < 2.0;
    s.x0 = max(i32(floor(lo.x)), 0);
    s.y0 = max(i32(floor(lo.y)), 0);
    s.x1 = min(i32(floor(hi.x)), i32(cam.width) - 1);
    s.y1 = min(i32(floor(hi.y)), i32(cam.height) - 1);
    if (s.x0 > s.x1 || s.y0 > s.y1) { return s; }
    s.big = max(s.x1 - s.x0, s.y1 - s.y0) >= i32(cam.max_splat);
    s.z = max(z, 0.0);
    s.ok = true;
    return s;
}

// Point `s`'s depth at pixel (x, y): the view depth where the pixel's ray enters its block;
// a tiny point (under 2 px) that the ray misses still covers the pixel at its centre's
// depth, so distant points do not vanish between pixel centres. `none` elsewhere.
const NONE: u32 = 0xffffffffu;
fn depth_at(s: Splat, x: i32, y: i32) -> u32 {
    let d = view_ray(vec2<u32>(u32(x), u32(y)));
    let inv = 1.0 / d;
    let ta = (s.bmin - cam.origin) * inv;
    let tb = (s.bmin + vec3<f32>(1.0) - cam.origin) * inv;
    let tlo = min(ta, tb);
    let thi = max(ta, tb);
    let tn = max(tlo.x, max(tlo.y, tlo.z));
    let tf = min(thi.x, min(thi.y, thi.z));
    if (tn <= tf && tf > 0.0) { return bitcast<u32>(max(tn, 0.0) * dot(d, cam.forward)); }
    if (s.tiny) { return bitcast<u32>(s.z); }
    return NONE;
}

fn depth_px(s: Splat, x: i32, y: i32) {
    let z = depth_at(s, x, y);
    if (z != NONE) { atomicMin(&zbuf[u32(y) * cam.width + u32(x)], z); }
}

fn id_px(s: Splat, id: u32, x: i32, y: i32) {
    let i = u32(y) * cam.width + u32(x);
    let z = depth_at(s, x, y);
    if (z != NONE && atomicLoad(&zbuf[i]) == z) { atomicMax(&idbuf[i], id + 1u); }
}

@compute @workgroup_size(64)
fn splat_depth(@builtin(global_invocation_id) gid: vec3<u32>) {
    let id = gid.x + gid.y * 65535u * 64u;
    if (id >= cam.count) { return; }
    let s = project(id);
    if (!s.ok) { return; }
    if (s.big) {
        let slot = atomicAdd(&big.n, 1u);
        if (slot < arrayLength(&big.ids)) { big.ids[slot] = id; }
        return;
    }
    for (var y = s.y0; y <= s.y1; y++) {
        for (var x = s.x0; x <= s.x1; x++) { depth_px(s, x, y); }
    }
}

@compute @workgroup_size(64)
fn splat_id(@builtin(global_invocation_id) gid: vec3<u32>) {
    let id = gid.x + gid.y * 65535u * 64u;
    if (id >= cam.count) { return; }
    let s = project(id);
    if (!s.ok || s.big) { return; }
    for (var y = s.y0; y <= s.y1; y++) {
        for (var x = s.x0; x <= s.x1; x++) { id_px(s, id, x, y); }
    }
}

// The big points: workgroup `wg` takes every `groups`-th one, its 64 threads stride over the
// footprint's pixels.
@compute @workgroup_size(64)
fn splat_big_depth(@builtin(workgroup_id) wg: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>, @builtin(local_invocation_index) t: u32) {
    let n = min(atomicLoad(&big.n), arrayLength(&big.ids));
    for (var i = wg.x; i < n; i += groups.x) {
        let s = project(big.ids[i]);
        let w = u32(s.x1 - s.x0 + 1);
        for (var k = t; k < w * u32(s.y1 - s.y0 + 1); k += 64u) { depth_px(s, s.x0 + i32(k % w), s.y0 + i32(k / w)); }
    }
}

@compute @workgroup_size(64)
fn splat_big_id(@builtin(workgroup_id) wg: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>, @builtin(local_invocation_index) t: u32) {
    let n = min(atomicLoad(&big.n), arrayLength(&big.ids));
    for (var i = wg.x; i < n; i += groups.x) {
        let id = big.ids[i];
        let s = project(id);
        let w = u32(s.x1 - s.x0 + 1);
        for (var k = t; k < w * u32(s.y1 - s.y0 + 1); k += 64u) { id_px(s, id, s.x0 + i32(k % w), s.y0 + i32(k / w)); }
    }
}

// The resolve is the common deferred pass (`shading/deferred.wgsl`, entry `shade`) over the
// id buffer: a point id is the record's encoding here, decoded by ray-boxing the pixel
// against the winning block. Splatting settles visibility only once every point is in, so
// shading is deferred by construction.

fn record_at(px: vec2<u32>) -> u32 { return atomicLoad(&idbuf[px.y * cam.width + px.x]); }
fn record_hit(id: u32) -> bool { return id != 0u; }
fn record_view(id: u32, color: vec4<f32>) -> vec4<f32> { return color; }
fn view_ray(px: vec2<u32>) -> vec3<f32> {
    return camera_ray(cam.forward, cam.right, cam.up, cam.tan_half_fov, cam.aspect, vec2<f32>(px) + 0.5, vec2<f32>(f32(cam.width), f32(cam.height)));
}
fn view_miss(px: vec2<u32>) -> vec4<f32> { return unpack4x8unorm(cam.sky); }
fn view_lit() -> bool { return cam.mode == 1u; }

// The surface of point `id` on primary ray `d`: the face the ray enters the block through,
// or, for splat pixels outside the block's silhouette, the box point nearest the ray.
fn record_decode(id: u32, d: vec3<f32>) -> Surface {
    let pt = points[id - 1u];
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
    return Surface(p, face_normal(face), local, pt.info & 0xffffu, face);
}
