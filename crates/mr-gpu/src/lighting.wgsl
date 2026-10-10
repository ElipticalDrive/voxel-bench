// Reference lighting model on top of the voxel world bindings. Expects the including
// shader (or an occlusion module) to provide `fn occluded(p, dir, max_t) -> bool`.

// Reference direct lighting at camera-relative surface point `p` (already offset along
// the normal) with normal `n`: ambient + sun + point lights, with shadow rays per settings.
fn direct_light(p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    var light = vp.ambient;
    if (vp.has_sun != 0u) {
        let ndl = dot(n, vp.sun_dir);
        if (ndl > 0.0 && (vp.shadows == 0u || !occluded(p, vp.sun_dir, vp.max_dist))) {
            light += vp.sun_color * ndl;
        }
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
        if (vp.shadows == 2u && occluded(p, dir, dist)) { continue; }
        light += l.color * (l.intensity * ndl * attenuation(dist, l.radius));
    }
    return light;
}

fn shade_lit(texel: vec4<f32>, p: vec3<f32>, n: vec3<f32>) -> vec4<f32> {
    let albedo = vec3<f32>(srgb_to_linear(texel.r), srgb_to_linear(texel.g), srgb_to_linear(texel.b));
    return encode_srgb(albedo * direct_light(p + n * 1e-3, n) * vp.exposure);
}
