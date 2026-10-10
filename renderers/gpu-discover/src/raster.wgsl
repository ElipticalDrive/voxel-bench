// gpu-discover raster: one instanced quad per discovered face (list built on the GPU).
// Lit mode traces shadow rays from the fragment shader through the voxel world (group 1).

struct Frame {
    view_proj: mat4x4<f32>,
    cam_frac: vec3<f32>, mode: u32,   // camera position within vp.cam_block; 0 unlit, 1 lit
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, view: u32,         // view: 0 image, 1 churn (tint faces new this frame)
    size: vec2<f32>, _p1: vec2<f32>,
}

@group(0) @binding(0) var<uniform> fr: Frame;
@group(0) @binding(1) var<storage, read> faces: array<vec4<u32>>; // key, table index, block
@group(0) @binding(2) var<storage, read> chunk_pos: array<vec4<i32>>;

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
    @location(3) @interpolate(flat) iid: u32,         // list index + 1 (face-id raster)
    @location(4) @interpolate(flat) fresh: u32,       // 1 = new to the face set this frame
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    let e = faces[ii];
    let face = e.x & 7u;
    let ax = face_axes(face);
    let cu = f32(vi & 1u);
    let cv = f32(vi >> 1u);
    let l = (e.x >> 3u) & 32767u;
    var p = vec3<f32>(f32(l & 31u), f32((l >> 5u) & 31u), f32((l >> 10u) & 31u));
    if ((face & 1u) == 0u) { p[ax.x] += 1.0; }
    p[ax.y] += cu;
    p[ax.z] += cv;
    let rel = vec3<f32>(chunk_pos[e.x >> 18u].xyz * 32 - vp.cam_block) + p;
    var out: VOut;
    out.pos = fr.view_proj * vec4<f32>(rel - fr.cam_frac, 1.0);
    out.rel = rel;
    out.info = vec2<u32>(face, e.z);
    out.plane = rel[ax.x];
    out.iid = ii + 1u;
    out.fresh = e.w;
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
    let texel = surface_texel(in.info.y, face, local);
    var color = texel;
    if (fr.mode != 0u) { color = shade_lit(texel, hit, face_normal(face)); }
    if (fr.view == 1u) {
        // Churn: faces discovered this frame red, faces carried over greyed.
        let g = dot(color.rgb, vec3<f32>(0.3, 0.5, 0.2));
        color = select(vec4<f32>(vec3<f32>(g * 0.6), 1.0), vec4<f32>(1.0, 0.15, 0.1, 1.0), in.fresh != 0u);
    }
    return color;
}

// Face-id raster (incremental mode): which list entry covers each pixel.
@fragment
fn fs_id(in: VOut) -> @location(0) u32 {
    return in.iid;
}
