// Traversal `topdown`: a top-down octree walk (Revelles, Ureña, Lastra 2000, "An efficient
// parametric algorithm for octree traversal"; the pop of Laine & Karras 2010, "Efficient
// sparse voxel octrees"). The current node is a level and the absolute corner of an aligned
// 2^level cube, so no stack is kept. An occupied node is entered: its child is chosen by
// comparing the midplane times with the node's entry time (no re-flooring). An empty node
// (or the start cell) is left through its exit face; the walk then pops to the highest
// level whose cell changed (the highest differing bit of the corner on that axis) and
// descends again from there. Levels the storage does not answer (LEVEL_MASK) are taken as
// occupied and descended through. Fast where the storage answers every level (octrees).

const TD_TOP: u32 = S_TOP;

fn ctrace(o: vec3<f32>, d_in: vec3<f32>, max_t: f32) -> Hit {
    var res: Hit;
    res.hit = false;
    let d = select(d_in, vec3<f32>(1e-20), abs(d_in) < vec3<f32>(1e-20));
    let inv = 1.0 / d;
    let pos = d > vec3<f32>(0.0);
    let t0 = (vec3<f32>(vp.bounds_min) - o) * inv;
    let t1 = (vec3<f32>(vp.bounds_max) - o) * inv;
    let tlo = min(t0, t1);
    let thi = max(t0, t1);
    let tnear = max(max(max(tlo.x, tlo.y), tlo.z), 0.0);
    let tfar = min(min(min(thi.x, thi.y), thi.z), max_t);
    if (tnear > tfar) { return res; }
    let start_cell = vec3<i32>(floor(o));
    let cell = clamp(vec3<i32>(floor(o + d * tnear)), vp.bounds_min, vp.bounds_max - vec3<i32>(1));
    var axis = 3u;
    if (tnear > 0.0) {
        axis = select(select(2u, 1u, tlo.y >= tlo.z), 0u, tlo.x >= tlo.y && tlo.x >= tlo.z);
    }
    res.t = tnear;
    // The current node: its level and absolute corner.
    var level = TD_TOP;
    var lo = (cell + vp.cam_block) & vec3<i32>(-(1 << TD_TOP));
    var cached = vec3<i32>(0x7fffffff);
    var val = EMPTY;
    for (var i = 0u; i < MAX_STEPS; i++) {
        ct_iters += 1u;
        let size = 1 << level;
        let key = lo >> vec3<u32>(TD_TOP);
        if (any(key != cached)) {
            cached = key;
            val = s_top(key);
        }
        // Camera-relative box of the node.
        let rlo = lo - vp.cam_block;
        let near = select(rlo + vec3<i32>(size), rlo, pos);
        let far = select(rlo, rlo + vec3<i32>(size), pos);
        let te = (vec3<f32>(near) - o) * inv;
        let tin = max(max(max(te.x, te.y), te.z), tnear);
        var b = 0u;
        var occupied = true;
        if (val == EMPTY) {
            occupied = false;
        } else if ((val & UNIFORM_BIT) != 0u) {
            b = val & 0xffffu;
            occupied = b != 0u;
        } else if (level == 0u) {
            b = s_block(val, lo & vec3<i32>((1 << TD_TOP) - 1));
            occupied = b != 0u;
        } else if (level != TD_TOP && ((LEVEL_MASK >> level) & 1u) != 0u) {
            occupied = s_occupied(val, level, (lo & vec3<i32>((1 << TD_TOP) - 1)) >> vec3<u32>(level));
        }
        if (occupied && level == 0u && any(rlo != start_cell)) {
            res.hit = true;
            res.t = tin;
            res.cell = rlo;
            res.block = b;
            res.axis = axis;
            return res;
        }
        if (occupied && level > 0u) {
            // Descend into the child the ray enters: the far half on each axis whose
            // midplane it has crossed by the node's entry.
            let half = size >> 1u;
            let tm = (vec3<f32>(rlo + vec3<i32>(half)) - o) * inv;
            let upper = (tm <= vec3<f32>(tin)) == pos;
            lo += select(vec3<i32>(0), vec3<i32>(half), upper);
            level -= 1u;
            continue;
        }
        // Leave the node through its exit face, then pop to the highest changed level.
        let tx = (vec3<f32>(far) - o) * inv;
        var a = 0u;
        if (tx.y < tx[a]) { a = 1u; }
        if (tx.z < tx[a]) { a = 2u; }
        res.t = tx[a];
        if (res.t > tfar) { return res; }
        let before = lo[a];
        lo[a] += select(-size, size, pos[a]);
        let up = min(firstLeadingBit(u32(before ^ lo[a])), TD_TOP);
        level = up;
        lo &= vec3<i32>(-(1 << up));
        axis = a;
    }
    return res;
}
