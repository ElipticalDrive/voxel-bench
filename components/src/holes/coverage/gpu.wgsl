// Hole detection `coverage` on the GPU (PLAN step 13b; a root over `quadtree::set`): which
// faces of the previous list still cover a pixel, which level-0 tiles need discovery, and
// the sky mask after the final draw. See the incremental notes in `quadtree::set`.

// ---- incremental mode ----

// Stamps every face of the previous list that covers a pixel in this frame's raster of it.
@compute @workgroup_size(256)
fn mark(@builtin(workgroup_id) wg: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let i = tid(wg, nwg, li);
    if (i >= pr.width * pr.height) { return; }
    let id = textureLoad(idtex, vec2<u32>(i % pr.width, i / pr.width), 0).r;
    if (id != 0u) { seen[id - 1u] = pr.frame; }
}

// Whether pixel `p`'s direction was sky in the previous frame. Sky is at infinity, so only
// the camera's rotation moves it on screen: map the direction into the previous camera's
// image. Things that can newly appear in front of the sky do so at silhouettes (the
// parallax rule), in changed blocks / chunks, or at the screen border (no history here).
fn sky_known(p: vec2<i32>) -> bool {
    if (pr.has_prev == 0u) { return false; }
    let d = camera_dir(pr.forward, pr.right, pr.up, pr.tan_half_fov, pr.aspect, vec2<f32>(p) + 0.5, vec2<f32>(f32(pr.width), f32(pr.height)));
    let z = dot(d, pr.prev_forward);
    if (z <= 0.0) { return false; }
    let u = dot(d, pr.prev_right) / (z * pr.prev_tan * pr.aspect);
    let v = dot(d, pr.prev_up) / (z * pr.prev_tan);
    let q = vec2<i32>(floor(vec2<f32>((u + 1.0) * 0.5 * f32(pr.width), (1.0 - v) * 0.5 * f32(pr.height))));
    if (any(q < vec2<i32>(0)) || q.x >= i32(pr.width) || q.y >= i32(pr.height)) { return false; }
    return sky_prev[u32(q.y) * pr.width + u32(q.x)] != 0u;
}

// After the final raster: which pixels are sky this frame. An uncovered pixel is
// confirmed sky if it was sky-known or its tile went through discovery this frame.
// Otherwise it is a hole (a face no sample has found yet, e.g. one that a rotation moved
// onto a pixel centre): not sky, and its tile is discovered next frame.
@compute @workgroup_size(256)
fn sky_update(@builtin(workgroup_id) wg: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let i = tid(wg, nwg, li);
    if (i >= pr.width * pr.height) { return; }
    let p = vec2<u32>(i % pr.width, i / pr.width);
    let d = textureLoad(depthtex, p, 0);
    var sky = d == 0.0;
    if (sky) {
        let t = p / pr.tile;
        let tile = t.y * pr.cols + t.x;
        if (need[pr.need_off + tile] == 0u && !sky_known(vec2<i32>(p))) {
            sky = false;
            need[3u * pr.need_off + tile] = 1u;
        }
    }
    sky_cur[i] = select(0u, 1u, sky);
}

// Per level-0 tile: 1 if it has uncovered pixels; 1 + R if an occlusion edge in it (or on
// its 1-px border) can reveal up to R tiles under the camera's translation since the last
// frame. Revealed width at an edge between depths z_near and z_far is about
// f * T * (1/z_near - 1/z_far) pixels, and with reverse-Z depth d = near / z that is
// par_k * (d_max - d_min). Uncovered pixels count as depth 0 (infinitely far).
@compute @workgroup_size(256)
fn need_raw(@builtin(workgroup_id) wg: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let i = tid(wg, nwg, li);
    if (i >= pr.cols * pr.rows) { return; }
    let s = i32(pr.tile);
    let t = vec2<i32>(i32(i % pr.cols), i32(i / pr.cols)) * s - vec2<i32>(i32(pr.off_x), i32(pr.off_y));
    let size = vec2<i32>(i32(pr.width), i32(pr.height));
    var uncovered = false;
    var dmin = 1e30;
    var dmax = -1e30;
    for (var y = -1; y <= s; y++) {
        for (var x = -1; x <= s; x++) {
            let p = t + vec2<i32>(x, y);
            if (any(p < vec2<i32>(0)) || any(p >= size)) { continue; }
            var d = 0.0;
            if (textureLoad(idtex, p, 0).r == 0u) {
                if (x >= 0 && y >= 0 && x < s && y < s && !sky_known(p)) { uncovered = true; }
            } else {
                d = textureLoad(depthtex, p, 0);
            }
            dmin = min(dmin, d);
            dmax = max(dmax, d);
        }
    }
    var r = select(0u, 1u, uncovered);
    // Reveals below a pixel per frame add up: bound what the camera's motion may have
    // revealed since the tile was last discovered (approximated as this frame's motion
    // times the frames since).
    let age = need[2u * pr.need_off + i];
    let reveal = pr.par_k * (dmax - dmin) * f32(age + 1u);
    if (dmax >= dmin && reveal > pr.par_thresh) {
        let tiles = u32(ceil(reveal / f32(s)));
        if (tiles > pr.r_max) { atomicAdd(&st[C_RCLAMP], 1u); }
        r = max(r, 1u + min(tiles, pr.r_max));
    }
    need[i] = r;
}

// A tile needs discovery if a tile within its reveal radius asked for it, it lies in a
// changed chunk's screen rectangle, it is its turn in the round-robin refresh, or the
// whole frame is forced (first frame, teleports, setting changes).
@compute @workgroup_size(256)
fn need_final(@builtin(workgroup_id) wg: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let i = tid(wg, nwg, li);
    if (i >= pr.cols * pr.rows) { return; }
    let tx = i32(i % pr.cols);
    let ty = i32(i / pr.cols);
    var why = select(0u, WHY_FULL, pr.full != 0u);
    if (pr.refresh != 0u && i % pr.refresh == pr.frame % pr.refresh) { why |= WHY_REFRESH; }
    let rm = i32(pr.r_max);
    for (var dy = -rm; dy <= rm; dy++) {
        for (var dx = -rm; dx <= rm; dx++) {
            let x = tx + dx;
            let y = ty + dy;
            if (x < 0 || y < 0 || x >= i32(pr.cols) || y >= i32(pr.rows)) { continue; }
            let v = need[u32(y) * pr.cols + u32(x)];
            if (v != 0u && max(abs(dx), abs(dy)) <= i32(v) - 1) {
                why |= select(WHY_PARALLAX, WHY_UNCOVERED, v == 1u);
            }
        }
    }
    why |= rect_why(u32(tx), u32(ty));
    finish_need(i, why);
}

fn rect_why(tx: u32, ty: u32) -> u32 {
    if (pr.changed != 0u && changed_tiles[ty * pr.cols + tx] != 0u) { return WHY_CHUNK; }
    return 0u;
}

// Stores the tile's reasons (non-zero = discover) and its age since last discovered.
fn finish_need(i: u32, why: u32) {
    need[pr.need_off + i] = why;
    need[3u * pr.need_off + i] = 0u;
    let age = need[2u * pr.need_off + i];
    need[2u * pr.need_off + i] = select(age + 1u, 0u, why != 0u);
    if (why != 0u) { atomicAdd(&st[C_NEED], 1u); }
}

// Whether the direction through image point `px` (pixel units, not pixel centres) lay
// inside the previous camera's image.
fn in_prev(px: vec2<f32>) -> bool {
    let d = camera_dir(pr.forward, pr.right, pr.up, pr.tan_half_fov, pr.aspect, px, vec2<f32>(f32(pr.width), f32(pr.height)));
    let z = dot(d, pr.prev_forward);
    if (z <= 0.0) { return false; }
    let u = dot(d, pr.prev_right) / (z * pr.prev_tan * pr.aspect);
    let v = dot(d, pr.prev_up) / (z * pr.prev_tan);
    return abs(u) <= 1.0 && abs(v) <= 1.0;
}

// Still frames (camera rotated and/or chunks changed, no translation): visibility can
// only change where the image shows directions the previous frame did not (the strip
// entering the screen) and in changed chunks. The previous image is a convex region of
// directions, so a tile lies inside it exactly when its four corners do.
@compute @workgroup_size(256)
fn need_still(@builtin(workgroup_id) wg: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let i = tid(wg, nwg, li);
    if (i >= pr.cols * pr.rows) { return; }
    let tx = i % pr.cols;
    let ty = i / pr.cols;
    var why = select(0u, WHY_FULL, pr.full != 0u || pr.has_prev == 0u);
    let s = f32(pr.tile);
    let lo = vec2<f32>(f32(tx), f32(ty)) * s;
    let hi = min(lo + vec2<f32>(s), vec2<f32>(f32(pr.width), f32(pr.height)));
    if (!(in_prev(lo) && in_prev(vec2<f32>(hi.x, lo.y)) && in_prev(vec2<f32>(lo.x, hi.y)) && in_prev(hi))) {
        why |= WHY_STRIP;
    }
    why |= rect_why(tx, ty);
    if (need[3u * pr.need_off + i] != 0u) { why |= WHY_UNCOVERED; }
    // Rotation keeps geometric visibility but moves pixel centres: a face too small to have
    // been sampled can land on one in front of a known face. Nothing local reveals that,
    // so the round-robin refresh stays on.
    if (pr.refresh != 0u && i % pr.refresh == pr.frame % pr.refresh) { why |= WHY_REFRESH; }
    finish_need(i, why);
}

// ---- back-to-front order (`order = painter`, PLAN step 4) ----
// Along any ray the voxel walk only moves away from the camera's cell on each axis, so the
// Manhattan distance of a cell from the camera cell strictly increases along the ray: an
