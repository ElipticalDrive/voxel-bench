// The layer walk (transparency): the surfaces along a ray front to back, as the reference's
// `layers` finds them: the entry face of every solid block, and the exit face of a see-through
// block left (a run of one block is one slab; a ray starting inside one leaves it through a
// surface). Past a see-through block the walk goes cell by cell through its run (blocks from
// the storage's `s_block_at`), then the kernel resumes (`ctrace`) from the air cell after it:
// any kernel, since it skips its start cell. A hit on a LOD cell (`lod > 0`) ends the walk,
// covering fully. The primary rays' walk (the ray-casting root's) and the shadow rays'
// (`vox_transmit` in a ray service) are each an instance of this text over their own kernel.

// A surface on the ray: distance, camera-relative cell, block, face, whether it is a
// see-through block's exit face (its normal faces back along the ray), whether its coverage
// counts (a see-through block at level 0; else it covers fully).
struct Layer { ok: bool, t: f32, cell: vec3<i32>, block: u32, face: u32, exit: bool, see: bool }

// The walk's state between surfaces. `state`: 0 the kernel's hit `h` (cast from distance
// `base`) is next; 1 crossing the see-through run of `block` from `cell`; 2 the entry face of
// `block` at `cell` (distance `t`, `axis`) is next; 3 done.
struct LayerWalk { state: u32, h: Hit, base: f32, cell: vec3<i32>, block: u32, t: f32, axis: u32 }

// The walk of ray (`o`, `d`) whose kernel hit is `h`.
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
