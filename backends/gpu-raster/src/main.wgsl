// GPU-raster back end: the geometry component's instances are drawn with vertex pulling
// (`g_face` / `g_size` / `g_corner` place each vertex); the visibility component supplies
// the fragment (and compute) entry points, built on `face_hit`; the shading component
// colours the hit (`shade_hit`), with shadow rays through the composed storage.

struct Frame {
    view_proj: mat4x4<f32>,
    cam_frac: vec3<f32>, mode: u32,   // camera position within vp.cam_block; 0 unlit, 1 lit
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, _p0: u32,
    size: vec2<f32>, _p1: vec2<f32>,
    sky: vec4<f32>,
}

@group(0) @binding(0) var<uniform> fr: Frame;
@group(0) @binding(1) var<storage, read> quads: array<vec2<u32>>;
@group(0) @binding(2) var<storage, read> chunk_pos: array<vec4<i32>>;
// Loop iterations of the shadow walk (the visibility walks count them; unused here).
var<private> ct_iters: u32;

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

// Quads merged across block types carry this block id; the hit looks the block up.
const LOOKUP_BLOCK: u32 = 0xffffu;

// Block at a camera-relative cell, read from the voxel world.
fn block_at(rel_cell: vec3<i32>) -> u32 {
    let abs_cell = rel_cell + vp.cam_block;
    let val = lookup_chunk(chunk_key(abs_cell >> vec3<u32>(5u), 0u));
    if (val == EMPTY) { return 0u; }
    if ((val & UNIFORM_BIT) != 0u) { return val & 0xffffu; }
    return voxel(val, abs_cell & vec3<i32>(31));
}

// Minimum corner of instance `q`'s `face` (camera-block relative), on the face plane.
fn quad_base(q: vec2<u32>, face: u32) -> vec3<f32> {
    var p = vec3<f32>(f32(q.x & 31u), f32((q.x >> 5u) & 31u), f32((q.x >> 10u) & 31u));
    if ((face & 1u) == 0u) { p[face_axes(face).x] += 1.0; }
    return vec3<f32>(chunk_pos[q.y >> 16u].xyz * 32 - vp.cam_block) + p;
}

struct VOut {
    @builtin(position) @invariant pos: vec4<f32>,
    @location(0) @interpolate(flat) info: vec2<u32>, // face, instance
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    let q = quads[ii];
    let face = g_face(q, vi);
    let ax = face_axes(face);
    let size = g_size(q);
    let c = g_corner(vi);
    var rel = quad_base(q, face);
    rel[ax.y] += c.x * size.x;
    rel[ax.z] += c.y * size.y;
    var out: VOut;
    out.pos = fr.view_proj * vec4<f32>(rel - fr.cam_frac, 1.0);
    out.info = vec2<u32>(face, ii);
    return out;
}

// The primary ray through pixel position `px` (pixel centres at +0.5), built exactly like
// the ray casters build theirs.
fn pixel_ray(px: vec2<f32>) -> vec3<f32> {
    let sx = (2.0 * px.x / fr.size.x - 1.0) * fr.tan_half_fov * fr.aspect;
    let sy = (1.0 - 2.0 * px.y / fr.size.y) * fr.tan_half_fov;
    return normalize(fr.forward + sx * fr.right + sy * fr.up);
}

// The hit of ray `d` (from the camera) on instance `ii`'s `face`: analytic intersection
// with the face plane rather than interpolated attributes, so texel selection matches the
// reference even far away. Quads merged across block types look their block up in the
// cell kept inside the quad (at silhouettes the analytic hit can land a hair outside it);
// the texel uses the unclamped cell, which matched the reference better.
fn face_hit(ii: u32, face: u32, d: vec3<f32>) -> Hit {
    let q = quads[ii];
    let ax = face_axes(face);
    let base = quad_base(q, face);
    let size = g_size(q);
    let plane = base[ax.x];
    var h: Hit;
    h.hit = true;
    h.axis = ax.x;
    h.lod = 0u;
    h.t = length(base - fr.cam_frac);
    if (abs(d[ax.x]) > 1e-6) { h.t = (plane - fr.cam_frac[ax.x]) / d[ax.x]; }
    let p = fr.cam_frac + d * h.t;
    h.cell[ax.x] = i32(round(plane)) - select(0, 1, (face & 1u) == 0u);
    h.cell[ax.y] = i32(floor(p[ax.y]));
    h.cell[ax.z] = i32(floor(p[ax.z]));
    h.block = q.y & 0xffffu;
    if (h.block == LOOKUP_BLOCK) {
        var cell = h.cell;
        cell[ax.y] = i32(clamp(floor(p[ax.y]), base[ax.y], base[ax.y] + size.x - 1.0));
        cell[ax.z] = i32(clamp(floor(p[ax.z]), base[ax.z], base[ax.z] + size.y - 1.0));
        h.block = block_at(cell);
    }
    return h;
}
