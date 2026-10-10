// gpu-sdf: distance-field ray marching. Each voxel of a dense chunk stores its Chebyshev
// distance k to the nearest solid voxel of the same chunk, so the cube of cells within
// k-1 of it is empty and a ray can leave that whole cube in one step. Jumps are clipped
// to the current chunk (the field is chunk-local); empty chunks are skipped whole.

struct Camera {
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, mode: u32,
    origin: vec3<f32>, sky: u32,
    width: u32, height: u32, max_steps: u32, _p0: u32,
}

@group(0) @binding(0) var<uniform> cam: Camera;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<storage, read> dist: array<u32>; // 4 x u8 per word, 8192 words per pool slot

fn df(slot: u32, l: vec3<i32>) -> u32 {
    let idx = u32(l.x | (l.z << 5u) | (l.y << 10u));
    return (dist[slot * 8192u + (idx >> 2u)] >> ((idx & 3u) * 8u)) & 0xffu;
}

fn trace_sdf(o: vec3<f32>, d_in: vec3<f32>, max_t: f32) -> Hit {
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
    res.t = tnear;
    var cached = vec3<i32>(0x7fffffff);
    var val = EMPTY;
    for (var i = 0u; i < cam.max_steps; i++) {
        let abs_cell = cell + vp.cam_block;
        let key = chunk_key(abs_cell >> vec3<u32>(5u), 0u);
        if (any(key != cached)) {
            cached = key;
            val = lookup_chunk(key);
        }
        let chunk_lo = (abs_cell & vec3<i32>(-32)) - vp.cam_block;
        // The empty box the ray may cross in one step: [lo, hi).
        var lo = chunk_lo;
        var hi = chunk_lo + vec3<i32>(32);
        if (val != EMPTY) {
            var b = 0u;
            var k = 0u;
            let local = abs_cell & vec3<i32>(31);
            if ((val & UNIFORM_BIT) != 0u) {
                b = val & 0xffffu;
            } else {
                k = df(val, local);
                // 255 = field not computed yet: plain single-voxel stepping.
                if (k == 0u || k == 255u) { b = voxel(val, local); }
                if (k == 255u) { k = 1u; }
            }
            if (b != 0u && any(cell != start_cell)) {
                res.hit = true;
                res.cell = cell;
                res.block = b;
                res.axis = axis;
                return res;
            }
            let r = i32(max(k, 1u)) - 1;
            lo = max(cell - vec3<i32>(r), chunk_lo);
            hi = min(cell + vec3<i32>(r + 1), chunk_lo + vec3<i32>(32));
        }
        let bound = select(lo, hi, stp > vec3<i32>(0));
        let tb = (vec3<f32>(bound) - o) * inv;
        var a = 0u;
        if (tb.y < tb[a]) { a = 1u; }
        if (tb.z < tb[a]) { a = 2u; }
        res.t = tb[a];
        if (res.t > tfar) { return res; }
        var nc = clamp(vec3<i32>(floor(o + d * res.t)), lo, hi - vec3<i32>(1));
        nc[a] = select(lo[a] - 1, hi[a], stp[a] > 0);
        cell = nc;
        axis = a;
    }
    return res;
}

fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool {
    return trace_sdf(p, d, max_t).hit;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width || gid.y >= cam.height) { return; }
    let sx = (2.0 * (f32(gid.x) + 0.5) / f32(cam.width) - 1.0) * cam.tan_half_fov * cam.aspect;
    let sy = (1.0 - 2.0 * (f32(gid.y) + 0.5) / f32(cam.height)) * cam.tan_half_fov;
    let d = normalize(cam.forward + sx * cam.right + sy * cam.up);
    let o = cam.origin;
    let h = trace_sdf(o, d, vp.max_dist);
    var color = unpack4x8unorm(cam.sky);
    if (h.hit) {
        let face = face_entered(h.axis, d[h.axis] > 0.0);
        let p = o + d * h.t;
        let texel = surface_texel(h.block, face, p - vec3<f32>(h.cell));
        // A branch, not `select`: select evaluates both sides, so unlit frames paid for
        // lighting and shadow rays (unlit and lit GPU times were identical).
        color = texel;
        if (cam.mode == 1u) { color = shade_lit(texel, p, face_normal(face)); }
    }
    textureStore(out_tex, vec2<i32>(gid.xy), color);
}
