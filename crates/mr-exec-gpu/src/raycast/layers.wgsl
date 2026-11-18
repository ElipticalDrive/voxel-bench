// Layers (`LAYERS`, transparency; compiled in only once a see-through block has arrived):
// the primary ray continues past see-through blocks, its surfaces found as the reference's
// `layers` finds them: the entry face of every solid block, and the exit face of a see-through
// block left (a run of one block is one slab; a ray starting inside one leaves it through a
// surface). Past a see-through block the walk goes cell by cell through its run (blocks from
// the primary storage's `s_block_at`), then the kernel resumes (`ctrace`) from the air cell
// after it: any kernel, since it skips its start cell. A hit on a LOD cell (`lod > 0`) ends
// the walk, drawn opaque. Inline the surfaces are composited front to back (OVER in linear
// light, the shading's `shade_radiance`); deferred, `pixel_layer_records` writes `LAYER_SLOTS`
// records per pixel and `shade_layers` composites them.

const LAYER_SLOTS: u32 = 8u;

// A surface on the primary ray: distance, camera-relative cell, block, face, whether it is a
// see-through block's exit face (its normal faces back along the ray), whether its coverage
// counts (a see-through block at level 0; else it covers fully).
struct Layer { ok: bool, t: f32, cell: vec3<i32>, block: u32, face: u32, exit: bool, see: bool }

// The walk's state between surfaces. `state`: 0 the kernel's hit `h` (cast from distance
// `base`) is next; 1 crossing the see-through run of `block` from `cell`; 2 the entry face of
// `block` at `cell` (distance `t`, `axis`) is next; 3 done.
struct LayerWalk { state: u32, h: Hit, base: f32, cell: vec3<i32>, block: u32, t: f32, axis: u32 }

// The walk of primary ray (`o`, `d`) whose kernel hit is `h`.
fn layer_walk(o: vec3<f32>, h: Hit) -> LayerWalk {
    var w: LayerWalk;
    let c0 = vec3<i32>(floor(o));
    let b0 = s_block_at(c0 + vp.cam_block);
    if (b0 != 0u && block_see_through(b0)) {
        w.state = 1u;
        w.cell = c0;
        w.block = b0;
    } else {
        w.state = 0u;
        w.h = h;
    }
    return w;
}

// The next surface (`ok` false: none left), rays ending at `max_t`.
fn layer_next(w: ptr<function, LayerWalk>, o: vec3<f32>, d: vec3<f32>, max_t: f32) -> Layer {
    var l: Layer;
    l.ok = false;
    let pos = d > vec3<f32>(0.0);
    let stp = select(vec3<i32>(-1), vec3<i32>(1), pos);
    switch (*w).state {
        case 0u: {
            let h = (*w).h;
            (*w).state = 3u;
            if (!h.hit) { return l; }
            let see = h.lod == 0u && block_see_through(h.block);
            l = Layer(true, (*w).base + h.t, h.cell, h.block, face_entered(h.axis, pos[h.axis]), false, see);
            if (see) {
                (*w).state = 1u;
                (*w).cell = h.cell;
                (*w).block = h.block;
            }
        }
        case 1u: {
            // Cell by cell through the run (a DDA from the cell entered).
            var c = (*w).cell;
            let flat = abs(d) < vec3<f32>(1e-20);
            let edge = vec3<f32>(c + select(vec3<i32>(0), vec3<i32>(1), pos));
            var tmax = select((edge - o) / d, vec3<f32>(1e30), flat);
            let delta = select(abs(1.0 / d), vec3<f32>(1e30), flat);
            var a = 0u;
            var t = 0.0;
            var n = c;
            var nb = 0u;
            for (var i = 0u; i < 4096u; i++) {
                a = select(select(2u, 1u, tmax.y <= tmax.z), 0u, tmax.x <= tmax.y && tmax.x <= tmax.z);
                t = tmax[a];
                n = c;
                n[a] += stp[a];
                tmax[a] += delta[a];
                nb = s_block_at(n + vp.cam_block);
                if (nb != (*w).block || t > max_t) { break; }
                c = n;
            }
            (*w).state = 3u;
            if (t > max_t) { return l; }
            l = Layer(true, t, c, (*w).block, face_entered(a, !pos[a]), true, true);
            if (nb != 0u) {
                (*w).state = 2u;
                (*w).cell = n;
                (*w).block = nb;
                (*w).t = t;
                (*w).axis = a;
            } else {
                // Out into the air: the kernel again, from the point where the ray enters
                // that cell (in it).
                let p = clamp(o + d * t, vec3<f32>(n), vec3<f32>(n) + vec3<f32>(0.99999));
                (*w).h = ctrace(p, d, max_t - t);
                (*w).base = t;
                (*w).state = 0u;
            }
        }
        case 2u: {
            let see = block_see_through((*w).block);
            l = Layer(true, (*w).t, (*w).cell, (*w).block, face_entered((*w).axis, pos[(*w).axis]), false, see);
            (*w).state = select(3u, 1u, see);
        }
        default: {}
    }
    return l;
}

// Layer `l`'s surface on ray (`o`, `d`) (an exit face's normal reversed).
fn layer_surface(l: Layer, o: vec3<f32>, d: vec3<f32>) -> Surface {
    let p = o + d * l.t;
    let n = face_normal(l.face) * select(1.0, -1.0, l.exit);
    return Surface(p, n, clamp(p - vec3<f32>(l.cell), vec3<f32>(0.0), vec3<f32>(1.0)), l.block, l.face);
}

// How much of the light surface `s` of layer `l` stops.
fn layer_coverage(l: Layer, s: Surface) -> f32 {
    return select(1.0, surface_coverage(s), l.see);
}

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
