// Geometry `boxes` on gpu-rt (from gpu-rtx-bricks): one AABB per occupied 4^3 brick (or one
// per uniform solid chunk). The RT cores walk the BVH to candidate boxes; a small software
// DDA through the voxel pool finds the voxel inside and reports it with
// rayQueryGenerateIntersection.

// DDA through the box `[corner, corner + size)` (camera-block-relative cells) of the chunk
// whose minimum corner is `origin`. `value` is the chunk-table value (pool slot or uniform
// block). Ignores the cell containing `o`, like every other walk.
fn box_dda(o: vec3<f32>, d: vec3<f32>, origin: vec3<i32>, value: u32, corner: vec3<i32>, size: i32, max_t: f32) -> Hit {
    var r: Hit;
    r.hit = false;
    r.lod = 0u;
    let dd = select(d, vec3<f32>(1e-20), abs(d) < vec3<f32>(1e-20));
    let inv = 1.0 / dd;
    let t0 = (vec3<f32>(corner) - o) * inv;
    let t1 = (vec3<f32>(corner + vec3<i32>(size)) - o) * inv;
    let tlo = min(t0, t1);
    let thi = max(t0, t1);
    let tnear = max(max(max(tlo.x, tlo.y), tlo.z), 0.0);
    let tfar = min(min(min(thi.x, thi.y), thi.z), max_t);
    if (tnear > tfar) { return r; }
    let stp = select(vec3<i32>(-1), vec3<i32>(1), dd > vec3<f32>(0.0));
    let start_cell = vec3<i32>(floor(o));
    var cell = clamp(vec3<i32>(floor(o + dd * tnear)), corner, corner + vec3<i32>(size - 1));
    var axis = 3u;
    if (tnear > 0.0) {
        axis = select(select(2u, 1u, tlo.y >= tlo.z), 0u, tlo.x >= tlo.y && tlo.x >= tlo.z);
    }
    var t = tnear;
    for (var i = 0; i < 3 * size + 3; i++) {
        var b = value & 0xffffu;
        if ((value & UNIFORM_BIT) == 0u) {
            b = voxel(value, cell - origin);
        }
        if (b != 0u && any(cell != start_cell)) {
            r.hit = true;
            r.t = t;
            r.cell = cell;
            r.block = b;
            r.axis = axis;
            return r;
        }
        let bound = select(cell, cell + vec3<i32>(1), stp > vec3<i32>(0));
        let tb = (vec3<f32>(bound) - o) * inv;
        var a = 0u;
        if (tb.y < tb[a]) { a = 1u; }
        if (tb.z < tb[a]) { a = 2u; }
        t = tb[a];
        if (t > tfar) { return r; }
        cell[a] += stp[a];
        axis = a;
        if (cell[a] < corner[a] || cell[a] >= corner[a] + size) { return r; }
    }
    return r;
}

fn g_rt_trace(o: vec3<f32>, d: vec3<f32>, max_t: f32, shadow: bool) -> Hit {
    var best: Hit;
    best.hit = false;
    var best_t = max_t;
    var rq: ray_query;
    rayQueryInitialize(&rq, acc, RayDesc(select(0u, RAY_FLAG_TERMINATE_ON_FIRST_HIT, shadow), 0xffu, 0.0, max_t, o, d));
    while (rayQueryProceed(&rq)) {
        let c = rayQueryGetCandidateIntersection(&rq);
        if (c.kind != RAY_QUERY_INTERSECTION_AABB) { continue; }
        let info = chunk_info[c.instance_custom_data];
        let origin = vec3<i32>(round(c.object_to_world[3]));
        let b = geo[info.y + c.primitive_index];
        var corner = origin;
        var size = 32;
        if (b != 0xffffffffu) {
            corner = origin + vec3<i32>(i32(b & 7u), i32((b >> 6u) & 7u), i32((b >> 3u) & 7u)) * 4;
            size = 4;
        }
        let h = box_dda(o, d, origin, info.x, corner, size, best_t);
        if (h.hit && h.t <= best_t) {
            best = h;
            best_t = h.t;
            // (naga 30 mis-scopes a struct-field argument here; pass a loaded local.)
            let gen_t = best_t;
            rayQueryGenerateIntersection(&rq, gen_t);
            if (shadow) {
                rayQueryTerminate(&rq);
            }
        }
    }
    return best;
}
