// Reference lighting model on top of the voxel world bindings. Expects the including
// shader (or an occlusion module) to provide `fn occluded(p, dir, max_t) -> bool` and, with
// shader def `LAYERS` (transparency), `fn transmit(p, dir, max_t) -> f32` (the light a
// shadow ray carries through see-through blocks).

// Reference direct lighting at camera-relative surface point `p` (already offset along
// the normal) with normal `n`: ambient + sun + point lights, with shadow rays per settings.
fn direct_light(p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    var light = vp.ambient;
    if (vp.has_sun != 0u) {
        let ndl = dot(n, vp.sun_dir);
#ifdef LAYERS
        if (ndl > 0.0) {
            var v = 1.0;
            if (vp.shadows != 0u) { v = transmit(p, vp.sun_dir, vp.max_dist); }
            light += vp.sun_color * (ndl * v);
        }
#else
        if (ndl > 0.0 && (vp.shadows == 0u || !occluded(p, vp.sun_dir, vp.max_dist))) {
            light += vp.sun_color * ndl;
        }
#endif
    }
    let lc = lookup_light_cell((vec3<i32>(floor(p)) + vp.cam_block) >> vec3<u32>(4u));
    for (var i = lc.x; i < lc.x + lc.y; i++) {
        let l = lights[light_idx[i]];
        let to = vec3<f32>(l.block - vp.cam_block) + l.frac - p;
        let dist = length(to);
        if (dist >= l.radius || dist < 1e-6) { continue; }
        let dir = to / dist;
        let ndl = dot(n, dir);
        if (ndl <= 0.0) { continue; }
#ifdef LAYERS
        var v = 1.0;
        if (vp.shadows == 2u) { v = transmit(p, dir, dist); }
        if (v <= 0.0) { continue; }
        light += l.color * (l.intensity * ndl * attenuation(dist, l.radius) * v);
#else
        if (vp.shadows == 2u && occluded(p, dir, dist)) { continue; }
        light += l.color * (l.intensity * ndl * attenuation(dist, l.radius));
#endif
    }
    return light;
}

fn shade_lit(texel: vec4<f32>, p: vec3<f32>, n: vec3<f32>) -> vec4<f32> {
    let albedo = vec3<f32>(srgb_to_linear(texel.r), srgb_to_linear(texel.g), srgb_to_linear(texel.b));
    return display(albedo * direct_light(p + n * 1e-3, n) * vp.exposure);
}
