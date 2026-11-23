// The ids -> records adapter of a `visbuffer` drawing whose shading runs on another GPU: one
// compute thread per pixel decodes the face id its source drew into the common hit record
// (`pack_hit`), what crosses between GPUs (8 bytes per pixel against the id's 4); the second
// GPU shades the records like a `gbuffer`'s. The source's diagnostic views do not cross (no
// instance in a record). Expects the visbuffer's `record_*` and `out_tex` (the frame's size),
// the back end's `view_ray(px)` and `view_origin()`, and `record_store(i, r)` (record `r` of
// pixel `i` in row-major order, where the back end keeps its records).

@compute @workgroup_size(8, 8)
fn decode(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(out_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }
    let r = record_at(gid.xy);
    var rec = vec2<u32>(0u);
    if (record_hit(r)) {
        let d = view_ray(gid.xy);
        let s = record_decode(r, d);
        rec = pack_hit(dot(s.p - view_origin(), d), s);
    }
    record_store(gid.y * dims.x + gid.x, rec);
}
