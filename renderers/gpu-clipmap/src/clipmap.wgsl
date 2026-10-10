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

// Clipmap: toroidal 3D textures addressed by absolute position modulo their size.
struct Clip { slots: vec3<i32>, _p: u32 }   // chunk slots per axis
@group(2) @binding(0) var<uniform> clip: Clip;
@group(2) @binding(1) var vox: texture_3d<u32>;   // block ids, slots*32 per axis
@group(2) @binding(2) var occ: texture_3d<u32>;   // 1 per non-empty 4^3 brick, slots*8 per axis
@group(2) @binding(3) var tags: texture_3d<u32>;  // per chunk slot: owner tag (0 = empty)

fn wrap(v: vec3<i32>, n: vec3<i32>) -> vec3<i32> {
    return ((v % n) + n) % n;
}

fn chunk_tag(c: vec3<i32>) -> u32 {
    let u = bitcast<vec3<u32>>(c) & vec3<u32>(1023u);
    return u.x | (u.y << 10u) | (u.z << 20u) | 0x80000000u;
}

// Same traversal as the shared `trace`, but chunk / brick / voxel lookups are textureLoads.
fn trace(o: vec3<f32>, d_in: vec3<f32>, max_t: f32) -> Hit {
    var res: Hit;
    res.hit = false;
    res.lod = 0u;
    let d = select(d_in, vec3<f32>(1e-20), abs(d_in) < vec3<f32>(1e-20));
    let inv = 1.0 / d;
    let stp = select(vec3<i32>(-1), vec3<i32>(1), d > vec3<f32>(0.0));
    let t0 = (vec3<f32>(vp.bounds_min) - o) * inv;
    let t1 = (vec3<f32>(vp.bounds_max) - o) * inv;
    let tlo = min(t0, t1);
    let thi = max(t0, t1);
    let tnear = max(max(max(tlo.x, tlo.y), tlo.z), 0.0);
    let tfar = min(min(min(thi.x, thi.y), thi.z), max_t);
    if (tnear > tfar) { return res; }

    let start_cell = vec3<i32>(floor(o));
    var cell = clamp(vec3<i32>(floor(o + d * tnear)), vp.bounds_min, vp.bounds_max - vec3<i32>(1));
    var axis = 3u;
    if (tnear > 0.0) {
        axis = select(select(2u, 1u, tlo.y >= tlo.z), 0u, tlo.x >= tlo.y && tlo.x >= tlo.z);
    }
    var cached = vec3<i32>(0x7fffffff);
    var present = false;
    res.t = tnear;
    for (var i = 0u; i < vp.max_steps; i++) {
        let abs_cell = cell + vp.cam_block;
        let key = abs_cell >> vec3<u32>(5u);
        if (any(key != cached)) {
            cached = key;
            present = textureLoad(tags, wrap(key, clip.slots), 0).r == chunk_tag(key);
        }
        var size = 32;
        if (present) {
            if (textureLoad(occ, wrap(abs_cell >> vec3<u32>(2u), clip.slots * 8), 0).r != 0u) {
                size = 1;
                let b = textureLoad(vox, wrap(abs_cell, clip.slots * 32), 0).r;
                if (b != 0u && any(cell != start_cell)) {
                    res.hit = true;
                    res.cell = cell;
                    res.block = b;
                    res.axis = axis;
                    return res;
                }
            } else {
                size = 4;
            }
        }
        let base = (abs_cell & vec3<i32>(-size)) - vp.cam_block;
        let bound = base + select(vec3<i32>(0), vec3<i32>(size), stp > vec3<i32>(0));
        let tb = (vec3<f32>(bound) - o) * inv;
        var a = 0u;
        if (tb.y < tb[a]) { a = 1u; }
        if (tb.z < tb[a]) { a = 2u; }
        res.t = tb[a];
        if (res.t > tfar) { return res; }
        var nc = clamp(vec3<i32>(floor(o + d * res.t)), base, base + vec3<i32>(size - 1));
        nc[a] = select(base[a] - 1, bound[a], stp[a] > 0);
        cell = nc;
        axis = a;
    }
    return res;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width || gid.y >= cam.height) { return; }
    let sx = (2.0 * (f32(gid.x) + 0.5) / f32(cam.width) - 1.0) * cam.tan_half_fov * cam.aspect;
    let sy = (1.0 - 2.0 * (f32(gid.y) + 0.5) / f32(cam.height)) * cam.tan_half_fov;
    let d = normalize(cam.forward + sx * cam.right + sy * cam.up);
    let o = cam.origin;
    let h = trace(o, d, vp.max_dist);
    var color = unpack4x8unorm(cam.sky);
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
