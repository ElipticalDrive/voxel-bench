// The face-drawing executor's frame: the core's camera uniform (group 0, binding 0), shared by every face source: the GPU face set's list (`set_faces.wgsl`), the CPU
// sampler's records (`records.wgsl`) and meshed chunks (`chunks.wgsl`), their two buffers at
// bindings 8 and 9.

struct Frame {
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, mode: u32,         // 0 unlit, 1 lit
    cam_frac: vec3<f32>, sky: u32,    // camera position within vp.cam_block; packed sRGB sky (deferred drawing: uncovered pixels)
    width: u32, height: u32,
    view: u32, _lane: u32,            // the face source's diagnostic tint (0 = the image)
    view_proj: mat4x4<f32>,
    anchor: vec4<i32>,                // CPU records: the absolute block they are relative to
}

@group(0) @binding(0) var<uniform> fr: Frame;

// The image's size in pixels.
fn frame_size() -> vec2<f32> {
    return vec2<f32>(f32(fr.width), f32(fr.height));
}
