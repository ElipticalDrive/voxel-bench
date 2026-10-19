// The ray-casting executor's view of the frame: the camera uniform, the
// output and the unit primary ray through a pixel; the rays' pass (`cast.wgsl`) and the
// deferred shading pass (`records.wgsl`, also on another GPU than the rays) read it.

struct Camera {
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, mode: u32,      // 0 unlit, 1 lit
    origin: vec3<f32>, sky: u32,   // camera position relative to vp.cam_block; packed sRGB sky
    width: u32, height: u32,
    view_max: u32,                 // steps view: the top of the colour scale
    ray_flags: u32,                // ray queries: primary-ray culling
}

@group(0) @binding(0) var<uniform> cam: Camera;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;
// Bindings 2 and 3: the shading state (`sp`, `accum`; shading::state); 4: the hit records
// (`placement = deferred`); from 8: the lane's.

// The unit primary ray through pixel `px` (also the deferred pass's view).
fn view_ray(px: vec2<u32>) -> vec3<f32> {
    return camera_ray(cam.forward, cam.right, cam.up, cam.tan_half_fov, cam.aspect, vec2<f32>(px) + 0.5, vec2<f32>(f32(cam.width), f32(cam.height)));
}

