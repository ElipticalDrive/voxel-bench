// The ray caster over the heightmap's march (visibility `raycast` with the `heightfield`
// kernel bound to its primary rays, run by the columns region; appended to the column march's
// root, whose camera, helpers and deferred view it shares). One invocation per pixel casts
// its ray through `hm_trace`: the surface the column march gives the same cell (texture
// coordinates across the column, the heightmap slope on lit tops), shaded inline or, with
// shader def `DEFERRED`, written as a hit record the common pass shades.

@compute @workgroup_size(8, 8)
fn cast_rays(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width || gid.y >= cam.height) { return; }
    let px = gid.xy;
    let o = cam.origin;
    let d = column_ray(f32(px.x) + 0.5, f32(px.y) + 0.5);
    let h = hm_trace(o, d, 3.0e38);
#ifdef DEFERRED
    let i = px.y * cam.width + px.x;
    if (!h.hit) { records[i] = vec2<u32>(0u); return; }
#else
    if (!h.hit) { textureStore(out_tex, vec2<i32>(px), unpack_sky(cam.sky)); return; }
#endif
    let p = o + d * h.t;
    let face = face_entered(h.axis, d[h.axis] > 0.0);
    let cell = h.cell.xz;
    let local = column_local(p, cell);
#ifdef DEFERRED
    records[i] = pack_hit(h.t, Surface(p, face_normal(face), local, h.block, face));
#else
#ifdef LIT
    var n = face_normal(face);
    if (face == 2u) { n = slope_normal(cell); }
    let color = shade_surface(Surface(p, n, local, h.block, face), d, true, px);
#else
    let color = shade_surface(Surface(p, face_normal(face), local, h.block, face), d, false, px);
#endif
    textureStore(out_tex, vec2<i32>(px), color);
#endif
}
