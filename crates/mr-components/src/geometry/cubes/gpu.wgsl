// Geometry `cubes`: one instance per block, expanded to 36 vertices (6 faces x 2
// triangles); the hardware back-face culls the hidden half.

const G_VERTS: u32 = 36u;

fn g_face(q: vec2<u32>, vi: u32) -> u32 {
    return vi / 6u;
}

fn g_size(q: vec2<u32>) -> vec2<f32> {
    return vec2<f32>(1.0);
}

// Two triangles per face in the winding of the 4-vertex strip: (0, 1, 2), (2, 1, 3).
fn g_corner(vi: u32) -> vec2<f32> {
    let k = vi % 6u;
    let c = select(select(k, 2u, k == 3u), select(1u, 3u, k == 5u), k >= 4u);
    return vec2<f32>(f32(c & 1u), f32(c >> 1u));
}
