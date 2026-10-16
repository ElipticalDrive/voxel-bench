// Deferred shading (PLAN step 12), the same for every visibility strategy: one compute
// thread per pixel reads the hit record the strategy wrote (`pack_hit`), rebuilds the
// surface (`record_surface`) and shades it with the shading component, once per pixel.
//
// Expects `HITS_GROUP` (the group of the records, binding 0, and the output, binding 1) and
// the back end's view of the frame: `view_ray(px)` (the pixel's unit primary ray),
// `view_origin()`, `view_sky()` and `view_lit()`.

@group(HITS_GROUP) @binding(0) var hits_tex: texture_2d<u32>;
@group(HITS_GROUP) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8)
fn shade(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(out_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }
    let r = textureLoad(hits_tex, vec2<i32>(gid.xy), 0).xy;
    if (r.y == 0u) {
        textureStore(out_tex, vec2<i32>(gid.xy), view_sky());
        return;
    }
    let d = view_ray(gid.xy);
    textureStore(out_tex, vec2<i32>(gid.xy), shade_surface(record_surface(r, view_origin(), d), d, view_lit(), gid.xy));
}
