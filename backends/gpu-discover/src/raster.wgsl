// gpu-discover raster: one instanced quad per discovered face (list built on the GPU).
// The shading component colours each fragment (`shade_surface`); lit-mode shadow rays walk the
// composed storage (group 2).

struct Frame {
    view_proj: mat4x4<f32>,
    cam_frac: vec3<f32>, mode: u32,   // camera position within vp.cam_block; 0 unlit, 1 lit
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, view: u32,         // view: 0 image, 1 churn (tint faces new this frame), 2 quads (colour per drawn quad)
    size: vec2<f32>, _p1: vec2<f32>,
}

@group(0) @binding(0) var<uniform> fr: Frame;
@group(0) @binding(1) var<storage, read> faces: array<vec4<u32>>; // key, table index, block, fresh | run length << 1 (merged runs; 0 = one face)
@group(0) @binding(2) var<storage, read> chunk_pos: array<vec4<i32>>;

struct VOut {
    @builtin(position) @invariant pos: vec4<f32>,
    @location(0) rel: vec3<f32>,               // relative to vp.cam_block
    @location(1) @interpolate(flat) info: vec2<u32>, // face, block
    @location(2) @interpolate(flat) plane: f32,       // face plane along its normal axis (camera-block relative)
    @location(3) @interpolate(flat) iid: u32,         // list index + 1 (face-id raster)
    @location(4) @interpolate(flat) fresh: u32,       // 1 = new to the face set this frame
    @location(5) @interpolate(flat) run: u32,         // merged run length (0 = a single face)
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
    // A merged run spans its length in cells along u (entries of unmerged lists: 1).
    let run = e.w >> 1u;
    p[ax.y] += cu * f32(max(1u, run));
    p[ax.z] += cv;
    if (run != 0u) {
        // Merged quads meet their neighbours in T-junctions, where pixel centres exactly on
        // the seam can fall between both quads. Grow seam edges by 1/16 px at their
        // distance (well above the rasteriser's 1/256-px snapping). The overlap is
        // coplanar; the depth test and the per-cell block lookup in `fs` resolve it.
        let base = vec3<f32>(chunk_pos[e.x >> 18u].xyz * 32 - vp.cam_block) + p - fr.cam_frac;
        let z = max(dot(base, fr.forward), 1e-3);
        let eps = z * 2.0 * fr.tan_half_fov / fr.size.y / 16.0;
        // Only seam edges (e.y bits: 0 -u, 1 +u, 2 -v, 3 +v); silhouettes stay exact.
        let grow_u = select(e.y & 1u, (e.y >> 1u) & 1u, cu > 0.5);
        let grow_v = select((e.y >> 2u) & 1u, (e.y >> 3u) & 1u, cv > 0.5);
        p[ax.y] += (cu * 2.0 - 1.0) * eps * f32(grow_u);
        p[ax.z] += (cv * 2.0 - 1.0) * eps * f32(grow_v);
    }
    let rel = vec3<f32>(chunk_pos[e.x >> 18u].xyz * 32 - vp.cam_block) + p;
    var out: VOut;
    out.pos = fr.view_proj * vec4<f32>(rel - fr.cam_frac, 1.0);
    out.rel = rel;
    out.info = vec2<u32>(face, e.z);
    out.plane = rel[ax.x];
    out.iid = ii + 1u;
    out.fresh = e.w & 1u;
    out.run = run;
    return out;
}

// The analytic surface point of this pixel on the face (`face_plane_hit`).
@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    let face = in.info.x;
    let d = pixel_ray(in.pos.xy);
    var h = face_plane_hit(face, in.plane, d, in.rel);
    h.block = in.info.y;
    if (in.run != 0u) {
        // Merged quad: take the block of the cell actually under this point (IDEAS §2), so
        // seams and the T-junction overlap texture exactly like a ray caster would.
        let b = s_block_at(h.cell + vp.cam_block);
        if (b != 0u) { h.block = b; }
    }
    var color = shade_surface(face_surface(h, face, d), d, fr.mode != 0u, vec2<u32>(in.pos.xy));
    if (fr.view == 2u) {
        // Quads: one colour per drawn instance (a face, or a merged run of faces).
        var h = in.iid * 0x9E3779B1u;
        h ^= h >> 15u; h *= 0x85EBCA77u; h ^= h >> 13u;
        let c = vec3<f32>(f32(h & 255u), f32((h >> 8u) & 255u), f32((h >> 16u) & 255u)) / 255.0;
        color = vec4<f32>(mix(color.rgb, c, 0.75), 1.0);
    }
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

// `view = overdraw`: every rasterised fragment adds 1/255 to red (additive blending), so
// red counts how many times each pixel was shaded (up to 255); the overlay pass colours it.
@fragment
fn fs_overdraw(in: VOut) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0 / 255.0, 0.0, 0.0, 1.0);
}
