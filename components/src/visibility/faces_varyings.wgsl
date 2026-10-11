// Face drawer, fragment data from flat varyings (see faces.wgsl).

struct VOut {
    @builtin(position) @invariant pos: vec4<f32>,
    @location(0) @interpolate(flat) info: vec2<u32>, // face, instance
    @location(1) @interpolate(flat) plane: f32,       // face plane along its normal axis
    @location(2) @interpolate(flat) block: u32,       // or LOOKUP_BLOCK
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    let v = src_vertex(ii, vi);
    var out: VOut;
    out.pos = fr.view_proj * vec4<f32>(v.rel - fr.cam_frac, 1.0);
    out.info = vec2<u32>(v.face, ii);
    out.plane = v.rel[face_axes(v.face).x];
    out.block = v.block;
    return out;
}

// The hit of primary ray `d` on fragment `in`'s face, from its varyings.
fn fragment_hit(in: VOut, d: vec3<f32>) -> Hit {
    var h = face_plane_hit(in.info.x, in.plane, d, fragment_point(in.pos, d));
    h.block = in.block;
    if (h.block == LOOKUP_BLOCK) { h.block = src_lookup(in.info.y, in.info.x, h, d); }
    return h;
}
