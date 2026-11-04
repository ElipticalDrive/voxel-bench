// The quadtree's GPU face set and discovery buffers (module `quadtree::set`), shared by the sampler (`gpu.wgsl`) and hole detection (`holes/coverage/gpu.wgsl`),
// which are separate roots over it. Was gpu-discover's `discover.wgsl`.
//
// gpu-discover: GPU face discovery. Adaptive screen-space quadtree of corner rays; coherent
// tiles add every exposed face of the hit plane inside their footprint to a GPU hash set
// of world-space faces (S). Faces persist for `keep` frames (survivor pass).
//
// Face key (u32): chunk slot (14 bits) << 18 | local index (x | y<<5 | z<<10) << 3 | face.
// Hash set stores key + 1 (0 = empty) and the frame the face was last confirmed.
//
// Incremental mode (hole detection; the sampler's shader def `HOLES`): last frame's face list is first rasterised at the current
// camera into a face-id buffer. `mark` stamps every face that covers a pixel as seen (seen
// faces survive; unseen ones expire after `keep` frames). `need_raw` / `need_final` pick
// the level-0 tiles that need discovery: uncovered pixels, occlusion edges whose parallax
// under the camera's translation can reveal something, changed chunks' screen rectangles,
// and a round-robin refresh. Level 0 casts and classifies only those tiles.
//
// Event-driven frames (`events`, chosen on the host): frames where nothing changed skip
// discovery entirely. A camera that only rotated keeps geometric visibility, so `still`
// frames skip seen-marking and the parallax test, and faces inside the view count as
// seen; but rotation moves pixel centres, so small faces can newly cover one. With
// `still_holes = now` the face-id raster still finds such holes in the same frame; with
// `late`, `need_still` discovers only the entering strip, changed tiles, holes found by
// `sky_update` after the previous raster, and the refresh.

struct Params {
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, frame: u32,
    origin: vec3<f32>, tile: u32,
    width: u32, height: u32, off_x: u32, off_y: u32,
    lat_w: u32, lat_h: u32, cols: u32, rows: u32,
    keep: u32, criterion: u32, sky_tile: u32, max_fp: u32,   // criterion: 0 plane, 1 face
    table_mask: u32, list_cap: u32, chunk_mask: u32, chunk_probe: u32,
    queue_cap: u32, parity: u32, strict: u32, still: u32,   // still: rotation / world-change-only frame
    closure_px: u32, closure_box: i32, block_faces: u32, inline_fp: u32,  // inline_fp: footprints up to this many cells expand in `classify`
    unused: u32, refresh: u32, par_k: f32, changed: u32, // par_k = f_px * |camera move| / near; changed: tile mask non-empty
    full: u32, r_max: u32, need_off: u32, par_thresh: f32,
    prev_forward: vec3<f32>, prev_tan: f32,               // previous camera, for sky reprojection
    prev_right: vec3<f32>, has_prev: u32,
    prev_up: vec3<f32>, keep_out: u32,                    // keep for faces outside the frustum (incremental)
    morton: u32, morton_side: u32, _qa: u32, _qb: u32,     // level 0 enumerated along a Z-curve over a morton_side^2 grid
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
@group(0) @binding(18) var<storage, read_write> need: array<u32>;  // [0..) reveal radius + 1 per tile; [need_off..) why discovered (0 = not); [2 need_off..) frames since discovered; [3 need_off..) hole seen after the last raster
@group(0) @binding(19) var idtex: texture_2d<u32>;                  // previous list's face index + 1 per pixel
@group(0) @binding(20) var depthtex: texture_depth_2d;
@group(0) @binding(21) var<storage, read> changed_tiles: array<u32>; // per level-0 tile: covered by an edited block or a changed chunk
@group(0) @binding(22) var<storage, read_write> sky_cur: array<u32>;  // 1 = sky in this frame's final raster
@group(0) @binding(23) var<storage, read> sky_prev: array<u32>;
@group(0) @binding(24) var<storage, read_write> cast_frame: array<u32>; // per lattice point: frame its ray was last cast (`rays` view)
@group(0) @binding(25) var<storage, read_write> hist: array<atomic<u32>>;  // painter order: bucket counts, then offsets
@group(0) @binding(26) var<storage, read_write> sorted: array<vec4<u32>>;  // painter order: the face list, farthest first
@group(0) @binding(27) var<storage, read_write> merged: array<vec4<u32>>;  // row merge: (start key, seam edges, block, fresh | run length << 1); count in lcount[2]

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
const C_SPLIT_PX: u32 = 38u;   // faces added by 1-px tiles
const C_FP_FACES: u32 = 39u; // faces added by coherent tiles
const C_CLOSURE: u32 = 40u;  // tiles accepted by local closure
const C_CL_FACES: u32 = 41u; // faces added by local closure
const C_NEED: u32 = 42u;     // level-0 tiles needing discovery (incremental)
const C_RCLAMP: u32 = 43u;   // tiles whose reveal radius hit r_max
const C_NEW: u32 = 44u;      // faces new to S this frame (not carried over by survive)

// Why a level-0 tile was discovered (bits of need[need_off + tile]; the `tiles` view).
const WHY_FULL: u32 = 1u;
const WHY_REFRESH: u32 = 2u;
const WHY_CHUNK: u32 = 4u;
const WHY_UNCOVERED: u32 = 8u;
const WHY_PARALLAX: u32 = 16u;
const WHY_STRIP: u32 = 32u;
const C_JOBS: u32 = 48u;     // + level: footprint jobs queued
const JOB_CAP: u32 = 262144u;

// Inverse of 2D Morton interleaving: (x, y) of Z-curve index i (x in the even bits).
fn compact_bits(v: u32) -> u32 {
    var x = v & 0x55555555u;
    x = (x | (x >> 1u)) & 0x33333333u;
    x = (x | (x >> 2u)) & 0x0f0f0f0fu;
    x = (x | (x >> 4u)) & 0x00ff00ffu;
    x = (x | (x >> 8u)) & 0x0000ffffu;
    return x;
}

// Level-0 work item `i` as grid coordinates: row-major, or along a Z-curve so that a
// wave's threads cover a compact square of the screen (rays then stay coherent).
fn grid_of(i: u32, w: u32) -> vec2<u32> {
    if (pr.morton != 0u) { return vec2<u32>(compact_bits(i), compact_bits(i >> 1u)); }
    return vec2<u32>(i % w, i / w);
}

fn tid(wg: vec3<u32>, nwg: vec3<u32>, li: u32) -> u32 {
    return (wg.y * nwg.x + wg.x) * WG + li;
}

// Primary ray through the centre of lattice point q (pixel q - offset); same arithmetic
// as gpu-dda and the raster fragment shader.
fn ray_dir(q: vec2<u32>) -> vec3<f32> {
    let px = f32(i32(q.x) - i32(pr.off_x));
    let py = f32(i32(q.y) - i32(pr.off_y));
    return camera_ray(pr.forward, pr.right, pr.up, pr.tan_half_fov, pr.aspect, vec2<f32>(px, py) + 0.5, vec2<f32>(f32(pr.width), f32(pr.height)));
}

fn normal_i(face: u32) -> vec3<i32> {
    var n = vec3<i32>(0);
    n[face / 2u] = select(1, -1, (face & 1u) == 1u);
    return n;
}

// Top-cell lookup cache for loops over neighbouring cells.
var<private> ck: vec3<i32> = vec3<i32>(0x7fffffff);
var<private> cv: u32 = 0xffffffffu;

fn block_at_cached(a: vec3<i32>) -> u32 {
    let k = a >> vec3<u32>(S_TOP);
    if (any(k != ck)) {
        ck = k;
        cv = s_top(k);
    }
    if (cv == EMPTY) { return 0u; }
    if ((cv & UNIFORM_BIT) != 0u) { return cv & 0xffffu; }
    return s_block(cv, a & vec3<i32>((1 << S_TOP) - 1));
}

fn exposed(a: vec3<i32>, face: u32) -> u32 {
    let b = block_at_cached(a);
    if (b == 0u || block_at_cached(a + normal_i(face)) != 0u) { return 0u; }
    return b;
}

fn chunk_slot(c: vec3<i32>) -> i32 {
    var i = hash_cell(c) & pr.chunk_mask;
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

// `fresh`: inserted by discovery rather than carried over by `survive` (list.w = 1).
fn set_insert(key: u32, block: u32, last: u32, fresh: u32) {
    let k1 = key + 1u;
    var i = hash_u(key) & pr.table_mask;
    for (var n = 0u; n < 128u; n++) {
        let r = atomicCompareExchangeWeak(&keys[i], 0u, k1);
        if (r.exchanged) {
            let j = atomicAdd(&lcount[pr.parity], 1u);
            if (j < pr.list_cap) {
                list[j] = vec4<u32>(key, i, block, fresh);
                if (fresh != 0u) { atomicAdd(&st[C_NEW], 1u); }
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
    set_insert(k, block, pr.frame, 1u);
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
