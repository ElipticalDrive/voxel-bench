// Post: ACES filmic, Narkowicz's fit (components/src/post, `Tone::Aces`).
fn display(c: vec3<f32>) -> vec4<f32> {
    let x = max(c, vec3<f32>(0.0));
    return encode_srgb((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14));
}
