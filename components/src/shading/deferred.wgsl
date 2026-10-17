// Deferred shading (PLAN step 12), the same for every visibility strategy: one compute
// thread per pixel reads the hit record the strategy stored, decodes it into the surface
// and shades it with the shading component, once per pixel.
//
// A record's encoding is the strategy's: the common packed record (`pack_hit`, decoded by
// `packed.wgsl`), or an id its source decodes (`visbuffer`'s face ids, `splat`'s point
// ids). Expects, from the encoding: `record_at(px)` (the pixel's record, wherever the
// strategy stored it), `record_hit(r)`, `record_decode(r, d)` (the surface on primary ray
// `d`) and `record_view(r, color)` (the source's diagnostic views, or `color`); from the
// back end, its view of the frame: `view_ray(px)` (the pixel's unit primary ray),
// `view_origin()`, `view_miss(px)` (the colour where no surface was hit: the sky, or what
// the back end composites there), `view_lit()`, and the output `out_tex`. With meshes
// (shader def `MESHES`, the record's mesh capability), also `record_is_mesh(r)` and
// `record_mesh(r, d)` (the `MeshSurface` on ray `d`); mesh surfaces are shaded by
// `shade_mesh` (`mr::mesh_surface`).

@compute @workgroup_size(8, 8)
fn shade(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(out_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }
    let r = record_at(gid.xy);
    if (!record_hit(r)) {
        textureStore(out_tex, vec2<i32>(gid.xy), view_miss(gid.xy));
        return;
    }
    let d = view_ray(gid.xy);
#ifdef MESHES
    if (record_is_mesh(r)) {
        textureStore(out_tex, vec2<i32>(gid.xy), shade_mesh(record_mesh(r, d), view_lit()));
        return;
    }
#endif
    textureStore(out_tex, vec2<i32>(gid.xy), record_view(r, shade_surface(record_decode(r, d), d, view_lit(), gid.xy)));
}
