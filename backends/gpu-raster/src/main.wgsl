// GPU-raster back end: the geometry component's instances are drawn with vertex pulling
// (`g_face` / `g_size` / `g_corner` place each vertex); the visibility component supplies
// the fragment (and compute) entry points, built on `face_hit`; the shading component
// colours the hit (`shade_surface`), with shadow rays through the composed storage.

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

// Quads merged across block types carry this block id; the hit looks the block up.
const LOOKUP_BLOCK: u32 = 0xffffu;

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

// The hit of ray `d` (from the camera) on instance `ii`'s `face` (`face_plane_hit`). Quads
// merged across block types look their block up in the cell kept inside the quad (at
// silhouettes the analytic hit can land a hair outside it); the texel uses the unclamped
// cell, which matched the reference better.
fn face_hit(ii: u32, face: u32, d: vec3<f32>) -> Hit {
    let q = quads[ii];
    let ax = face_axes(face);
    let base = quad_base(q, face);
    let size = g_size(q);
    var h = face_plane_hit(face, base[ax.x], d, base);
    h.block = q.y & 0xffffu;
    if (h.block == LOOKUP_BLOCK) {
        let p = fr.cam_frac + d * h.t;
        var cell = h.cell;
        cell[ax.y] = i32(clamp(floor(p[ax.y]), base[ax.y], base[ax.y] + size.x - 1.0));
        cell[ax.z] = i32(clamp(floor(p[ax.z]), base[ax.z], base[ax.z] + size.y - 1.0));
        h.block = s_block_at(cell + vp.cam_block);
    }
    return h;
}
