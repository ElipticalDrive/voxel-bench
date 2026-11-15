// The deferred pass's view of the hit records of `placement = deferred` (the common pass
// `shading/deferred.wgsl` expects these). The records are the hits edge's resource, attached
// by the executor's declaration (`records`, `record_at`): where the rays' pass wrote them, or
// where they arrived from the GPU that cast the rays.

fn view_origin() -> vec3<f32> { return cam.origin; }
fn view_lit() -> bool { return cam.mode == 1u; }
fn view_surface(s: Surface) -> Surface { return s; }
fn view_miss(px: vec2<u32>) -> vec4<f32> { return unpack4x8unorm(cam.sky); }
