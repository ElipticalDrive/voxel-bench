// Geometry `greedy` (and `naive`, whose one-block quads use the same encoding): one
// 4-vertex strip per packed quad; face and extent come from the quad.

const G_VERTS: u32 = 4u;

fn g_face(q: vec2<u32>, vi: u32) -> u32 {
    return (q.x >> 25u) & 7u;
}

fn g_size(q: vec2<u32>) -> vec2<f32> {
    return vec2<f32>(f32(((q.x >> 15u) & 31u) + 1u), f32(((q.x >> 20u) & 31u) + 1u));
}

fn g_corner(vi: u32) -> vec2<f32> {
    return vec2<f32>(f32(vi & 1u), f32(vi >> 1u));
}
