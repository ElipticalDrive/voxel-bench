// gpu-visbuffer: pass 1 rasterises greedy quads writing only (global quad index + 1) and depth;
// pass 2 (compute) reads the id, refetches the quad, intersects the pixel ray with its plane and shades.

struct Frame {
    view_proj: mat4x4<f32>,
    cam_frac: vec3<f32>, mode: u32,
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, _p0: u32,
    size: vec2<f32>, _p1: vec2<f32>,
    sky: vec4<f32>,
}

@group(0) @binding(0) var<uniform> fr: Frame;
@group(0) @binding(1) var<storage, read> quads: array<vec2<u32>>;
@group(0) @binding(2) var<storage, read> chunk_pos: array<vec4<i32>>;

@group(2) @binding(0) var vis_tex: texture_2d<u32>;
@group(2) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;

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

// Quad corner (camera-block relative) at (cu, cv) in quad units.
fn quad_corner(q: vec2<u32>, cu: f32, cv: f32) -> vec3<f32> {
    let face = (q.x >> 25u) & 7u;
    let ax = face_axes(face);
    var p = vec3<f32>(f32(q.x & 31u), f32((q.x >> 5u) & 31u), f32((q.x >> 10u) & 31u));
    if ((face & 1u) == 0u) { p[ax.x] += 1.0; }
    p[ax.y] += cu;
    p[ax.z] += cv;
    return vec3<f32>(chunk_pos[q.y >> 16u].xyz * 32 - vp.cam_block) + p;
}

struct VOut {
    @builtin(position) @invariant pos: vec4<f32>,
    @location(0) @interpolate(flat) id: u32,
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    let q = quads[ii];
    let w = f32(((q.x >> 15u) & 31u) + 1u);
    let h = f32(((q.x >> 20u) & 31u) + 1u);
    let rel = quad_corner(q, f32(vi & 1u) * w, f32(vi >> 1u) * h);
    var out: VOut;
    out.pos = fr.view_proj * vec4<f32>(rel - fr.cam_frac, 1.0);
    out.id = ii + 1u;
    return out;
}

@fragment
fn fs(in: VOut) -> @location(0) u32 {
    return in.id;
}

@compute @workgroup_size(8, 8)
fn shade(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(out_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }
    let id = textureLoad(vis_tex, vec2<i32>(gid.xy), 0).x;
    if (id == 0u) {
        textureStore(out_tex, vec2<i32>(gid.xy), fr.sky);
        return;
    }
    let q = quads[id - 1u];
    let face = (q.x >> 25u) & 7u;
    let ax = face_axes(face);
    let corner = quad_corner(q, 0.0, 0.0);
    let plane = corner[ax.x];
    let px = vec2<f32>(gid.xy) + 0.5;
    let sx = (2.0 * px.x / fr.size.x - 1.0) * fr.tan_half_fov * fr.aspect;
    let sy = (1.0 - 2.0 * px.y / fr.size.y) * fr.tan_half_fov;
    let d = normalize(fr.forward + sx * fr.right + sy * fr.up);
    var hit = corner;
    if (abs(d[ax.x]) > 1e-6) {
        hit = fr.cam_frac + d * ((plane - fr.cam_frac[ax.x]) / d[ax.x]);
    }
    // Clamp into the quad so texel selection never leaks past its edge (silhouette pixels).
    let w = f32(((q.x >> 15u) & 31u) + 1u);
    let h = f32(((q.x >> 20u) & 31u) + 1u);
    hit[ax.y] = clamp(hit[ax.y], corner[ax.y] + 1e-4, corner[ax.y] + w - 1e-4);
    hit[ax.z] = clamp(hit[ax.z], corner[ax.z] + 1e-4, corner[ax.z] + h - 1e-4);
    var local = vec3<f32>(0.0);
    local[ax.x] = select(0.0, 1.0, (face & 1u) == 0u);
    local[ax.y] = fract(hit[ax.y]);
    local[ax.z] = fract(hit[ax.z]);
    // Block id is per quad (greedy merges same-block faces only).
    let texel = surface_texel(q.y & 0xffffu, face, local);
    var c = texel;
    if (fr.mode != 0u) { c = shade_lit(texel, hit, face_normal(face)); }
    textureStore(out_tex, vec2<i32>(gid.xy), c);
}
