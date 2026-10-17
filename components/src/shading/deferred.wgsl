// Deferred shading (PLAN step 12), the same for every visibility strategy: one compute
// thread per pixel reads the hit record the strategy wrote (`pack_hit`), rebuilds the
// surface (`record_surface`) and shades it with the shading component, once per pixel.
//
// Expects the back end's records and view of the frame: `record_at(px)` (the pixel's record,
// wherever the strategy stored it), `view_ray(px)` (its unit primary ray), `view_origin()`,
// `view_miss(px)` (the colour where no surface was hit: the sky, or what the back end
// composites there), `view_lit()`, and the output `out_tex`.

@compute @workgroup_size(8, 8)
fn shade(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(out_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }
    let r = record_at(gid.xy);
    if (r.y == 0u) {
        textureStore(out_tex, vec2<i32>(gid.xy), view_miss(gid.xy));
        return;
    }
    let d = view_ray(gid.xy);
    textureStore(out_tex, vec2<i32>(gid.xy), shade_surface(record_surface(r, view_origin(), d), d, view_lit(), gid.xy));
}
