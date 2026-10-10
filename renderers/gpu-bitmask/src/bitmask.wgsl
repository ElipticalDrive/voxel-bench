// gpu-bitmask: per-pixel ray cast; per-chunk x-row occupancy bitmasks let rays jump along rows.

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
    let h = trace_bm(o, d, vp.max_dist);
    var color = unpack4x8unorm(cam.sky);
    if (h.hit) {
        let face = face_entered(h.axis, d[h.axis] > 0.0);
        let p = o + d * h.t;
        // LOD cells span several blocks; textures still tile once per block.
        var local = p - vec3<f32>(h.cell);
        if (h.lod > 0u) { local = p - floor(p); }
        let texel = surface_texel(h.block, face, local);
        color = select(texel, shade_lit(texel, p, face_normal(face)), cam.mode == 1u);
    }
    textureStore(out_tex, vec2<i32>(gid.xy), color);
}

// One u32 per (y, z) row of each dense chunk slot: bit x set when the voxel is non-air.
@group(2) @binding(0) var<storage, read> rows: array<u32>;

fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool { return trace_bm(p, d, max_t).hit; }

// Exit distance through the x face of relative cell column `k`.
fn tbx_exit(k: i32, sx: i32, ox: f32, ix: f32) -> f32 {
    return (f32(k + select(0, 1, sx > 0)) - ox) * ix;
}

// `trace` from voxels.wgsl, with the voxel level answered by the row bitmask and
// empty row runs crossed in one jump (same cells, t and axis as the plain DDA).
fn trace_bm(o: vec3<f32>, d_in: vec3<f32>, max_t: f32) -> Hit {
    var res: Hit;
    res.hit = false;
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
    // Row jumps only pay off when the ray runs mostly along x.
    let ad = abs(d);
    let x_major = ad.x >= ad.y && ad.x >= ad.z;
    var cached = vec3<i32>(0x7fffffff);
    var val = EMPTY;
    res.t = tnear;
    for (var i = 0u; i < vp.max_steps; i++) {
        let abs_cell = cell + vp.cam_block;
        let key = chunk_key(abs_cell >> vec3<u32>(5u), 0u);
        if (any(key != cached)) {
            cached = key;
            val = lookup_chunk(key);
        }
        var size = 32;
        if (val != EMPTY) {
            let local = abs_cell & vec3<i32>(31);
            var b = 0u;
            if ((val & UNIFORM_BIT) != 0u) {
                b = val & 0xffffu;
                size = 1;
            } else if (brick_occupied(val, local)) {
                size = 1;
                let row = rows[val * 1024u + u32(local.y * 32 + local.z)];
                if (((row >> u32(local.x)) & 1u) != 0u) {
                    b = voxel(val, local);
                } else if (x_major) {
                    // Row jump: x columns the ray crosses before leaving this (y, z) row.
                    let bnd = cell + select(vec3<i32>(0), vec3<i32>(1), stp > vec3<i32>(0));
                    let tyz = (vec2<f32>(vec2<i32>(bnd.y, bnd.z)) - o.yz) * inv.yz;
                    let limit = min(min(tyz.x, tyz.y), tfar);
                    let lo = cell.x - local.x;
                    let hi = lo + 31;
                    var e = clamp(i32(floor(o.x + d.x * limit)), lo, hi);
                    if ((e - cell.x) * stp.x < 0) { e = cell.x; }
                    // Fix float estimate against the exact DDA predicate.
                    for (var f = 0; f < 2; f++) {
                        if (e != cell.x && tbx_exit(e - stp.x, stp.x, o.x, inv.x) > limit) { e -= stp.x; }
                    }
                    for (var f = 0; f < 2; f++) {
                        let n = e + stp.x;
                        if (n >= lo && n <= hi && tbx_exit(e, stp.x, o.x, inv.x) <= limit) { e = n; }
                    }
                    if (e != cell.x) {
                        let lx = u32(local.x);
                        let le = u32(e - lo);
                        var tgt = e;
                        if (stp.x > 0) {
                            let m = row & (0xffffffffu << (lx + 1u)) & (0xffffffffu >> (31u - le));
                            if (m != 0u) { tgt = lo + i32(firstTrailingBit(m)); }
                        } else {
                            let m = row & (0xffffffffu >> (32u - lx)) & (0xffffffffu << le);
                            if (m != 0u) { tgt = lo + i32(firstLeadingBit(m)); }
                        }
                        res.t = tbx_exit(tgt - stp.x, stp.x, o.x, inv.x);
                        cell.x = tgt;
                        axis = 0u;
                        continue;
                    }
                }
            } else {
                size = 4;
            }
            if (b != 0u && any(cell != start_cell)) {
                res.hit = true;
                res.cell = cell;
                res.block = b;
                res.axis = axis;
                res.lod = 0u;
                return res;
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
