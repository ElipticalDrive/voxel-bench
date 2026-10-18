// Post: Reinhard on luminance (components/src/post, `Tone::Reinhard`).
fn display(c: vec3<f32>) -> vec4<f32> {
    return encode_srgb(c / (1.0 + dot(c, vec3<f32>(0.2126, 0.7152, 0.0722))));
}
