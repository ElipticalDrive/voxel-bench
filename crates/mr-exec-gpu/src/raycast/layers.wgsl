// Layers (`LAYERS`, transparency; compiled in only once a see-through block has arrived):
// the primary ray continues past see-through blocks, its surfaces found by the layer walk
// (`mr_components::visibility::LAYER_WALK_WGSL`, before this text in the root, over the
// primary kernel and storage; a hit on a LOD cell is drawn opaque). Inline the surfaces are
// composited front to back (OVER in linear light, the shading's `shade_radiance`); deferred,
// `pixel_layer_records` writes `LAYER_SLOTS` records per pixel and `shade_layers` composites
// them.

const LAYER_SLOTS: u32 = 8u;

// The colour of the primary ray `d` through pixel `px` with layers (`h` its kernel hit; mesh
// mode: the mesh at `mt`, behind the layers in front of it): front-to-back OVER in linear light
// (stops once less than 1e-3 of the light behind passes). A ray whose first surface is an entry
// face covering fully is the pixel without layers exactly.
fn pixel_layers(px: vec2<u32>, d: vec3<f32>, mt: f32, h: Hit) -> vec4<f32> {
    let o = cam.origin;
    let lit = cam.mode == 1u;
    var w = layer_walk(o, h);
    var c = vec3<f32>(0.0);
    var left = 1.0;
    var count = 0u;
    for (var i = 0u; i < 1024u; i++) {
        let l = layer_next(&w, o, d, min(vp.max_dist, mt));
        if (!l.ok) { break; }
        let s = layer_surface(l, o, d);
        let a = layer_coverage(l, s);
        if (a <= 0.0) { continue; }
        if (count == 0u && a >= 1.0 && !l.exit) { return shade_surface(s, d, lit, px); }
        count++;
        c += shade_radiance(s, d, lit, px) * (left * a);
        left *= 1.0 - a;
        if (left < 1e-3) { return display(c); }
    }
#ifdef MESHES
    // The walk ended at the mesh: it is behind the layers.
    if (mt < 1e29) {
        let m = mesh_record(px);
        let ms = mesh_surface(record_mesh_tri(m), o, d, bitcast<f32>(m.x));
        if (count == 0u) { return shade_mesh(ms, d, lit, px); }
        return display(c + radiance_point(ms.color, ms.p, ms.n, lit) * left);
    }
#endif
    if (count == 0u) { return unpack_sky(cam.sky); }
    return display(c + sky_linear() * left);
}

#ifdef DEFERRED_LAYERS
// The records of the primary ray `d` through pixel `px` with layers: `LAYER_SLOTS` per pixel,
// front to back (the layer index in bits 24-26, bit 27 on an exit face), unused ones the sky:
// the surfaces `pixel_layers` composites. The last slot holds the first opaque surface behind
// the others (the light the layers past the slots would have stopped is lost).
fn pixel_layer_records(px: vec2<u32>, d: vec3<f32>, mt: f32, h: Hit) {
    let o = cam.origin;
    let at = (px.y * cam.width + px.x) * LAYER_SLOTS;
    var w = layer_walk(o, h);
    var n = 0u;
    var left = 1.0;
    for (var i = 0u; i < 1024u; i++) {
        let l = layer_next(&w, o, d, min(vp.max_dist, mt));
        if (!l.ok) { break; }
        let s = layer_surface(l, o, d);
        let a = layer_coverage(l, s);
        if (a <= 0.0 || (n == LAYER_SLOTS - 1u && a < 1.0)) { continue; }
        let r = pack_hit(l.t, s);
        records[at + n] = vec2<u32>(r.x, r.y | (n << 24u) | select(0u, 1u << 27u, l.exit));
        n++;
        left *= 1.0 - a;
        if (left < 1e-3 || n == LAYER_SLOTS) { break; }
    }
#ifdef MESHES
    if (n < LAYER_SLOTS && left >= 1e-3 && mt < 1e29) {
        records[at + n] = mesh_record(px);
        n++;
    }
#endif
    for (; n < LAYER_SLOTS; n++) { records[at + n] = vec2<u32>(0u); }
}

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
