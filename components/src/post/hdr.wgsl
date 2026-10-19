// Post: none, unlimited (components/src/post, `Tone::Hdr`): extended sRGB into the float
// output (`#ifdef HDR`), sRGB's curve continued past 1, unquantised.
fn display(c: vec3<f32>) -> vec4<f32> {
    return vec4<f32>(srgb_extended(c.x), srgb_extended(c.y), srgb_extended(c.z), 1.0);
}

fn srgb_extended(x: f32) -> f32 {
    let c = max(x, 0.0);
    if (c <= 0.0031308) { return c * 12.92; }
    return 1.055 * pow(c, 1.0 / 2.4) - 0.055;
}
