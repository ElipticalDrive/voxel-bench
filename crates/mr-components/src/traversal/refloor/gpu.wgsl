// Traversal `refloor`: gpu-dda's walk without LOD. Each step asks the storage for the
// largest empty aligned cube at the cell (`s_probe`; a whole top cell if it is absent),
// steps to its exit and re-floors the position. Ignores the cell containing `o`, like the
// reference. With `LOD` (a compile-time constant of the assembler, true when LOD chunks are
// streamed), cells, top cells and probe results grow 2^lod with distance (`lod_of`, the
// finest level whose streamed region contains the cell; gpu-dda-lod's scheme).

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
    var cached = vec3<i32>(0x7fffffff);
    var cached_lod = 0u;
    var val = EMPTY;
    res.t = tnear;
    for (var i = 0u; i < MAX_STEPS; i++) {
        ct_iters += 1u;
        let abs_cell = cell + vp.cam_block;
        var lod = 0u;
        if (LOD) { lod = lod_of(abs_cell); }
        let key = abs_cell >> vec3<u32>(S_TOP + lod);
        if (any(key != cached) || lod != cached_lod) {
            cached = key;
            cached_lod = lod;
            if (LOD) { val = s_top_lod(key, lod); } else { val = s_top(key); }
        }
        var size = 1 << (S_TOP + lod);
        if (val != EMPTY) {
            var b = 0u;
            if ((val & UNIFORM_BIT) != 0u) {
                b = val & 0xffffu;
                size = 1 << lod;
            } else {
                let p = s_probe(val, (abs_cell >> vec3<u32>(lod)) & vec3<i32>((1 << S_TOP) - 1));
                b = p.x;
                size = i32(p.y) << lod;
            }
            if (b != 0u && any(cell != start_cell)) {
                res.hit = true;
                res.cell = cell;
                res.block = b;
                res.axis = axis;
                res.lod = lod;
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
