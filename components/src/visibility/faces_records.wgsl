// Face drawer, fragment data from the face records (see faces.wgsl).

struct VOut {
    @builtin(position) @invariant pos: vec4<f32>,
    @location(0) @interpolate(flat) info: vec2<u32>, // face, instance
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    let v = src_vertex(ii, vi);
    var out: VOut;
    out.pos = fr.view_proj * vec4<f32>(v.rel - fr.cam_frac, 1.0);
    out.info = vec2<u32>(v.face, ii);
    return out;
}

// The hit of primary ray `d` on fragment `in`'s face, from its record.
fn fragment_hit(in: VOut, d: vec3<f32>) -> Hit {
    let ii = in.info.y;
    let face = in.info.x;
    let origin = src_origin(ii, face);
    var h = face_plane_hit(face, origin[face_axes(face).x], d, fragment_point(in.pos, d));
    h.block = src_block(ii);
    if (h.block == LOOKUP_BLOCK) { h.block = src_lookup(ii, face, h, d); }
    return h;
}
