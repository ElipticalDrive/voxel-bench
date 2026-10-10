// Shading `direct`: the reference lighting model. Unlit: the face texel. Lit: ambient, sun
// and point lights with shadow rays (`occluded`, i.e. the composed visibility walk).

fn shade_hit(h: Hit, o: vec3<f32>, d: vec3<f32>, lit: bool, px: vec2<u32>) -> vec4<f32> {
    let face = face_entered(h.axis, d[h.axis] > 0.0);
    let p = o + d * h.t;
    // LOD cells span several blocks; textures still tile once per block.
    var local = p - vec3<f32>(h.cell);
    if (h.lod > 0u) { local = p - floor(p); }
    let texel = surface_texel(h.block, face, local);
    // A branch, not `select`: select evaluates both sides, so unlit pixels would pay for
    // lighting and shadow rays whenever the compiler doesn't turn it into a branch.
    if (!lit) { return texel; }
    return shade_lit(texel, p, face_normal(face));
}
