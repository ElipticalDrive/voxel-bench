// Storage `heightmap` (GPU side; the ring is built on the CPU, `heightmap::Heightmap`).
// `hm`: `size * size` entries (low 16 bits = top y + 32768, 0 = empty column; high 16 bits
// = block id), then per 32x32 tile (a chunk column) the largest top (same encoding).
// `hm_info`: x = ring mask, y = reach from the camera (blocks), z = tile base (words),
// w = highest top in the ring.

// The entry of camera-relative column `c`.
fn hm_at(c: vec2<i32>) -> u32 {
    let a = c + vp.cam_block.xz;
    let m = i32(hm_info.x);
    return hm[u32((a.y & m) * (m + 1) + (a.x & m))];
}

// Top surface height of entry `e` (camera-relative, = top block y + 1), or very low.
fn hm_top(e: u32) -> f32 {
    if ((e & 0xffffu) == 0u) { return -1e6; }
    return f32(i32(e & 0xffffu) - 32768 + 1 - vp.cam_block.y);
}

// The highest top of the tile holding camera-relative column `c`.
fn hm_tile_top(c: vec2<i32>) -> f32 {
    let t = (c + vp.cam_block.xz) >> vec2<u32>(5u);
    let m = (i32(hm_info.x) + 1) / 32 - 1;
    return hm_top(hm[hm_info.z + u32((t.y & m) * (m + 1) + (t.x & m))]);
}

// Where the ray (`o`, horizontal direction `dxz`) leaves camera-relative column `cell`,
// per axis (very far along an axis it does not move on).
fn hm_bound(o: vec2<f32>, dxz: vec2<f32>, inv: vec2<f32>, cell: vec2<i32>) -> vec2<f32> {
    let t = (select(vec2<f32>(cell), vec2<f32>(cell + 1), dxz > vec2<f32>(0.0)) - o) * sign(dxz) * inv;
    return select(t, vec2<f32>(3.0e38), abs(dxz) < vec2<f32>(1e-9));
}

// The first column a ray from camera-relative `o` along `d` hits within `max_t`: a 2D DDA
// over the columns. Entering a column below its top hits its side; going below the top
// inside a column hits the top. The start column is ignored when `o` lies below its top.
// Side cells take the top block's id (the heightmap keeps no other). Tiles the ray passes
// above are skipped whole, and a rising ray above the highest top stops. As
// `Heightmap::trace` on the CPU.
fn hm_trace(o: vec3<f32>, d: vec3<f32>, max_t: f32) -> Hit {
    var h: Hit;
    h.hit = false;
    let dxz = d.xz;
    let hlen = length(dxz);
    let global = hm_top(hm_info.w);
    let stp = vec2<i32>(select(-1, 1, dxz.x > 0.0), select(-1, 1, dxz.y > 0.0));
    let inv = 1.0 / max(abs(dxz), vec2<f32>(1e-9));
    var cell = vec2<i32>(floor(o.xz));
    var tmax = hm_bound(o.xz, dxz, inv, cell);
    var t0 = 0.0;
    var axis = 0u;
    var first = true;
    let reach = f32(hm_info.y);
    // At most one step per column and tile within reach (a safety bound).
    for (var guard = 0u; guard < 4u * hm_info.y + 64u && t0 <= max_t && t0 * hlen <= reach; guard++) {
        let y0 = o.y + d.y * t0;
        if (d.y >= 0.0 && y0 >= global) { break; }
        // Skip the tile if the ray stays above its highest column.
        let abs_c = cell + vp.cam_block.xz;
        let tile_min = ((abs_c >> vec2<u32>(5u)) << vec2<u32>(5u)) - vp.cam_block.xz;
        let tb = hm_bound(o.xz, dxz, inv, tile_min + select(vec2<i32>(0), vec2<i32>(31), stp > vec2<i32>(0)));
        let t_exit = min(tb.x, tb.y);
        if (min(y0, o.y + d.y * t_exit) >= hm_tile_top(cell)) {
            // Into the next tile, in integers so that every skip advances: the exit axis
            // steps over the tile edge, the other is clamped into the tile's range.
            first = false;
            axis = select(1u, 0u, tb.x < tb.y);
            t0 = t_exit;
            let p = o.xz + dxz * t0;
            var next = clamp(vec2<i32>(floor(p)), tile_min, tile_min + vec2<i32>(31));
            next[axis] = select(tile_min[axis] - 1, tile_min[axis] + 32, stp[axis] > 0);
            cell = next;
            tmax = hm_bound(o.xz, dxz, inv, cell);
            continue;
        }
        let t1 = min(tmax.x, tmax.y);
        let e = hm_at(cell);
        let H = hm_top(e);
        if (!first && y0 < H) {
            h.hit = true;
            h.t = t0;
            h.cell = vec3<i32>(cell.x, min(i32(floor(y0)), i32(H) - 1), cell.y);
            h.axis = select(0u, 2u, axis == 1u);
            h.block = e >> 16u;
            return h;
        }
        if (d.y < 0.0 && (!first || o.y >= H)) {
            let tt = (H - o.y) / d.y;
            if (tt >= t0 && tt < t1) {
                h.hit = tt <= max_t;
                h.t = tt;
                h.cell = vec3<i32>(cell.x, i32(H) - 1, cell.y);
                h.axis = 1u;
                h.block = e >> 16u;
                return h;
            }
        }
        first = false;
        t0 = t1;
        if (tmax.x < tmax.y) { cell.x += stp.x; tmax.x += inv.x; axis = 0u; } else { cell.y += stp.y; tmax.y += inv.y; axis = 1u; }
    }
    return h;
}

// The heightmap's walk as the composed walk: what shadow (`occluded`) and bounce rays use.
fn ctrace(o: vec3<f32>, d: vec3<f32>, max_t: f32) -> Hit {
    return hm_trace(o, d, max_t);
}
