// The discovery executor's frame (group 0, binding 0), shared by both face sources: the
// GPU face set's list (`set_faces.wgsl`) and the CPU sampler's records (`records.wgsl`).

struct Frame {
    view_proj: mat4x4<f32>,
    cam_frac: vec3<f32>, mode: u32,   // camera position within vp.cam_block; 0 unlit, 1 lit
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, view: u32,         // the face source's diagnostic tint (0 = the image)
    size: vec2<f32>, _p1: vec2<f32>,
    anchor: vec4<i32>,                // CPU records: the absolute block they are relative to
    sky: vec4<f32>,                   // deferred drawing: the colour of uncovered pixels
}

@group(0) @binding(0) var<uniform> fr: Frame;
