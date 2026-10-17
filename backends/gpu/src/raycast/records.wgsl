// The hit records of `placement = deferred` and the deferred pass's view of them (the
// common pass `shading/deferred.wgsl` expects these): where the rays' pass wrote them, or
// (PLAN step 13e4c) where they arrived from the GPU that cast the rays.

@group(0) @binding(4) var<storage, read_write> records: array<vec2<u32>>;

fn record_at(px: vec2<u32>) -> vec2<u32> { return records[px.y * cam.width + px.x]; }
fn view_origin() -> vec3<f32> { return cam.origin; }
fn view_lit() -> bool { return cam.mode == 1u; }
fn view_surface(s: Surface) -> Surface { return s; }
fn view_miss(px: vec2<u32>) -> vec4<f32> { return unpack4x8unorm(cam.sky); }
