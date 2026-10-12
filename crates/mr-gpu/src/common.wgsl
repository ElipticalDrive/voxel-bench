// Shared WGSL mirroring mr_core conventions (prepended to renderer shaders).

fn srgb_to_linear(c: f32) -> f32 {
    if (c <= 0.04045) { return c / 12.92; }
    return pow((c + 0.055) / 1.055, 2.4);
}

fn linear_to_srgb(x: f32) -> f32 {
    let c = clamp(x, 0.0, 1.0);
    if (c <= 0.0031308) { return c * 12.92; }
    return 1.055 * pow(c, 1.0 / 2.4) - 0.055;
}

// Quantised exactly like mr_core::lighting::encode_srgb, so rgba8unorm storage reproduces it.
fn encode_srgb(c: vec3<f32>) -> vec4<f32> {
    let q = floor(vec3<f32>(linear_to_srgb(c.x), linear_to_srgb(c.y), linear_to_srgb(c.z)) * 255.0 + 0.5) / 255.0;
    return vec4<f32>(q, 1.0);
}

fn attenuation(dist: f32, radius: f32) -> f32 {
    if (dist >= radius) { return 0.0; }
    let x = dist / radius;
    let w = clamp(1.0 - x * x * x * x, 0.0, 1.0);
    return w * w / (dist * dist + 1.0);
}

// Face index: 0 +X, 1 -X, 2 +Y, 3 -Y, 4 +Z, 5 -Z.
fn face_entered(axis: u32, step_positive: bool) -> u32 {
    return axis * 2u + select(0u, 1u, step_positive);
}

fn face_normal(face: u32) -> vec3<f32> {
    var n = vec3<f32>(0.0);
    n[face / 2u] = select(1.0, -1.0, (face & 1u) == 1u);
    return n;
}

// The (unnormalised) primary ray direction through pixel position `px` (pixel centres at
// +0.5) of a `size` image, for the camera basis `forward`, `right`, `up`, the tangent of
// half the vertical field of view and the aspect ratio. Every back end builds its primary
// rays (and pixel reprojections) with it, so rasterisers and ray casters agree exactly.
fn camera_dir(forward: vec3<f32>, right: vec3<f32>, up: vec3<f32>, tan_half_fov: f32, aspect: f32, px: vec2<f32>, size: vec2<f32>) -> vec3<f32> {
    let sx = (2.0 * px.x / size.x - 1.0) * tan_half_fov * aspect;
    let sy = (1.0 - 2.0 * px.y / size.y) * tan_half_fov;
    return forward + sx * right + sy * up;
}

fn camera_ray(forward: vec3<f32>, right: vec3<f32>, up: vec3<f32>, tan_half_fov: f32, aspect: f32, px: vec2<f32>, size: vec2<f32>) -> vec3<f32> {
    return normalize(camera_dir(forward, right, up, tan_half_fov, aspect, px, size));
}

// (normal axis, u axis, v axis) per face; must match mr_mesh::FACE_AXES.
fn face_axes(face: u32) -> vec3<u32> {
    switch face {
        case 0u: { return vec3<u32>(0u, 1u, 2u); }
        case 1u: { return vec3<u32>(0u, 2u, 1u); }
        case 2u: { return vec3<u32>(1u, 2u, 0u); }
        case 3u: { return vec3<u32>(1u, 0u, 2u); }
        case 4u: { return vec3<u32>(2u, 0u, 1u); }
        default: { return vec3<u32>(2u, 1u, 0u); }
    }
}

fn face_uv(face: u32, l: vec3<f32>) -> vec2<f32> {
    switch face {
        case 0u: { return vec2<f32>(1.0 - l.z, 1.0 - l.y); }
        case 1u: { return vec2<f32>(l.z, 1.0 - l.y); }
        case 2u: { return vec2<f32>(l.x, l.z); }
        case 3u: { return vec2<f32>(l.x, 1.0 - l.z); }
        case 4u: { return vec2<f32>(l.x, 1.0 - l.y); }
        default: { return vec2<f32>(1.0 - l.x, 1.0 - l.y); }
    }
}

fn texel_coord(u: f32) -> u32 {
    return u32(clamp(i32(floor(u * 16.0)), 0, 15));
}
