// Traversal `sphere` (from gpu-sdf): Chebyshev sphere tracing. In a cell at distance k
// from the nearest solid (`s_dist`), the whole box cell +- (k - 1), clipped to the top cell,
// is empty, so the ray leaves it in one step. Fast with the `distance` storage; other
// storages answer `s_dist` through the generic fallback.

fn ctrace(o: vec3<f32>, d_in: vec3<f32>, max_t: f32) -> Hit {
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
    for (var i = 0u; i < vp.max_steps; i++) {
        ct_iters += 1u;
        let abs_cell = cell + vp.cam_block;
        let key = abs_cell >> vec3<u32>(S_TOP);
        if (any(key != cached)) {
            cached = key;
            val = s_top(key);
        }
        let top = 1 << S_TOP;
        let chunk_lo = (abs_cell & vec3<i32>(-top)) - vp.cam_block;
        // The empty box the ray may cross in one step: [lo, hi).
        var lo = chunk_lo;
        var hi = chunk_lo + vec3<i32>(top);
        if (val != EMPTY) {
            var b = 0u;
            var k = 0u;
            let local = abs_cell & vec3<i32>(top - 1);
            if ((val & UNIFORM_BIT) != 0u) {
                b = val & 0xffffu;
            } else {
                k = s_dist(val, local);
                if (k == 0u) { b = s_block(val, local); }
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
            hi = min(cell + vec3<i32>(r + 1), chunk_lo + vec3<i32>(top));
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
