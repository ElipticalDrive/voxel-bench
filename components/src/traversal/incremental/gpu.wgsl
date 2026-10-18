// Traversal `incremental` (IDEAS §1, port of cpu-simd-mem's `trace_incremental`): a
// hierarchical DDA that keeps a cell `c` at the current level (absolute coords >> level)
// with per-axis exit times `tmax` and cell sizes in t `tdelta`. Steps, ascents and
// descents are incremental; nothing is re-floored. With EXACT, exit times are recomputed
// from the integer cell after each move (no float drift). LEVEL_MASK (bit k = 2^k cubes;
// 0 and the top level S_TOP always set) comes from the storage's S_LEVELS and the skip
// knobs.
//
// On NVIDIA (RTX 5060, 2026-10) this loop runs ~5x slower than `refloor` whenever the
// ray's result is consumed, but only ~1.4x in `view = "steps"` and ~1.3x on AMD (RADV).
// Ruled out: dynamic vector indexing, private globals, struct writes and returns inside
// the loop, unbounded inner loops, a flattened one-test-per-iteration form, runaway rays,
// `continue`, extra live state. Cause still open (see IDEAS §1).

const TOP: u32 = S_TOP;

// Whether the level-`level` cube at absolute level coords `c` is occupied. `key`/`val`
// cache the last top cell looked up.
fn ic_occupied(level: u32, c: vec3<i32>, key: ptr<function, vec3<i32>>, val: ptr<function, u32>) -> bool {
    let shift = TOP - level;
    let ch = c >> vec3<u32>(shift);
    if (any(ch != *key)) {
        *key = ch;
        *val = s_top(ch);
    }
    let v = *val;
    if (v == EMPTY) { return false; }
    if ((v & UNIFORM_BIT) != 0u || level == TOP) { return true; }
    return s_occupied(v, level, c & vec3<i32>((1 << shift) - 1));
}

fn level_above(level: u32) -> u32 {
    return firstTrailingBit(LEVEL_MASK >> (level + 1u)) + level + 1u;
}

fn level_below(level: u32) -> u32 {
    return firstLeadingBit(LEVEL_MASK & ((1u << level) - 1u));
}

// Exit times of the level-`level` cell `cb` (absolute level coords), from integers.
fn ic_exit(cb: vec3<i32>, level: u32, pos: vec3<bool>, o: vec3<f32>, inv: vec3<f32>) -> vec3<f32> {
    let bound = ((cb + select(vec3<i32>(0), vec3<i32>(1), pos)) << vec3<u32>(level)) - vp.cam_block;
    return (vec3<f32>(bound) - o) * inv;
}

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
    var t = max(max(max(tlo.x, tlo.y), tlo.z), 0.0);
    let tfar = min(min(min(thi.x, thi.y), thi.z), max_t);
    if (t > tfar) { return res; }

    var axis = 3u;
    if (t > 0.0) {
        axis = select(select(2u, 1u, tlo.y >= tlo.z), 0u, tlo.x >= tlo.y && tlo.x >= tlo.z);
    }
    let start = vec3<i32>(floor(o)) + vp.cam_block;
    let rel = clamp(vec3<i32>(floor(o + d * t)), vp.bounds_min, vp.bounds_max - vec3<i32>(1));
    var c = rel + vp.cam_block;
    var tmax = (vec3<f32>(rel + select(vec3<i32>(0), vec3<i32>(1), pos)) - o) * inv;
    var tdelta = abs(inv);
    var level = 0u;
    var may_ascend = true;
    var hit = false;
    var ic_key = vec3<i32>(0x7fffffff);
    var ic_val = EMPTY;
    for (var i = 0u; i < MAX_STEPS; i++) {
        ct_iters += 1u;
        if (ic_occupied(level, c, &ic_key, &ic_val)) {
            if (level > 0u) {
                // Descend to the next level the storage tests.
                let below = level_below(level);
                while (level > below) {
                    var tmid: vec3<f32>;
                    if (EXACT) {
                        tmid = ic_exit(c * 2 + select(vec3<i32>(1), vec3<i32>(0), pos), level - 1u, pos, o, inv);
                    } else {
                        tdelta *= 0.5;
                        tmid = tmax - tdelta;
                    }
                    let second = tmid <= vec3<f32>(t);
                    tmax = select(tmid, tmax, second);
                    c = c * 2 + select(vec3<i32>(0), vec3<i32>(1), second == pos);
                    level -= 1u;
                }
                may_ascend = false;
                continue;
            }
            if (any(c != start)) {
                hit = true;
                break;
            }
        } else {
            // Climb while the parent is empty, then step at the highest empty level.
            while (may_ascend && level < TOP) {
                let above = level_above(level);
                if (ic_occupied(above, c >> vec3<u32>(above - level), &ic_key, &ic_val)) { break; }
                while (level < above) {
                    if (!EXACT) {
                        let first = ((c & vec3<i32>(1)) == vec3<i32>(0)) == pos;
                        tmax += select(vec3<f32>(0.0), tdelta, first);
                        tdelta *= 2.0;
                    }
                    c = c >> vec3<u32>(1u);
                    level += 1u;
                }
                if (EXACT) { tmax = ic_exit(c, level, pos, o, inv); }
            }
        }
        // Step at the current level.
        let ax = select(select(2u, 1u, tmax.y <= tmax.z), 0u, tmax.x <= tmax.y && tmax.x <= tmax.z);
        let m = vec3<u32>(ax) == vec3<u32>(0u, 1u, 2u);
        t = min(min(tmax.x, tmax.y), tmax.z);
        if (t > tfar) { break; }
        let old = c;
        c += select(vec3<i32>(0), select(vec3<i32>(-1), vec3<i32>(1), pos), m);
        if (EXACT) {
            tmax = select(tmax, ic_exit(c, level, pos, o, inv), m);
        } else {
            tmax += select(vec3<f32>(0.0), tdelta, m);
        }
        axis = ax;
        if (level < TOP) {
            let k = vec3<u32>(level_above(level) - level);
            may_ascend = any((old >> k) != (c >> k));
        } else {
            may_ascend = false;
        }
    }
    if (hit) {
        res.hit = true;
        res.t = t;
        res.cell = c - vp.cam_block;
        res.axis = axis;
        if ((ic_val & UNIFORM_BIT) != 0u) {
            res.block = ic_val & 0xffffu;
        } else {
            res.block = s_block(ic_val, c & vec3<i32>((1 << TOP) - 1));
        }
    }
    return res;
}
