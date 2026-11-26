// Shading `ao`: unlit = the surface's albedo; lit = direct lighting (sun and point lights with
// their shadow rays, `occluded`) with the ambient term scaled by ambient occlusion: the share
// of `sp.spp` cosine-weighted rays per pixel per frame, traced with the composed secondary
// walk (`btrace`), that meet no surface within `sp.reach` blocks; averaged over frames while
// the view is still, as `pathtrace` accumulates (`accum`, `sp.prev_count` samples so far).
// `sp` and `accum` are the shading state (`shading::state`), bound by the back end.

fn shade_point(albedo: vec4<f32>, p: vec3<f32>, n: vec3<f32>, d: vec3<f32>, lit: bool, px: vec2<u32>) -> vec4<f32> {
    if (!lit) { return albedo; }
    let idx = px.y * sp.width + px.x;
    rng_state = rng_pcg(idx ^ rng_pcg(sp.frame * 0x9E3779B9u + 1u));
    let po = p + n * 1e-3;
    var open = 0u;
    for (var i = 0u; i < sp.spp; i++) {
        if (!btrace(po, rng_cosine_dir(n), sp.reach).hit) { open += 1u; }
    }
    // The mean share of open rays so far in x (f32 bits); w: the primary surface's signature
    // (high bits) and the pixel's own sample count (low bits), as `pathtrace` keeps its mean.
    let sig = surface_sig(p, n);
    let old = accum[idx];
    var count = 0u;
    if (sp.prev_count > 0u && (old.w & ACC_SIG_MASK) == sig) { count = old.w & ACC_COUNT_MASK; }
    var share = f32(open) / f32(sp.spp);
    if (count > 0u) {
        share = (bitcast<f32>(old.x) * f32(count) + f32(open)) / f32(count + sp.spp);
    }
    accum[idx] = vec4<u32>(bitcast<u32>(share), 0u, 0u, sig | min(count + sp.spp, ACC_COUNT_MASK));
    let a = vec3<f32>(srgb_to_linear(albedo.r), srgb_to_linear(albedo.g), srgb_to_linear(albedo.b));
    let light = direct_light(po, n) - vp.ambient + vp.ambient * share;
    return display(a * light * vp.exposure);
}

fn shade_surface(s: Surface, d: vec3<f32>, lit: bool, px: vec2<u32>) -> vec4<f32> {
    return shade_point(surface_texel(s.block, s.face, s.local), s.p, s.n, d, lit, px);
}
