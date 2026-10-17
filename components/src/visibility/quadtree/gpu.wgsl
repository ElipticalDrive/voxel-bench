// The quadtree sampler on the GPU (PLAN step 13b; a root over `quadtree::set`): temporal
// survivors, rays at lattice points, tile classification, footprints, the audit, and for
// the drawer the painter's sort and the row merge.

// ---- temporal survivors: re-insert last frame's faces still young and still exposed ----
@compute @workgroup_size(256)
fn survive(@builtin(workgroup_id) wg: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let i = tid(wg, nwg, li);
    let n = min(atomicLoad(&lcount[1u - pr.parity]), pr.list_cap);
    if (i >= n) { return; }
    let e = plist[i];
    var last = pvals[e.y];
    if (seen[i] == pr.frame) { last = pr.frame; } // visible in this frame's raster of the old list
    let key = e.x;
    let cp = cpos[key >> 18u];
    if (cp.w == 0) { return; }
    let l = (key >> 3u) & 32767u;
    let a = cp.xyz * 32 + vec3<i32>(i32(l & 31u), i32((l >> 5u) & 31u), i32(l >> 10u));
    // Incremental: unseen faces inside the view (sub-pixel, briefly hidden) are kept long;
    // faces outside it (behind or beside the camera) expire quickly.
    var keep = pr.keep;
#ifdef HOLES
    let v = vec3<f32>(a - vp.cam_block) + vec3<f32>(0.5) - pr.origin;
    let z = dot(v, pr.forward);
    let inside = z > -1.0 && abs(dot(v, pr.right)) <= z * pr.tan_half_fov * pr.aspect + 1.0 && abs(dot(v, pr.up)) <= z * pr.tan_half_fov + 1.0;
    if (!inside) { keep = pr.keep_out; }
    // Rotation (or world changes) only: what was visible and is still in view still is.
    if (pr.still != 0u && inside) { last = pr.frame; }
#endif
    if (pr.frame - last >= keep) { return; }
    let face = key & 7u;
    let b = exposed(a, face);
    if (b == 0u) { return; }
    set_insert(key, b, last, 0u);
    atomicAdd(&st[C_SURV], 1u);
}

// ---- ray casting at lattice points ----
fn cast_ray(q: vec2<u32>) {
    let d = ray_dir(q);
    // The composed walk over the composed storage.
    let h = ctrace(pr.origin, d, vp.max_dist);
    let idx = q.y * pr.lat_w + q.x;
    cast_frame[idx] = pr.frame;
    if (h.hit) {
        let face = face_entered(h.axis, d[h.axis] > 0.0);
        cache_c[idx] = vec4<i32>(h.cell, i32(face | (h.block << 3u)));
        cache_t[idx] = h.t;
    } else {
        cache_c[idx] = vec4<i32>(0, 0, 0, -1);
    }
}

@compute @workgroup_size(256)
fn rays(@builtin(workgroup_id) wg: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let i = tid(wg, nwg, li);
    var q: vec2<u32>;
    if (lv.level == 0u) {
        let n = (pr.cols + 1u) * (pr.rows + 1u);
#ifndef HOLES
        if (i == 0u) { atomicStore(&st[C_RAYS], n); }
#endif
        let g = grid_of(i, pr.cols + 1u);
        let cx = g.x;
        let cy = g.y;
        if (cx > pr.cols || cy > pr.rows) { return; }
#ifdef HOLES
        // Only corners of tiles that need discovery.
        var any = false;
        for (var k = 0u; k < 4u; k++) {
            let tx = i32(cx) - i32(k & 1u);
            let ty = i32(cy) - i32(k >> 1u);
            if (tx >= 0 && ty >= 0 && tx < i32(pr.cols) && ty < i32(pr.rows) && need[pr.need_off + u32(ty) * pr.cols + u32(tx)] != 0u) { any = true; }
        }
        if (!any) { return; }
        atomicAdd(&st[C_RAYS], 1u);
#endif
        q = vec2<u32>(cx, cy) * pr.tile;
    } else {
        let n = min(atomicLoad(&st[C_RAYS + lv.level]), pr.queue_cap);
        if (i >= n) { return; }
        let e = rayq[i];
        q = vec2<u32>(e & 0xffffu, e >> 16u);
    }
    cast_ray(q);
}

// ---- tile classification ----
fn tile_on_screen(x0: u32, y0: u32, s: u32) -> bool {
    let px = i32(x0) - i32(pr.off_x);
    let py = i32(y0) - i32(pr.off_y);
    return px < i32(pr.width) && py < i32(pr.height) && px + i32(s) >= 0 && py + i32(s) >= 0;
}

// Pushes the on-screen children and the not-yet-claimed new lattice points, with one
// atomicAdd per queue (global counters are heavily contended).
var<private> deferring: bool = false;
var<private> split_req: bool = false;
var<private> split_t: vec2<u32>;

fn split(t: vec2<u32>, s: u32) {
    if (deferring) {
        split_req = true;
        split_t = t;
        return;
    }
    split_direct(t, s);
}

fn split_direct(t: vec2<u32>, s: u32) {
    let h = s / 2u;
    var kids: array<u32, 4>;
    var nk = 0u;
    for (var k = 0u; k < 4u; k++) {
        let c = t + vec2<u32>(k & 1u, k >> 1u) * h;
        if (tile_on_screen(c.x, c.y, h)) {
            kids[nk] = c.x | (c.y << 16u);
            nk++;
        }
    }
    if (nk > 0u) {
        let j = atomicAdd(&st[C_TILES + lv.level + 1u], nk);
        for (var k = 0u; k < nk; k++) {
            if (j + k < pr.queue_cap) { tileq[lv.out_off + j + k] = kids[k]; } else { atomicAdd(&st[C_OVER], 1u); }
        }
    }
    let stamp = pr.frame * 8u + lv.level + 1u;
    var pts = array<vec2<u32>, 5>(t + vec2<u32>(h, 0u), t + vec2<u32>(0u, h), t + vec2<u32>(h, h), t + vec2<u32>(s, h), t + vec2<u32>(h, s));
    var mine: array<u32, 5>;
    var nr = 0u;
    for (var k = 0u; k < 5u; k++) {
        let q = pts[k];
        if (atomicExchange(&claim[q.y * pr.lat_w + q.x], stamp) != stamp) {
            mine[nr] = q.x | (q.y << 16u);
            nr++;
        }
    }
    if (nr > 0u) {
        let j = atomicAdd(&st[C_RAYS + lv.level + 1u], nr);
        for (var k = 0u; k < nr; k++) {
            if (j + k < pr.queue_cap) { rayq[j + k] = mine[k]; } else { atomicAdd(&st[C_OVER], 1u); }
        }
    }
}

// Local closure: a small all-hit tile whose corner hit cells fit in a closure_box^3 box is
// accepted by adding every camera-facing exposed face inside that box (instead of
// splitting down to pixel rays).
fn closure(t: vec2<u32>, s: u32, c: array<vec4<i32>, 4>) -> bool {
    if (s > pr.closure_px) { return false; }
    var lo = c[0].xyz;
    var hi = c[0].xyz;
    for (var k = 0u; k < 4u; k++) {
        if (c[k].w < 0) { return false; }
        lo = min(lo, c[k].xyz);
        hi = max(hi, c[k].xyz);
    }
    if (any(hi - lo >= vec3<i32>(pr.closure_box))) { return false; }
    var added = 0u;
    for (var z = lo.z; z <= hi.z; z++) {
        for (var y = lo.y; y <= hi.y; y++) {
            for (var x = lo.x; x <= hi.x; x++) {
                let rel = vec3<i32>(x, y, z);
                // Cell centre relative to the camera.
                let to = vec3<f32>(rel) + vec3<f32>(0.5) - pr.origin;
                for (var f = 0u; f < 6u; f++) {
                    // Skip back faces: the face's outward normal must point to the camera.
                    let n = vec3<f32>(normal_i(f));
                    if (dot(n, to) + 0.5 >= 0.0) { continue; }
                    let b = exposed(rel + vp.cam_block, f);
                    if (b != 0u) {
                        add_face_plain(rel + vp.cam_block, f, b);
                        added++;
                    }
                }
            }
        }
    }
    atomicAdd(&st[C_CLOSURE], 1u);
    atomicAdd(&st[C_CL_FACES], added);
    return true;
}

fn reject(t: vec2<u32>, s: u32, c: array<vec4<i32>, 4>) {
    if (!closure(t, s, c)) { split(t, s); }
}

var<workgroup> wg_tbase: u32;
var<workgroup> wg_rbase: u32;
var<workgroup> wg_scan: array<u32, 256>;

// Exclusive prefix sum of `v` over the workgroup's threads, in thread order (so appended
// queue entries keep their parents' order); returns (offset, workgroup total).
fn wg_prefix(li: u32, v: u32) -> vec2<u32> {
    wg_scan[li] = v;
    workgroupBarrier();
    for (var d = 1u; d < 256u; d <<= 1u) {
        var x = 0u;
        if (li >= d) { x = wg_scan[li - d]; }
        workgroupBarrier();
        wg_scan[li] += x;
        workgroupBarrier();
    }
    let incl = wg_scan[li];
    let total = wg_scan[255];
    workgroupBarrier();
    return vec2<u32>(incl - v, total);
}

// Classification with workgroup-aggregated queue appends for splits (one global atomic
// per workgroup and queue instead of one per tile).
@compute @workgroup_size(256)
fn classify(@builtin(workgroup_id) wg: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    deferring = true;
    classify_tile(tid(wg, nwg, li));
    var kids: array<u32, 4>;
    var mine: array<u32, 5>;
    var nk = 0u;
    var nr = 0u;
    if (split_req) {
        let t = split_t;
        let s = lv.size;
        let h = s / 2u;
        for (var k = 0u; k < 4u; k++) {
            let c = t + vec2<u32>(k & 1u, k >> 1u) * h;
            if (tile_on_screen(c.x, c.y, h)) {
                kids[nk] = c.x | (c.y << 16u);
                nk++;
            }
        }
        let stamp = pr.frame * 8u + lv.level + 1u;
        var pts = array<vec2<u32>, 5>(t + vec2<u32>(h, 0u), t + vec2<u32>(0u, h), t + vec2<u32>(h, h), t + vec2<u32>(s, h), t + vec2<u32>(h, s));
        for (var k = 0u; k < 5u; k++) {
            let q = pts[k];
            if (atomicExchange(&claim[q.y * pr.lat_w + q.x], stamp) != stamp) {
                mine[nr] = q.x | (q.y << 16u);
                nr++;
            }
        }
    }
    let ts = wg_prefix(li, nk);
    let rs = wg_prefix(li, nr);
    if (li == 0u) {
        wg_tbase = atomicAdd(&st[C_TILES + lv.level + 1u], ts.y);
        wg_rbase = atomicAdd(&st[C_RAYS + lv.level + 1u], rs.y);
    }
    let tb = workgroupUniformLoad(&wg_tbase) + ts.x;
    let rb = workgroupUniformLoad(&wg_rbase) + rs.x;
    for (var k = 0u; k < nk; k++) {
        if (tb + k < pr.queue_cap) { tileq[lv.out_off + tb + k] = kids[k]; } else { atomicAdd(&st[C_OVER], 1u); }
    }
    for (var k = 0u; k < nr; k++) {
        if (rb + k < pr.queue_cap) { rayq[rb + k] = mine[k]; } else { atomicAdd(&st[C_OVER], 1u); }
    }
}

fn classify_tile(i: u32) {
    let s = lv.size;
    var t: vec2<u32>;
    if (lv.level == 0u) {
        let g = grid_of(i, pr.cols);
        if (g.x >= pr.cols || g.y >= pr.rows) { return; }
#ifdef HOLES
        if (need[pr.need_off + g.y * pr.cols + g.x] == 0u) { return; }
#endif
        t = g * pr.tile;
    } else {
        let n = min(atomicLoad(&st[C_TILES + lv.level]), pr.queue_cap);
        if (i >= n) { return; }
        let e = tileq[lv.in_off + i];
        t = vec2<u32>(e & 0xffffu, e >> 16u);
    }
    var q: array<vec2<u32>, 4>;
    var c: array<vec4<i32>, 4>;
    var nhit = 0u;
    for (var k = 0u; k < 4u; k++) {
        q[k] = t + vec2<u32>(k & 1u, k >> 1u) * s;
        c[k] = cache_c[q[k].y * pr.lat_w + q[k].x];
        if (c[k].w >= 0) { nhit++; }
    }
    // 1-px tiles: the corner rays are pixel rays; their faces are the samples.
    if (s == 1u) {
        // Neighbouring 1-px tiles share corners: insert each pixel's face once.
        let stamp = pr.frame * 8u + 7u;
        for (var k = 0u; k < 4u; k++) {
            if (c[k].w >= 0 && atomicExchange(&claim[q[k].y * pr.lat_w + q[k].x], stamp) != stamp) {
                add_face(c[k].xyz + vp.cam_block, u32(c[k].w) & 7u, u32(c[k].w) >> 3u);
            }
        }
        return;
    }
    if (nhit == 0u) {
        if (s <= pr.sky_tile) { atomicAdd(&st[C_SKY + lv.level], 1u); return; }
        split(t, s);
        return;
    }
    if (nhit < 4u) { split(t, s); return; }
    let face = u32(c[0].w) & 7u;
    let axis = face / 2u;
    for (var k = 1u; k < 4u; k++) {
        if ((u32(c[k].w) & 7u) != face || c[k][axis] != c[0][axis]) { reject(t, s, c); return; }
    }
    if (pr.criterion == 1u) {
        for (var k = 1u; k < 4u; k++) {
            if (any(c[k].xyz != c[0].xyz)) { reject(t, s, c); return; }
        }
        add_face(c[0].xyz + vp.cam_block, face, u32(c[0].w) >> 3u);
        atomicAdd(&st[C_ACC + lv.level], 1u);
        atomicAdd(&st[C_FP_FACES], 1u);
        return;
    }
    // Plane criterion: footprint = bounding box of the corner hit points on the plane.
    let ua = (axis + 1u) % 3u;
    let va = (axis + 2u) % 3u;
    var lo = vec2<f32>(1e30);
    var hi = vec2<f32>(-1e30);
    for (var k = 0u; k < 4u; k++) {
        let p = pr.origin + ray_dir(q[k]) * cache_t[q[k].y * pr.lat_w + q[k].x];
        let uv = vec2<f32>(p[ua], p[va]);
        lo = min(lo, uv);
        hi = max(hi, uv);
    }
    let ext = hi - lo;
    if (ext.x > 4096.0 || ext.y > 4096.0) { atomicAdd(&st[C_FP_REJECT], 1u); reject(t, s, c); return; }
    let a0 = vec2<i32>(floor(lo - vec2<f32>(1e-3)));
    let a1 = vec2<i32>(floor(hi + vec2<f32>(1e-3)));
    let nu = a1.x - a0.x + 1;
    let nv = a1.y - a0.y + 1;
    if (u32(nu * nv) > pr.max_fp) { atomicAdd(&st[C_FP_REJECT], 1u); reject(t, s, c); return; }
    var base = c[0].xyz + vp.cam_block;
    let cb = vp.cam_block;
    let n = u32(nu * nv);
    if (n <= pr.inline_fp) {
        // Small footprints (most tiles at deep levels): expand here. A 64-thread workgroup
        // per 1-4 cell job would leave nearly all of its threads idle.
        for (var k = 0u; k < n && pr.strict != 0u; k++) {
            var a = base;
            a[ua] = a0.x + cb[ua] + i32(k % u32(nu));
            a[va] = a0.y + cb[va] + i32(k / u32(nu));
            if (exposed(a, face) == 0u) { atomicAdd(&st[C_FP_REJECT], 1u); reject(t, s, c); return; }
        }
        var added = 0u;
        for (var k = 0u; k < n; k++) {
            var a = base;
            a[ua] = a0.x + cb[ua] + i32(k % u32(nu));
            a[va] = a0.y + cb[va] + i32(k / u32(nu));
            let b = exposed(a, face);
            if (b != 0u) {
                add_face(a, face, b);
                added++;
            }
        }
        atomicAdd(&st[C_FP_FACES], added);
        atomicAdd(&st[C_ACC + lv.level], 1u);
        return;
    }
    // Larger footprints are expanded cooperatively (one workgroup per tile) in `footprint`.
    let j = atomicAdd(&st[C_JOBS + lv.level], 1u);
    if (j < JOB_CAP) {
        jobs[2u * j] = vec4<i32>(base, i32(face));
        jobs[2u * j + 1u] = vec4<i32>(a0.x + cb[ua], a0.y + cb[va], nu | (nv << 16u), bitcast<i32>(t.x | (t.y << 16u)));
        return;
    }
    atomicAdd(&st[C_OVER], 1u);
    split(t, s);
}

var<workgroup> job_n: u32;
var<workgroup> job_ok: atomic<u32>;
var<workgroup> job_added: atomic<u32>;

@compute @workgroup_size(64)
fn footprint(@builtin(workgroup_id) wg: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let j = wg.y * nwg.x + wg.x;
    if (li == 0u) { job_n = min(atomicLoad(&st[C_JOBS + lv.level]), JOB_CAP); }
    if (j >= workgroupUniformLoad(&job_n)) { return; }
    let h0 = jobs[2u * j];
    let h1 = jobs[2u * j + 1u];
    let face = u32(h0.w);
    let axis = face / 2u;
    let ua = (axis + 1u) % 3u;
    let va = (axis + 2u) % 3u;
    let nu = u32(h1.z) & 0xffffu;
    let n = nu * (u32(h1.z) >> 16u);
    if (li == 0u) {
        atomicStore(&job_ok, 1u);
        atomicStore(&job_added, 0u);
    }
    workgroupBarrier();
    if (pr.strict != 0u) {
        for (var k = li; k < n; k += 64u) {
            var a = h0.xyz;
            a[ua] = h1.x + i32(k % nu);
            a[va] = h1.y + i32(k / nu);
            if (exposed(a, face) == 0u) { atomicStore(&job_ok, 0u); }
        }
    }
    let ok = workgroupUniformLoad(&job_ok);
    if (ok == 0u) {
        if (li == 0u) {
            atomicAdd(&st[C_FP_REJECT], 1u);
            let tp = bitcast<u32>(h1.w);
            let t = vec2<u32>(tp & 0xffffu, tp >> 16u);
            let s = lv.size;
            var c: array<vec4<i32>, 4>;
            for (var k = 0u; k < 4u; k++) {
                let q = t + vec2<u32>(k & 1u, k >> 1u) * s;
                c[k] = cache_c[q.y * pr.lat_w + q.x];
            }
            reject(t, s, c);
        }
        return;
    }
    var added = 0u;
    for (var k = li; k < n; k += 64u) {
        var a = h0.xyz;
        a[ua] = h1.x + i32(k % nu);
        a[va] = h1.y + i32(k / nu);
        let b = exposed(a, face);
        if (b != 0u) {
            add_face(a, face, b);
            added++;
        }
    }
    atomicAdd(&job_added, added);
    workgroupBarrier();
    if (li == 0u) {
        atomicAdd(&st[C_FP_FACES], atomicLoad(&job_added));
        atomicAdd(&st[C_ACC + lv.level], 1u);
    }
}

// ---- audit: one ray per pixel; count pixels whose visible face is not in S ----
@compute @workgroup_size(256)
fn audit(@builtin(workgroup_id) wg: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let i = tid(wg, nwg, li);
    if (i >= pr.width * pr.height) { return; }
    let q = vec2<u32>(i % pr.width + pr.off_x, i / pr.width + pr.off_y);
    let d = ray_dir(q);
    let h = ctrace(pr.origin, d, vp.max_dist);
    if (!h.hit) { return; }
    atomicAdd(&st[C_AUDIT_HIT], 1u);
    let k = face_key(h.cell + vp.cam_block, face_entered(h.axis, d[h.axis] > 0.0));
    if (k == NO_KEY || !set_contains(k)) { atomicAdd(&st[C_MISS], 1u); }
}


// occluder always has a smaller distance. Drawing faces by decreasing distance is therefore
// an exact painter's order, and since the key is a small integer, a counting sort does it.

const SORT_BUCKETS: u32 = 2048u;

fn sort_key(e: vec4<u32>) -> u32 {
    let cp = cpos[e.x >> 18u];
    let l = (e.x >> 3u) & 32767u;
    let a = cp.xyz * 32 + vec3<i32>(i32(l & 31u), i32((l >> 5u) & 31u), i32(l >> 10u));
    let d = abs(a - (vp.cam_block + vec3<i32>(floor(pr.origin))));
    return min(u32(d.x + d.y + d.z), SORT_BUCKETS - 1u);
}

@compute @workgroup_size(256)
fn sort_hist(@builtin(workgroup_id) wg: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let i = tid(wg, nwg, li);
    if (i >= min(atomicLoad(&lcount[pr.parity]), pr.list_cap)) { return; }
    atomicAdd(&hist[sort_key(list[i])], 1u);
}

// One workgroup: turns bucket counts into start offsets, farthest bucket first.
@compute @workgroup_size(256)
fn sort_scan(@builtin(local_invocation_index) li: u32) {
    // Each thread owns 8 buckets, counted from the far end.
    var local: array<u32, 8>;
    var sum = 0u;
    for (var k = 0u; k < 8u; k++) {
        let b = SORT_BUCKETS - 1u - (li * 8u + k);
        local[k] = atomicLoad(&hist[b]);
        sum += local[k];
    }
    let pre = wg_prefix(li, sum).x;
    var run = pre;
    for (var k = 0u; k < 8u; k++) {
        let b = SORT_BUCKETS - 1u - (li * 8u + k);
        atomicStore(&hist[b], run);
        run += local[k];
    }
}

@compute @workgroup_size(256)
fn sort_scatter(@builtin(workgroup_id) wg: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let i = tid(wg, nwg, li);
    if (i >= min(atomicLoad(&lcount[pr.parity]), pr.list_cap)) { return; }
    let e = list[i];
    sorted[atomicAdd(&hist[sort_key(e)], 1u)] = e;
}

// ---- greedy row merge (`merge = rows`, PLAN step 2b) ----
// Each face of S whose predecessor along its face's u axis differs (not in S, another
// block, or another v-neighbour pattern) starts a run; the run takes every following face
// with the same signature, up to MERGE_MAX. One quad per run is drawn instead of one per face. Texturing stays
// exact: the fragment shader derives texel coordinates per cell from the hit point.

const MERGE_MAX: u32 = 64u;

// Whether the face of cell `a` (on this face's plane) is in S.
fn in_s(a: vec3<i32>, face: u32) -> bool {
    let k = face_key(a, face);
    return k != NO_KEY && set_contains(k);
}

// What a run needs to agree on per cell: the block, and whether the coplanar neighbours
// before and after it along v are in S (bit 0: -v, bit 1: +v). Agreeing on the latter
// makes each long edge of the run either entirely a seam or entirely a silhouette.
fn run_sig(a: vec3<i32>, face: u32, dv: vec3<i32>) -> u32 {
    if (!in_s(a, face)) { return 0xffffffffu; }
    return (exposed(a, face) << 2u) | select(0u, 1u, in_s(a - dv, face)) | select(0u, 2u, in_s(a + dv, face));
}

@compute @workgroup_size(256)
fn merge_rows(@builtin(workgroup_id) wg: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let i = tid(wg, nwg, li);
    if (i >= min(atomicLoad(&lcount[pr.parity]), pr.list_cap)) { return; }
    let e = list[i];
    let cp = cpos[e.x >> 18u];
    let l = (e.x >> 3u) & 32767u;
    let a = cp.xyz * 32 + vec3<i32>(i32(l & 31u), i32((l >> 5u) & 31u), i32(l >> 10u));
    let face = e.x & 7u;
    let fa = face_axes(face);
    var du = vec3<i32>(0);
    du[fa.y] = 1;
    var dv = vec3<i32>(0);
    dv[fa.z] = 1;
    let sig = run_sig(a, face, dv);
    if (run_sig(a - du, face, dv) == sig) { return; } // not the start of a run
    var len = 1u;
    while (len < MERGE_MAX && run_sig(a + du * i32(len), face, dv) == sig) { len++; }
    // Seam edges (the face continues coplanar beyond them) get grown in the raster to
    // close T-junction cracks; silhouette edges must not, or they bleed over the
    // background. Bits: 0 -u, 1 +u, 2 -v, 3 +v.
    var seams = (sig & 1u) << 2u | (sig & 2u) << 2u;
    if (in_s(a - du, face)) { seams |= 1u; }
    if (in_s(a + du * i32(len), face)) { seams |= 2u; }
    let j = atomicAdd(&lcount[2], 1u);
    if (j < pr.list_cap) { merged[j] = vec4<u32>(e.x, seams, e.z, e.w | (len << 1u)); }
}
