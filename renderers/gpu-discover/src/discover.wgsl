// gpu-discover: GPU face discovery. Adaptive screen-space quadtree of corner rays; coherent
// tiles add every exposed face of the hit plane inside their footprint to a GPU hash set
// of world-space faces (S). Faces persist for `keep` frames (survivor pass).
//
// Face key (u32): chunk slot (14 bits) << 18 | local index (x | y<<5 | z<<10) << 3 | face.
// Hash set stores key + 1 (0 = empty) and the frame the face was last confirmed.
//
// Incremental mode (`incr`): last frame's face list is first rasterised at the current
// camera into a face-id buffer. `mark` stamps every face that covers a pixel as seen (seen
// faces survive; unseen ones expire after `keep` frames). `need_raw` / `need_final` pick
// the level-0 tiles that need discovery: uncovered pixels, occlusion edges whose parallax
// under the camera's translation can reveal something, changed chunks' screen rectangles,
// and a round-robin refresh. Level 0 casts and classifies only those tiles.

struct Params {
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, frame: u32,
    origin: vec3<f32>, tile: u32,
    width: u32, height: u32, off_x: u32, off_y: u32,
    lat_w: u32, lat_h: u32, cols: u32, rows: u32,
    keep: u32, criterion: u32, sky_tile: u32, max_fp: u32,   // criterion: 0 plane, 1 face
    table_mask: u32, list_cap: u32, chunk_mask: u32, chunk_probe: u32,
    queue_cap: u32, parity: u32, strict: u32, _p0: u32,
    closure_px: u32, closure_box: i32, block_faces: u32, _p2: u32,
    incr: u32, refresh: u32, par_k: f32, n_rects: u32,   // par_k = f_px * |camera move| / near
    full: u32, r_max: u32, need_off: u32, par_thresh: f32,
    prev_forward: vec3<f32>, prev_tan: f32,               // previous camera, for sky reprojection
    prev_right: vec3<f32>, has_prev: u32,
    prev_up: vec3<f32>, keep_out: u32,                    // keep for faces outside the frustum (incremental)
}

struct Level { level: u32, size: u32, in_off: u32, out_off: u32, parity: u32, cap: u32, list_cap: u32, _p: u32 }

@group(0) @binding(0) var<uniform> pr: Params;
@group(0) @binding(1) var<uniform> lv: Level;
@group(0) @binding(2) var<storage, read_write> st: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> cache_c: array<vec4<i32>>;
@group(0) @binding(4) var<storage, read_write> cache_t: array<f32>;
@group(0) @binding(5) var<storage, read_write> claim: array<atomic<u32>>;
@group(0) @binding(6) var<storage, read_write> rayq: array<u32>;
@group(0) @binding(7) var<storage, read_write> tileq: array<u32>;
@group(0) @binding(8) var<storage, read_write> keys: array<atomic<u32>>;
@group(0) @binding(9) var<storage, read_write> vals: array<atomic<u32>>;
@group(0) @binding(10) var<storage, read_write> list: array<vec4<u32>>;
@group(0) @binding(11) var<storage, read> pvals: array<u32>;
@group(0) @binding(12) var<storage, read> plist: array<vec4<u32>>;
@group(0) @binding(13) var<storage, read> ctable: array<vec4<i32>>;
@group(0) @binding(14) var<storage, read> cpos: array<vec4<i32>>;
@group(0) @binding(15) var<storage, read_write> lcount: array<atomic<u32>>;
@group(0) @binding(16) var<storage, read_write> jobs: array<vec4<i32>>; // footprint jobs, 2 per job
@group(0) @binding(17) var<storage, read_write> seen: array<u32>;  // per previous-list entry: frame last rasterised
@group(0) @binding(18) var<storage, read_write> need: array<u32>;  // [0..) reveal radius + 1 per tile; [need_off..) final flag; [2 need_off..) frames since discovered
@group(0) @binding(19) var idtex: texture_2d<u32>;                  // previous list's face index + 1 per pixel
@group(0) @binding(20) var depthtex: texture_depth_2d;
@group(0) @binding(21) var<storage, read> rects: array<vec4<u32>>; // changed-chunk screen rects, in level-0 tiles
@group(0) @binding(22) var<storage, read_write> sky_cur: array<u32>;  // 1 = sky in this frame's final raster
@group(0) @binding(23) var<storage, read> sky_prev: array<u32>;

const WG: u32 = 256u;
const NO_KEY: u32 = 0xffffffffu;
// Counter indices in `st`.
const C_TILES: u32 = 0u;     // + level
const C_RAYS: u32 = 8u;      // + level
const C_ACC: u32 = 16u;      // + level
const C_SKY: u32 = 24u;      // + level
const C_SURV: u32 = 32u;
const C_MISS: u32 = 33u;
const C_OVER: u32 = 34u;
const C_NOCHUNK: u32 = 35u;
const C_AUDIT_HIT: u32 = 36u;
const C_FP_REJECT: u32 = 37u;
const C_SPLIT1: u32 = 38u;   // faces added by 1-px tiles
const C_FP_FACES: u32 = 39u; // faces added by coherent tiles
const C_CLOSURE: u32 = 40u;  // tiles accepted by local closure
const C_CL_FACES: u32 = 41u; // faces added by local closure
const C_NEED: u32 = 42u;     // level-0 tiles needing discovery (incremental)
const C_RCLAMP: u32 = 43u;   // tiles whose reveal radius hit r_max
const C_JOBS: u32 = 48u;     // + level: footprint jobs queued
const JOB_CAP: u32 = 262144u;

fn tid(wg: vec3<u32>, nwg: vec3<u32>, li: u32) -> u32 {
    return (wg.y * nwg.x + wg.x) * WG + li;
}

// Primary ray through the centre of lattice point q (pixel q - offset); same arithmetic
// as gpu-dda and the raster fragment shader.
fn ray_dir(q: vec2<u32>) -> vec3<f32> {
    let px = f32(i32(q.x) - i32(pr.off_x));
    let py = f32(i32(q.y) - i32(pr.off_y));
    let sx = (2.0 * (px + 0.5) / f32(pr.width) - 1.0) * pr.tan_half_fov * pr.aspect;
    let sy = (1.0 - 2.0 * (py + 0.5) / f32(pr.height)) * pr.tan_half_fov;
    return normalize(pr.forward + sx * pr.right + sy * pr.up);
}

fn block_at(a: vec3<i32>) -> u32 {
    let v = lookup_chunk(chunk_key(a >> vec3<u32>(5u), 0u));
    if (v == EMPTY) { return 0u; }
    if ((v & UNIFORM_BIT) != 0u) { return v & 0xffffu; }
    return voxel(v, a & vec3<i32>(31));
}

fn normal_i(face: u32) -> vec3<i32> {
    var n = vec3<i32>(0);
    n[face / 2u] = select(1, -1, (face & 1u) == 1u);
    return n;
}

// Chunk-table lookup cache for loops over neighbouring cells.
var<private> ck: vec3<i32> = vec3<i32>(0x7fffffff);
var<private> cv: u32 = 0xffffffffu;

fn block_at_cached(a: vec3<i32>) -> u32 {
    let k = chunk_key(a >> vec3<u32>(5u), 0u);
    if (any(k != ck)) {
        ck = k;
        cv = lookup_chunk(k);
    }
    if (cv == EMPTY) { return 0u; }
    if ((cv & UNIFORM_BIT) != 0u) { return cv & 0xffffu; }
    return voxel(cv, a & vec3<i32>(31));
}

fn exposed(a: vec3<i32>, face: u32) -> u32 {
    let b = block_at_cached(a);
    if (b == 0u || block_at_cached(a + normal_i(face)) != 0u) { return 0u; }
    return b;
}

fn chunk_slot(c: vec3<i32>) -> i32 {
    var i = hash3(c) & pr.chunk_mask;
    for (var n = 0u; n <= pr.chunk_probe; n++) {
        let e = ctable[i];
        if (e.w < 0) { return -1; }
        if (all(e.xyz == c)) { return e.w; }
        i = (i + 1u) & pr.chunk_mask;
    }
    return -1;
}

fn face_key(a: vec3<i32>, face: u32) -> u32 {
    let slot = chunk_slot(a >> vec3<u32>(5u));
    if (slot < 0) { return NO_KEY; }
    let l = a & vec3<i32>(31);
    return (u32(slot) << 18u) | (u32(l.x | (l.y << 5u) | (l.z << 10u)) << 3u) | face;
}

fn hash_u(k: u32) -> u32 {
    var h = k;
    h ^= h >> 16u; h *= 0x7feb352du;
    h ^= h >> 15u; h *= 0x846ca68bu;
    h ^= h >> 16u;
    return h;
}

fn set_insert(key: u32, block: u32, last: u32) {
    let k1 = key + 1u;
    var i = hash_u(key) & pr.table_mask;
    for (var n = 0u; n < 128u; n++) {
        let r = atomicCompareExchangeWeak(&keys[i], 0u, k1);
        if (r.exchanged) {
            let j = atomicAdd(&lcount[pr.parity], 1u);
            if (j < pr.list_cap) {
                list[j] = vec4<u32>(key, i, block, 0u);
            } else {
                atomicAdd(&st[C_OVER], 1u);
            }
            atomicMax(&vals[i], last);
            return;
        }
        if (r.old_value == k1) {
            atomicMax(&vals[i], last);
            return;
        }
        if (r.old_value == 0u) { continue; } // spurious failure: retry this slot
        i = (i + 1u) & pr.table_mask;
    }
    atomicAdd(&st[C_OVER], 1u);
}

fn set_contains(key: u32) -> bool {
    let k1 = key + 1u;
    var i = hash_u(key) & pr.table_mask;
    for (var n = 0u; n < 128u; n++) {
        let k = atomicLoad(&keys[i]);
        if (k == k1) { return true; }
        if (k == 0u) { return false; }
        i = (i + 1u) & pr.table_mask;
    }
    return false;
}

fn add_face_plain(a: vec3<i32>, face: u32, block: u32) {
    let k = face_key(a, face);
    if (k == NO_KEY) { atomicAdd(&st[C_NOCHUNK], 1u); return; }
    set_insert(k, block, pr.frame);
}

// Adds a face and (with `block_faces`) the block's other camera-facing exposed faces: a
// pixel ray grazing a block edge may hit one face while rasterisation covers that pixel
// with the adjacent, nearly edge-on face of the same block.
fn add_face(a: vec3<i32>, face: u32, block: u32) {
    add_face_plain(a, face, block);
    if (pr.block_faces == 0u) { return; }
    let rel = a - vp.cam_block;
    for (var ax = 0u; ax < 3u; ax++) {
        if (ax == face / 2u) { continue; }
        var f = 9u;
        if (pr.origin[ax] < f32(rel[ax])) { f = ax * 2u + 1u; }
        if (pr.origin[ax] > f32(rel[ax] + 1)) { f = ax * 2u; }
        if (f < 6u && block_at_cached(a + normal_i(f)) == 0u) {
            add_face_plain(a, f, block);
        }
    }
}

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
    if (pr.incr != 0u) {
        let v = vec3<f32>(a - vp.cam_block) + vec3<f32>(0.5) - pr.origin;
        let z = dot(v, pr.forward);
        let inside = z > -1.0 && abs(dot(v, pr.right)) <= z * pr.tan_half_fov * pr.aspect + 1.0 && abs(dot(v, pr.up)) <= z * pr.tan_half_fov + 1.0;
        if (!inside) { keep = pr.keep_out; }
    }
    if (pr.frame - last >= keep) { return; }
    let face = key & 7u;
    let b = exposed(a, face);
    if (b == 0u) { return; }
    set_insert(key, b, last);
    atomicAdd(&st[C_SURV], 1u);
}

// ---- ray casting at lattice points ----
fn cast_ray(q: vec2<u32>) {
    let d = ray_dir(q);
    let h = trace(pr.origin, d, vp.max_dist);
    let idx = q.y * pr.lat_w + q.x;
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
        if (pr.incr == 0u && i == 0u) { atomicStore(&st[C_RAYS], n); }
        if (i >= n) { return; }
        let cx = i % (pr.cols + 1u);
        let cy = i / (pr.cols + 1u);
        if (pr.incr != 0u) {
            // Only corners of tiles that need discovery.
            var any = false;
            for (var k = 0u; k < 4u; k++) {
                let tx = i32(cx) - i32(k & 1u);
                let ty = i32(cy) - i32(k >> 1u);
                if (tx >= 0 && ty >= 0 && tx < i32(pr.cols) && ty < i32(pr.rows) && need[pr.need_off + u32(ty) * pr.cols + u32(tx)] != 0u) { any = true; }
            }
            if (!any) { return; }
            atomicAdd(&st[C_RAYS], 1u);
        }
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

var<workgroup> wg_tiles: atomic<u32>;
var<workgroup> wg_rays: atomic<u32>;
var<workgroup> wg_tbase: u32;
var<workgroup> wg_rbase: u32;

// Classification with workgroup-aggregated queue appends for splits (one global atomic
// per workgroup and queue instead of one per tile).
@compute @workgroup_size(256)
fn classify(@builtin(workgroup_id) wg: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    if (li == 0u) {
        atomicStore(&wg_tiles, 0u);
        atomicStore(&wg_rays, 0u);
    }
    workgroupBarrier();
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
    let tl = atomicAdd(&wg_tiles, nk);
    let rl = atomicAdd(&wg_rays, nr);
    workgroupBarrier();
    if (li == 0u) {
        wg_tbase = atomicAdd(&st[C_TILES + lv.level + 1u], atomicLoad(&wg_tiles));
        wg_rbase = atomicAdd(&st[C_RAYS + lv.level + 1u], atomicLoad(&wg_rays));
    }
    let tb = workgroupUniformLoad(&wg_tbase) + tl;
    let rb = workgroupUniformLoad(&wg_rbase) + rl;
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
        if (i >= pr.cols * pr.rows) { return; }
        if (pr.incr != 0u && need[pr.need_off + i] == 0u) { return; }
        t = vec2<u32>(i % pr.cols, i / pr.cols) * pr.tile;
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
    // Footprints are expanded cooperatively (one workgroup per tile) in `footprint`.
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
    let h = trace(pr.origin, d, vp.max_dist);
    if (!h.hit) { return; }
    atomicAdd(&st[C_AUDIT_HIT], 1u);
    let k = face_key(h.cell + vp.cam_block, face_entered(h.axis, d[h.axis] > 0.0));
    if (k == NO_KEY || !set_contains(k)) { atomicAdd(&st[C_MISS], 1u); }
}


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
// parallax rule), in changed chunks (rects), or at the screen border (no history here).
fn sky_known(p: vec2<i32>) -> bool {
    if (pr.has_prev == 0u) { return false; }
    let sx = (2.0 * (f32(p.x) + 0.5) / f32(pr.width) - 1.0) * pr.tan_half_fov * pr.aspect;
    let sy = (1.0 - 2.0 * (f32(p.y) + 0.5) / f32(pr.height)) * pr.tan_half_fov;
    let d = pr.forward + sx * pr.right + sy * pr.up;
    let z = dot(d, pr.prev_forward);
    if (z <= 0.0) { return false; }
    let u = dot(d, pr.prev_right) / (z * pr.prev_tan * pr.aspect);
    let v = dot(d, pr.prev_up) / (z * pr.prev_tan);
    let q = vec2<i32>(floor(vec2<f32>((u + 1.0) * 0.5 * f32(pr.width), (1.0 - v) * 0.5 * f32(pr.height))));
    if (any(q < vec2<i32>(0)) || q.x >= i32(pr.width) || q.y >= i32(pr.height)) { return false; }
    return sky_prev[u32(q.y) * pr.width + u32(q.x)] != 0u;
}

// After the final raster: which pixels are sky this frame. Every uncovered pixel that was
// not sky-known forced its tile through discovery, so uncovered now means confirmed sky.
@compute @workgroup_size(256)
fn sky_update(@builtin(workgroup_id) wg: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let i = tid(wg, nwg, li);
    if (i >= pr.width * pr.height) { return; }
    let d = textureLoad(depthtex, vec2<u32>(i % pr.width, i / pr.width), 0);
    sky_cur[i] = select(0u, 1u, d == 0.0);
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
    var n = pr.full != 0u || (pr.refresh != 0u && i % pr.refresh == pr.frame % pr.refresh);
    let rm = i32(pr.r_max);
    for (var dy = -rm; dy <= rm && !n; dy++) {
        for (var dx = -rm; dx <= rm; dx++) {
            let x = tx + dx;
            let y = ty + dy;
            if (x < 0 || y < 0 || x >= i32(pr.cols) || y >= i32(pr.rows)) { continue; }
            let v = need[u32(y) * pr.cols + u32(x)];
            if (v != 0u && max(abs(dx), abs(dy)) <= i32(v) - 1) { n = true; break; }
        }
    }
    for (var k = 0u; k < pr.n_rects && !n; k++) {
        let r = rects[k];
        if (u32(tx) >= r.x && u32(ty) >= r.y && u32(tx) <= r.z && u32(ty) <= r.w) { n = true; }
    }
    need[pr.need_off + i] = select(0u, 1u, n);
    let age = need[2u * pr.need_off + i];
    need[2u * pr.need_off + i] = select(age + 1u, 0u, n);
    if (n) { atomicAdd(&st[C_NEED], 1u); }
}
