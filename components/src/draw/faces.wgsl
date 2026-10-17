// The face drawer (IDEAS §16 step 2): what every rasteriser of block faces shares
// (gpu-raster, gpu-discover, cpu-discover): the vertex stage, the pixel's ray and the
// analytic surface point. The fragment and compute entry points come from the raster
// visibility components (`raster`, `visbuffer`, `gbuffer`), built on `face_hit`.
//
// Expects the back end's frame uniform `fr` (`view_proj`, `cam_frac` = camera within
// vp.cam_block, `forward`, `right`, `up`, `tan_half_fov`, `aspect`, `size` in pixels,
// `mode`) and a face source, which owns the face records:
// * `src_vertex(ii, vi) -> FaceVertex`: vertex `vi` of instance `ii`;
// * `src_origin(ii, face) -> vec3<f32>`: the face's origin (`face_origin`, camera-block
//   relative), on its plane;
// * `src_block(ii) -> u32`: the instance's block, or `LOOKUP_BLOCK` if it spans several
//   (merged quads), and then `src_lookup(ii, face, h, d) -> u32`: the block at hit `h` of ray `d`;
// * `src_view(ii, color) -> vec4<f32>`: the source's diagnostic views (or `color`).
// Compute passes read the records (`face_hit`). Fragment shaders (`fragment_hit`) either
// read them too (`faces_records.wgsl`: gpu-raster, whose cubes pay 8-15% for extra
// vertex outputs) or take the plane and the block from flat varyings and read a record
// only for merged quads (`faces_varyings.wgsl`: the discovery back ends, whose unlit
// fragments are too cheap for a record read); `visibility::faces_wgsl` picks one.

// Instances spanning several blocks carry this block id; the hit looks the block up.
const LOOKUP_BLOCK: u32 = 0xffffu;

// A vertex: camera-block-relative position, the face it belongs to and its block (or
// `LOOKUP_BLOCK`).
struct FaceVertex { rel: vec3<f32>, face: u32, block: u32 }

// The fragment's surface point from its depth (`pos.w` = 1 / view depth): the `near`
// point of `face_plane_hit` in fragment shaders, used where the pixel's ray runs along the face
// plane (rows at the horizon) and the analytic hit is ill-conditioned. Free, unlike an
// interpolated position (a varying cost gpu-raster's cubes 8-16%).
fn fragment_point(pos: vec4<f32>, d: vec3<f32>) -> vec3<f32> {
    return fr.cam_frac + d * (1.0 / (pos.w * dot(d, fr.forward)));
}

// The face's corner nearest the origin, on the face plane: the block at `cell` (camera-block
// relative), moved to its far side for faces looking along +axis.
fn face_origin(cell: vec3<f32>, face: u32) -> vec3<f32> {
    var p = cell;
    if ((face & 1u) == 0u) { p[face_axes(face).x] += 1.0; }
    return p;
}

// A corner of a quad spanning `size` blocks along the face's (u, v) axes from `origin`;
// `c` = (0|1, 0|1).
fn face_corner(origin: vec3<f32>, face: u32, size: vec2<f32>, c: vec2<f32>) -> vec3<f32> {
    let ax = face_axes(face);
    var p = origin;
    p[ax.y] += c.x * size.x;
    p[ax.z] += c.y * size.y;
    return p;
}

// The primary ray through pixel position `px` (pixel centres at +0.5), built exactly like
// the ray casters build theirs.
fn pixel_ray(px: vec2<f32>) -> vec3<f32> {
    return camera_ray(fr.forward, fr.right, fr.up, fr.tan_half_fov, fr.aspect, px, frame_size());
}

// The deferred shading pass's view of the frame (`shading/deferred.wgsl`).
fn view_ray(px: vec2<u32>) -> vec3<f32> { return pixel_ray(vec2<f32>(px) + 0.5); }
fn view_origin() -> vec3<f32> { return fr.cam_frac; }
fn view_miss(px: vec2<u32>) -> vec4<f32> { return unpack4x8unorm(fr.sky); }
fn view_lit() -> bool { return fr.mode == 1u; }
fn view_surface(s: Surface) -> Surface { return s; }

// The hit of primary ray `d` on the plane of `face` at `plane` (its coordinate along the
// face's normal axis, camera-block relative): analytic, rather than interpolated
// attributes, so texel selection matches the reference even far away. `near` (a point on
// the face) stands in when the ray runs along the plane. The block is left 0 for the
// caller.
fn face_plane_hit(face: u32, plane: f32, d: vec3<f32>, near: vec3<f32>) -> Hit {
    let ax = face_axes(face);
    var h: Hit;
    h.hit = true;
    h.axis = ax.x;
    h.block = 0u;
    h.t = length(near - fr.cam_frac);
    if (abs(d[ax.x]) > 1e-6) { h.t = (plane - fr.cam_frac[ax.x]) / d[ax.x]; }
    let p = fr.cam_frac + d * h.t;
    h.cell = vec3<i32>(floor(p));
    h.cell[ax.x] = i32(round(plane)) - select(0, 1, (face & 1u) == 0u);
    return h;
}

// The surface of hit `h` on the drawn `face`. Unlike `surface_of`, which infers the face
// from the ray's direction, it keeps the face the rasteriser drew: at a few grazing pixels
// the two disagree, and the drawn face is the one the reference sees.
fn face_surface(h: Hit, face: u32, d: vec3<f32>) -> Surface {
    var s = surface_of(h, fr.cam_frac, d);
    s.face = face;
    s.n = face_normal(face);
    return s;
}

// The hit of primary ray `d` on instance `ii`'s `face`, from its record (compute passes).
fn face_hit(ii: u32, face: u32, d: vec3<f32>) -> Hit {
    let origin = src_origin(ii, face);
    var h = face_plane_hit(face, origin[face_axes(face).x], d, origin);
    h.block = src_block(ii);
    if (h.block == LOOKUP_BLOCK) { h.block = src_lookup(ii, face, h, d); }
    return h;
}
