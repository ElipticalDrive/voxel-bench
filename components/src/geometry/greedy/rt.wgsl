// Geometry `greedy` / `naive` on gpu-rt: the quads as two triangles each in one BLAS per
// chunk; the RT cores resolve everything, the shader only decodes the quad that was hit
// into a `Hit`. `shadow`: any hit ends the query, no facing is culled.

fn g_rt_trace(o: vec3<f32>, d: vec3<f32>, max_t: f32, shadow: bool) -> Hit {
    var res: Hit;
    res.hit = false;
    res.lod = 0u;
    let flags = select(cam.ray_flags, RAY_FLAG_TERMINATE_ON_FIRST_HIT, shadow) | RAY_FLAG_FORCE_OPAQUE;
    var rq: ray_query;
    rayQueryInitialize(&rq, acc, RayDesc(flags, 0xffu, 0.0, max_t, o, d));
    rayQueryProceed(&rq);
    let h = rayQueryGetCommittedIntersection(&rq);
    if (h.kind != RAY_QUERY_INTERSECTION_TRIANGLE) { return res; }
    res.hit = true;
    res.t = h.t;
    if (shadow) { return res; }
    let qi = chunk_info[h.instance_custom_data].x + h.primitive_index / 2u;
    let w0 = geo[qi * 2u];
    let face = (w0 >> 25u) & 7u;
    let a = face / 2u;
    // The cell behind the face: on the face plane along the normal, floor elsewhere.
    let p = o + d * h.t;
    res.cell = vec3<i32>(floor(p));
    res.cell[a] = i32(round(p[a])) - select(0, 1, (face & 1u) == 0u);
    res.axis = a;
    res.block = geo[qi * 2u + 1u] & 0xffffu;
    return res;
}
