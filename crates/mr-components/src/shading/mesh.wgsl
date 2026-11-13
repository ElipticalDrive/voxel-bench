// Mesh surfaces (`MeshSurface`, decoded from the triangles by `mr::mesh_surface`) shaded by
// the shading component: the interpolated vertex colour is the albedo of `shade_point`, so
// a mesh pixel is lit as a voxel's is (`direct`'s lights and shadow rays, `pathtrace`'s
// bounces). The shading instance is the shading node's when the mesh-shadow site shares
// its shadow rays' kernel, else one lit through the mesh-shadow site's rays
// (`shaders::meshes`).

fn shade_mesh(m: MeshSurface, d: vec3<f32>, lit: bool, px: vec2<u32>) -> vec4<f32> {
    return shade_point(m.color, m.p, m.n, d, lit, px);
}
