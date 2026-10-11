// Shading `direct`: the reference lighting model. Unlit: the face texel. Lit: ambient, sun
// and point lights with shadow rays (`occluded`, i.e. the composed visibility walk).

fn shade_surface(s: Surface, d: vec3<f32>, lit: bool, px: vec2<u32>) -> vec4<f32> {
    let texel = surface_texel(s.block, s.face, s.local);
    // A branch, not `select`: select evaluates both sides, so unlit pixels would pay for
    // lighting and shadow rays whenever the compiler doesn't turn it into a branch.
    if (!lit) { return texel; }
    return shade_lit(texel, s.p, s.n);
}
