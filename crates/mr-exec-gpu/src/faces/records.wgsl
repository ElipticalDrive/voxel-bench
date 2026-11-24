// The CPU sampler's face source for the face drawer (`visibility/faces.wgsl`): one unit
// quad (4-vertex strip) per face record uploaded from the CPU. The drawer's visibility
// component shades (`shade_surface`); lit-mode shadow rays walk their GPU storage.

@group(0) @binding(8) var<storage, read> quads: array<vec2<u32>>;
@group(0) @binding(9) var<storage, read> tags: array<u32>;  // views: kind << 30 | cluster id (while viewed)

// Record: x(15) | z(15) << 15 | face.lo(2) << 30 ; block(16) | y(15) << 16 | face.hi(1) << 31
fn rec_face(q: vec2<u32>) -> u32 {
    return (q.x >> 30u) | ((q.y >> 31u) << 2u);
}

// The face's origin (camera-block relative), on the face plane.
fn rec_origin(q: vec2<u32>, face: u32) -> vec3<f32> {
    let cell = vec3<i32>(i32(q.x & 0x7fffu), i32((q.y >> 16u) & 0x7fffu), i32((q.x >> 15u) & 0x7fffu)) - 16384;
    return vec3<f32>(fr.anchor.xyz + cell - vp.cam_block) + face_origin(vec3<f32>(0.0), face);
}

fn src_vertex(ii: u32, vi: u32) -> FaceVertex {
    let q = quads[ii];
    let face = rec_face(q);
    return FaceVertex(rec_origin(q, face) + face_corner(vec3<f32>(0.0), face, vec2<f32>(1.0), vec2<f32>(f32(vi & 1u), f32(vi >> 1u))), face, q.y & 0xffffu);
}

fn src_origin(ii: u32, face: u32) -> vec3<f32> {
    return rec_origin(quads[ii], face);
}

fn src_block(ii: u32) -> u32 {
    return quads[ii].y & 0xffffu;
}

// Faces are single blocks: never asked.
fn src_lookup(ii: u32, face: u32, h: Hit, d: vec3<f32>) -> u32 {
    return h.block;
}

// The post's category views of cluster discovery, rasterised on their own (`tags`, uploaded
// only while one is viewed) and copied into its quantity buffer. `clusters`: an id per
// cluster found this pass (category 1), carried over (2). `kinds`: confirmed (1), partial
// (2), carried over (3).
@fragment
fn fs_clusters(in: VOut) -> @location(0) u32 {
    let tag = tags[in.info.y];
    return select(2u, ((tag & 0x3fffffu) + 1u) << 8u | 1u, (tag >> 30u) != 0u);
}

@fragment
fn fs_kinds(in: VOut) -> @location(0) u32 {
    let kind = tags[in.info.y] >> 30u;
    return select(kind, 3u, kind == 0u);
}
