// Traversal `rowrun` (from gpu-bitmask): the refloor walk, plus a row jump: in an empty
// cell of an occupied region, when the ray runs mostly along x, it crosses the empty run
// of its (y, z) row up to the next solid cell or the row's exit in one step, found with
// `firstTrailingBit` / `firstLeadingBit` on the row's occupancy mask (`s_row`). Same cells,
// t and axis as the plain walk. Fast with the `rows` storage; others answer `s_row`
// through the generic fallback (32 voxel queries).

// Exit distance through the x face of relative cell column `k`.
fn tbx_exit(k: i32, sx: i32, ox: f32, ix: f32) -> f32 {
    return (f32(k + select(0, 1, sx > 0)) - ox) * ix;
}

fn ctrace(o: vec3<f32>, d_in: vec3<f32>, max_t: f32) -> Hit {
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
        ct_iters += 1u;
        let abs_cell = cell + vp.cam_block;
        let key = abs_cell >> vec3<u32>(5u);
        if (any(key != cached)) {
            cached = key;
            val = lookup_chunk(chunk_key(key, 0u));
        }
        var size = 32;
        if (val != EMPTY) {
            var b = 0u;
            if ((val & UNIFORM_BIT) != 0u) {
                b = val & 0xffffu;
                size = 1;
            } else {
                let local = abs_cell & vec3<i32>(31);
                let p = s_probe(val, local);
                b = p.x;
                size = i32(p.y);
                if (b == 0u && size == 1 && x_major) {
                    // Row jump: x columns the ray crosses before leaving this (y, z) row.
                    let row = s_row(val, local.y, local.z);
                    let bnd = cell + select(vec3<i32>(0), vec3<i32>(1), stp > vec3<i32>(0));
                    let tyz = (vec2<f32>(vec2<i32>(bnd.y, bnd.z)) - o.yz) * inv.yz;
                    let limit = min(min(tyz.x, tyz.y), tfar);
                    let lo = cell.x - local.x;
                    let hi = lo + 31;
                    var e = clamp(i32(floor(o.x + d.x * limit)), lo, hi);
                    if ((e - cell.x) * stp.x < 0) { e = cell.x; }
                    // Fix the float estimate against the exact DDA predicate.
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
