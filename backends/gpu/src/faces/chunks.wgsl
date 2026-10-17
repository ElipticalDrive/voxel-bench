// Meshing's face source for the face drawer (`visibility/faces.wgsl`): the geometry
// component's instances, drawn with vertex pulling (`g_face` / `g_size` / `g_corner` place
// each vertex); the draw component supplies the fragment (and compute) entry points, built on
// `face_hit`; the shading component colours the hit (`shade_surface`), with shadow rays
// through the composed storage. The frame is the executor's (`frame.wgsl`).

@group(0) @binding(8) var<storage, read> quads: array<vec2<u32>>;
@group(0) @binding(9) var<storage, read> chunk_pos: array<vec4<i32>>;

// Minimum corner of instance `q`'s `face` (camera-block relative), on the face plane.
fn quad_base(q: vec2<u32>, face: u32) -> vec3<f32> {
    let l = vec3<f32>(f32(q.x & 31u), f32((q.x >> 5u) & 31u), f32((q.x >> 10u) & 31u));
    return vec3<f32>(chunk_pos[q.y >> 16u].xyz * 32 - vp.cam_block) + face_origin(l, face);
}

fn src_vertex(ii: u32, vi: u32) -> FaceVertex {
    let q = quads[ii];
    let face = g_face(q, vi);
    return FaceVertex(face_corner(quad_base(q, face), face, g_size(q), g_corner(vi)), face, q.y & 0xffffu);
}

fn src_origin(ii: u32, face: u32) -> vec3<f32> {
    return quad_base(quads[ii], face);
}

fn src_block(ii: u32) -> u32 {
    return quads[ii].y & 0xffffu;
}

// Quads merged across block types (`merge = exposure`) look their block up in the cell
// kept inside the quad (at silhouettes the analytic hit can land a hair outside it); the
// texel uses the unclamped cell, which matched the reference better.
fn src_lookup(ii: u32, face: u32, h: Hit, d: vec3<f32>) -> u32 {
    let q = quads[ii];
    let ax = face_axes(face);
    let base = quad_base(q, face);
    let size = g_size(q);
    let p = fr.cam_frac + d * h.t;
    var cell = h.cell;
    cell[ax.y] = i32(clamp(floor(p[ax.y]), base[ax.y], base[ax.y] + size.x - 1.0));
    cell[ax.z] = i32(clamp(floor(p[ax.z]), base[ax.z], base[ax.z] + size.y - 1.0));
    return s_block_at(cell + vp.cam_block);
}

fn src_view(ii: u32, color: vec4<f32>) -> vec4<f32> {
    return color;
}
