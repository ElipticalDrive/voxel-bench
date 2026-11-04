// The CPU sampler's face source for the face drawer (`visibility/faces.wgsl`): one unit
// quad (4-vertex strip) per face record uploaded from the CPU (`view`: 1 clusters, 2 kinds,
// cluster discovery). The drawer's visibility component shades (`shade_surface`); lit-mode
// shadow rays walk their GPU storage.

@group(0) @binding(8) var<storage, read> quads: array<vec2<u32>>;
@group(0) @binding(9) var<storage, read> tags: array<u32>;  // views: kind << 30 | cluster id

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

fn src_view(ii: u32, color: vec4<f32>) -> vec4<f32> {
    if (fr.view == 0u) { return color; }
    let tag = tags[ii];
    let kind = tag >> 30u;
    let g = dot(color.rgb, vec3<f32>(0.3, 0.5, 0.2));
    if (fr.view == 1u) {
        // Clusters: one colour per cluster (grey for faces carried over from earlier passes).
        var h = (tag & 0x3fffffffu) * 0x9E3779B1u;
        h ^= h >> 15u; h *= 0x85EBCA77u; h ^= h >> 13u;
        let c = vec3<f32>(f32(h & 255u), f32((h >> 8u) & 255u), f32((h >> 16u) & 255u)) / 255.0;
        return select(vec4<f32>(vec3<f32>(g * 0.5), 1.0), vec4<f32>(mix(color.rgb, c, 0.7), 1.0), kind != 0u);
    }
    // Kinds: confirmed (all corners visible) green, partial (an occluded corner) yellow,
    // carried over grey.
    var c = vec3<f32>(g * 0.5);
    if (kind == 1u) { c = mix(vec3<f32>(g), vec3<f32>(0.2, 0.9, 0.3), 0.6); }
    if (kind == 2u) { c = mix(vec3<f32>(g), vec3<f32>(1.0, 0.85, 0.1), 0.7); }
    return vec4<f32>(c, 1.0);
}
