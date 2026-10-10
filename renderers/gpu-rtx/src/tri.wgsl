// Triangle geometry: greedy quads as two triangles each; everything is resolved by the
// RT cores, the shader only decodes the quad that was hit.

fn face_axes(face: u32) -> vec3<u32> {
    switch face {
        case 0u: { return vec3<u32>(0u, 1u, 2u); }
        case 1u: { return vec3<u32>(0u, 2u, 1u); }
        case 2u: { return vec3<u32>(1u, 2u, 0u); }
        case 3u: { return vec3<u32>(1u, 0u, 2u); }
        case 4u: { return vec3<u32>(2u, 0u, 1u); }
        default: { return vec3<u32>(2u, 1u, 0u); }
    }
}

fn primary(o: vec3<f32>, d: vec3<f32>) -> Surface {
    var s: Surface;
    s.hit = false;
    var rq: ray_query;
    rayQueryInitialize(&rq, acc, RayDesc(cam.ray_flags | RAY_FLAG_FORCE_OPAQUE, 0xffu, 0.0, vp.max_dist, o, d));
    rayQueryProceed(&rq);
    let h = rayQueryGetCommittedIntersection(&rq);
    if (h.kind != RAY_QUERY_INTERSECTION_TRIANGLE) { return s; }
    let qi = chunk_info[h.instance_custom_data].x + h.primitive_index / 2u;
    let w0 = geo[qi * 2u];
    let face = (w0 >> 25u) & 7u;
    let ax = face_axes(face);
    let p = o + d * h.t;
    var l = vec3<f32>(0.0);
    l[ax.x] = select(0.0, 1.0, (face & 1u) == 0u);
    l[ax.y] = fract(p[ax.y]);
    l[ax.z] = fract(p[ax.z]);
    s.hit = true;
    s.t = h.t;
    s.block = geo[qi * 2u + 1u] & 0xffffu;
    s.face = face;
    s.local = l;
    return s;
}

fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool {
    var rq: ray_query;
    rayQueryInitialize(&rq, acc, RayDesc(RAY_FLAG_TERMINATE_ON_FIRST_HIT | RAY_FLAG_FORCE_OPAQUE, 0xffu, 0.0, max_t, p, d));
    rayQueryProceed(&rq);
    return rayQueryGetCommittedIntersection(&rq).kind != RAY_QUERY_INTERSECTION_NONE;
}
