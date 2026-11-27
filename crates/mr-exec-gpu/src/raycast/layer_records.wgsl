// The layered hit records (`LAYERS`, transparency): `LAYER_SLOTS` records per pixel, front to
// back (the layer index in bits 24-26, bit 27 on an exit face), unused ones the sky. Written
// by the rays' `pixel_layer_records` (`layers.wgsl`); with `DEFERRED_LAYERS` the pass
// `shade_layers` composites them, on the GPU that cast them or where they arrived (another
// GPU, or past a gate: the remote shading pass).

const LAYER_SLOTS: u32 = 8u;

#ifdef DEFERRED_LAYERS
// The deferred pass with layers: each pixel's records composited front to back as
// `pixel_layers` does inline.
@compute @workgroup_size(8, 8)
fn shade_layers(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(out_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }
    let px = gid.xy;
    let at = (px.y * cam.width + px.x) * LAYER_SLOTS;
    if (!record_hit(records[at])) {
        textureStore(out_tex, vec2<i32>(px), view_miss(px));
        return;
    }
    let d = view_ray(px);
    let lit = view_lit();
    var c = vec3<f32>(0.0);
    var left = 1.0;
    for (var i = 0u; i < LAYER_SLOTS; i++) {
        let r = records[at + i];
        if (!record_hit(r)) { break; }
#ifdef MESHES
        if (record_is_mesh(r)) {
            let m = record_mesh(r, d);
            if (i == 0u) {
                textureStore(out_tex, vec2<i32>(px), shade_mesh(m, d, lit, px));
                return;
            }
            c += radiance_point(m.color, m.p, m.n, lit) * left;
            left = 0.0;
            break;
        }
#endif
        let exit = (r.y & (1u << 27u)) != 0u;
        var s = record_decode(r, d);
        if (exit) { s.n = -s.n; }
        let a = select(1.0, surface_coverage(s), block_see_through(s.block));
        if (i == 0u && a >= 1.0 && !exit) {
            textureStore(out_tex, vec2<i32>(px), shade_surface(s, d, lit, px));
            return;
        }
        c += shade_radiance(s, d, lit, px) * (left * a);
        left *= 1.0 - a;
        if (left < 1e-3) { break; }
    }
    if (left >= 1e-3) { c += sky_linear() * left; }
    textureStore(out_tex, vec2<i32>(px), display(c));
}
#endif
