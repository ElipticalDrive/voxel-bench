// Visibility `slices` on the GPU (the root of slices-gpu's program): plane sweeping. Every
// integer plane of the loaded region, along each axis, is one instanced quad spanning the
// region; the rasteriser hands each pixel of a plane to the fragment shader, which keeps it
// only where the plane holds a block face looking at the camera (the cell beyond the plane
// solid, the one on the camera's side air: two lookups in the storage the node reads,
// `s_block_at`) and writes that face's hit record (`pack_hit`) into the hits edge, a render
// target of records, behind the depth test. Planes go out from the camera, nearest first,
// so the depth test rejects most of what is hidden. Then the common deferred pass shades the
// records. No geometry: the storage is the only input.

struct Camera {
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, mode: u32,
    origin: vec3<f32>, sky: u32,
    width: u32, height: u32, planes: u32, _pad: u32,
    view_proj: mat4x4<f32>,
    anchor: vec4<i32>,
}

@group(0) @binding(0) var<uniform> cam: Camera;
#ifdef HDR
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba16float, write>;
#else
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;
#endif

struct VOut {
    @builtin(position) pos: vec4<f32>,
    // The plane's axis and its coordinate (camera-block relative).
    @location(0) @interpolate(flat) plane: vec2<i32>,
}

// The plane coordinate of the `j`-th plane of a range `lo..=hi`, nearest to `c` first.
fn nearest_first(j: i32, lo: i32, hi: i32, c: i32) -> i32 {
    let at = clamp(c, lo, hi);
    let m = min(at - lo, hi - at);
    if (j <= 2 * m) {
        return select(at + j / 2, at - (j + 1) / 2, (j & 1) == 1);
    }
    let r = j - 2 * m;
    return select(at - m - r, at + m + r, hi - at > at - lo);
}

// The planes of each axis: every integer plane of the loaded region, its ends included.
fn planes_on(axis: u32) -> i32 { return vp.bounds_max[axis] - vp.bounds_min[axis] + 1; }

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    var out: VOut;
    var j = i32(ii);
    var axis = 0u;
    for (; axis < 2u; axis++) {
        if (j < planes_on(axis)) { break; }
        j -= planes_on(axis);
    }
    let lo = vp.bounds_min;
    let hi = vp.bounds_max;
    if (j >= planes_on(axis)) {
        out.pos = vec4<f32>(0.0, 0.0, -1.0, 1.0);
        return out;
    }
    let k = nearest_first(j, lo[axis], hi[axis], i32(floor(cam.origin[axis])));
    // The quad's corners over the other two axes (a triangle strip).
    let ax = face_axes(axis * 2u);
    var p = vec3<f32>(0.0);
    p[axis] = f32(k);
    p[ax.y] = f32(select(lo[ax.y], hi[ax.y], (vi & 1u) == 1u));
    p[ax.z] = f32(select(lo[ax.z], hi[ax.z], (vi & 2u) == 2u));
    out.pos = cam.view_proj * vec4<f32>(p - cam.origin, 1.0);
    out.plane = vec2<i32>(i32(axis), k);
    return out;
}

// The face of plane `in.plane` seen along this pixel's ray, as a hit record; discarded where
// the plane holds none facing the camera.
@fragment
fn fs(in: VOut) -> @location(0) vec2<u32> {
    let axis = u32(in.plane.x);
    let k = in.plane.y;
    let d = camera_ray(cam.forward, cam.right, cam.up, cam.tan_half_fov, cam.aspect, in.pos.xy, vec2<f32>(f32(cam.width), f32(cam.height)));
    let o = cam.origin;
    if (d[axis] == 0.0) { discard; }
    let t = (f32(k) - o[axis]) / d[axis];
    if (t <= 0.0) { discard; }
    var p = o + d * t;
    p[axis] = f32(k);
    // The cell beyond the plane (along the ray) and the one before it.
    var cell = vec3<i32>(floor(p));
    let forward = d[axis] > 0.0;
    cell[axis] = select(k - 1, k, forward);
    var before = cell;
    before[axis] = select(k, k - 1, forward);
    let b = s_block_at(cell + vp.cam_block);
    if (b == 0u || s_block_at(before + vp.cam_block) != 0u) { discard; }
    let face = face_entered(axis, forward);
    let local = clamp(p - vec3<f32>(cell), vec3<f32>(0.0), vec3<f32>(0.9999));
    return pack_hit(t, Surface(p, face_normal(face), local, b, face));
}

// The common deferred pass's view of the frame (`shading/deferred.wgsl`): the records
// the planes wrote, decoded by `shading/packed.wgsl`.
@group(0) @binding(8) var rec_tex: texture_2d<u32>;
fn record_at(px: vec2<u32>) -> vec2<u32> { return textureLoad(rec_tex, vec2<i32>(px), 0).xy; }
fn view_ray(px: vec2<u32>) -> vec3<f32> {
    return camera_ray(cam.forward, cam.right, cam.up, cam.tan_half_fov, cam.aspect, vec2<f32>(px) + 0.5, vec2<f32>(f32(cam.width), f32(cam.height)));
}
fn view_origin() -> vec3<f32> { return cam.origin; }
fn view_miss(px: vec2<u32>) -> vec4<f32> { return unpack_sky(cam.sky); }
fn view_lit() -> bool { return cam.mode == 1u; }
fn view_surface(s: Surface) -> Surface { return s; }
