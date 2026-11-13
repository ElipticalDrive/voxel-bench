// Shading `direct`: the reference lighting model. Unlit: the surface's albedo (a voxel
// face's texel, a mesh's vertex colour). Lit: ambient, sun and point lights with shadow
// rays (`occluded`, i.e. the composed visibility walk).

fn shade_point(albedo: vec4<f32>, p: vec3<f32>, n: vec3<f32>, d: vec3<f32>, lit: bool, px: vec2<u32>) -> vec4<f32> {
    // A branch, not `select`: select evaluates both sides, so unlit pixels would pay for
    // lighting and shadow rays whenever the compiler doesn't turn it into a branch.
    if (!lit) { return albedo; }
    return shade_lit(albedo, p, n);
}

fn shade_surface(s: Surface, d: vec3<f32>, lit: bool, px: vec2<u32>) -> vec4<f32> {
    return shade_point(surface_texel(s.block, s.face, s.local), s.p, s.n, d, lit, px);
}
