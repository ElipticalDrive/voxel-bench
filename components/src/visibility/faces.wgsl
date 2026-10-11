// The face drawer (IDEAS §16 step 2): what every rasteriser of block faces shares
// (gpu-raster, gpu-discover, cpu-discover). Expects the back end's frame uniform `fr` with
// `cam_frac` (camera within vp.cam_block), `forward`, `right`, `up`, `tan_half_fov`,
// `aspect` and `size` (pixels).

// The primary ray through pixel position `px` (pixel centres at +0.5), built exactly like
// the ray casters build theirs.
fn pixel_ray(px: vec2<f32>) -> vec3<f32> {
    let sx = (2.0 * px.x / fr.size.x - 1.0) * fr.tan_half_fov * fr.aspect;
    let sy = (1.0 - 2.0 * px.y / fr.size.y) * fr.tan_half_fov;
    return normalize(fr.forward + sx * fr.right + sy * fr.up);
}

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
    h.lod = 0u;
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
