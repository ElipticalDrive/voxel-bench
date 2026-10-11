// cpu-discover: vertex-pulled unit face quads (4-vertex strips, one instance per discovered face).
// The shading component colours each fragment (`shade_surface`); lit-mode shadow rays walk
// the voxel world (group 1).

struct Frame {
    view_proj: mat4x4<f32>,
    cam_frac: vec3<f32>, mode: u32,   // camera position within vp.cam_block; 0 unlit, 1 lit
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, view: u32,        // 0 image, 1 clusters, 2 kinds (cluster discovery)
    size: vec2<f32>, _p1: vec2<f32>,
    anchor: vec4<i32>,               // absolute block the face records are relative to
}

@group(0) @binding(0) var<uniform> fr: Frame;
@group(0) @binding(1) var<storage, read> quads: array<vec2<u32>>;
@group(0) @binding(2) var<storage, read> tags: array<u32>;  // views: kind << 30 | cluster id

// (normal axis, u axis, v axis) per face; must match mesher::FACE_AXES.
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

struct VOut {
    @builtin(position) @invariant pos: vec4<f32>,
    @location(0) rel: vec3<f32>,               // relative to vp.cam_block
    @location(1) @interpolate(flat) info: vec2<u32>, // face, block
    @location(2) @interpolate(flat) plane: f32,       // face plane along its normal axis (camera-block relative)
    @location(3) @interpolate(flat) tag: u32,
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    let q = quads[ii];
    // Record: x(15) | z(15) << 15 | face.lo(2) << 30 ; block(16) | y(15) << 16 | face.hi(1) << 31
    let face = (q.x >> 30u) | ((q.y >> 31u) << 2u);
    let ax = face_axes(face);
    let cu = f32(vi & 1u);
    let cv = f32(vi >> 1u);
    let cell = vec3<i32>(i32(q.x & 0x7fffu), i32((q.y >> 16u) & 0x7fffu), i32((q.x >> 15u) & 0x7fffu)) - 16384;
    var p = vec3<f32>(0.0);
    if ((face & 1u) == 0u) { p[ax.x] += 1.0; }
    p[ax.y] += cu;
    p[ax.z] += cv;
    let rel = vec3<f32>(fr.anchor.xyz + cell - vp.cam_block) + p;
    var out: VOut;
    out.pos = fr.view_proj * vec4<f32>(rel - fr.cam_frac, 1.0);
    out.rel = rel;
    out.info = vec2<u32>(face, q.y & 0xffffu);
    out.plane = rel[ax.x];
    out.tag = select(0u, tags[ii], fr.view != 0u);
    return out;
}

// Analytic surface point: intersect this pixel's primary ray (built exactly like the ray
// casters build theirs) with the face plane, instead of trusting interpolated attributes.
// Makes texel selection match the reference even on texel boundaries far away.
@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    let face = in.info.x;
    let ax = face_axes(face);
    let sx = (2.0 * in.pos.x / fr.size.x - 1.0) * fr.tan_half_fov * fr.aspect;
    let sy = (1.0 - 2.0 * in.pos.y / fr.size.y) * fr.tan_half_fov;
    let d = normalize(fr.forward + sx * fr.right + sy * fr.up);
    var hit = in.rel;
    if (abs(d[ax.x]) > 1e-6) {
        hit = fr.cam_frac + d * ((in.plane - fr.cam_frac[ax.x]) / d[ax.x]);
    }
    var local = vec3<f32>(0.0);
    local[ax.x] = select(0.0, 1.0, (face & 1u) == 0u);
    local[ax.y] = fract(hit[ax.y]);
    local[ax.z] = fract(hit[ax.z]);
    var color = shade_surface(Surface(hit, face_normal(face), local, in.info.y, face), d, fr.mode != 0u, vec2<u32>(in.pos.xy));
    let kind = in.tag >> 30u;
    if (fr.view == 1u) {
        // Clusters: one colour per cluster (grey for faces carried over from earlier passes).
        var h = (in.tag & 0x3fffffffu) * 0x9E3779B1u;
        h ^= h >> 15u; h *= 0x85EBCA77u; h ^= h >> 13u;
        let c = vec3<f32>(f32(h & 255u), f32((h >> 8u) & 255u), f32((h >> 16u) & 255u)) / 255.0;
        let g = dot(color.rgb, vec3<f32>(0.3, 0.5, 0.2));
        color = select(vec4<f32>(vec3<f32>(g * 0.5), 1.0), vec4<f32>(mix(color.rgb, c, 0.7), 1.0), kind != 0u);
    } else if (fr.view == 2u) {
        // Kinds: confirmed (all corners visible) green, partial (an occluded corner) yellow,
        // carried over grey.
        let g = dot(color.rgb, vec3<f32>(0.3, 0.5, 0.2));
        var c = vec3<f32>(g * 0.5);
        if (kind == 1u) { c = mix(vec3<f32>(g), vec3<f32>(0.2, 0.9, 0.3), 0.6); }
        if (kind == 2u) { c = mix(vec3<f32>(g), vec3<f32>(1.0, 0.85, 0.1), 0.7); }
        color = vec4<f32>(c, 1.0);
    }
    return color;
}
