// gpu-tree64: 64^3 regions stored as sparse 64-trees (4x4x4 children per node,
// 64-bit child masks, children packed by popcount, leaves store only solid voxel ids).
// Traversal: descend from the region root to the largest empty cell containing the
// current position, then DDA-step past it.

struct Camera {
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, mode: u32,
    origin: vec3<f32>, sky: u32,
    width: u32, height: u32, region_mask: u32, region_probe: u32,
    max_steps: u32, _p0: u32, _p1: u32, _p2: u32,
}

struct Node { mask_lo: u32, mask_hi: u32, child: u32, block: u32 }

@group(0) @binding(0) var<uniform> cam: Camera;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<storage, read> nodes: array<Node>;
@group(0) @binding(3) var<storage, read> leaf_ids: array<u32>;
@group(0) @binding(4) var<storage, read> regions: array<vec4<u32>>;

const NO_REGION: u32 = 0xffffffffu;

fn lookup_region(c: vec3<i32>) -> u32 {
    var i = hash3(c) & cam.region_mask;
    for (var n = 0u; n <= cam.region_probe; n++) {
        let e = regions[i];
        if (e.w == NO_REGION) { return NO_REGION; }
        if (all(bitcast<vec3<i32>>(e.xyz) == c)) { return e.w; }
        i = (i + 1u) & cam.region_mask;
    }
    return NO_REGION;
}

fn has_bit(n: Node, b: u32) -> bool {
    if (b < 32u) { return ((n.mask_lo >> b) & 1u) != 0u; }
    return ((n.mask_hi >> (b - 32u)) & 1u) != 0u;
}

fn rank(n: Node, b: u32) -> u32 {
    if (b < 32u) { return countOneBits(n.mask_lo & ((1u << b) - 1u)); }
    return countOneBits(n.mask_lo) + countOneBits(n.mask_hi & ((1u << (b - 32u)) - 1u));
}

// (block, size): block id at `local` (0..63 within the region) or 0 and the edge length
// of the empty cell containing it.
fn query(root: u32, local: vec3<i32>) -> vec2<u32> {
    var n = nodes[root];
    var shift = 6u;
    loop {
        if (n.block != 0u) { return vec2<u32>(n.block, 1u); }
        shift -= 2u;
        let c = (local >> vec3<u32>(shift)) & vec3<i32>(3);
        let b = u32(c.x | (c.z << 2u) | (c.y << 4u));
        if (!has_bit(n, b)) { return vec2<u32>(0u, 1u << shift); }
        let idx = n.child + rank(n, b);
        if (shift == 0u) {
            return vec2<u32>((leaf_ids[idx >> 1u] >> ((idx & 1u) * 16u)) & 0xffffu, 1u);
        }
        n = nodes[idx];
    }
    return vec2<u32>(0u, 1u);
}

struct TreeHit { hit: bool, t: f32, cell: vec3<i32>, block: u32, axis: u32 }

fn trace_tree(o: vec3<f32>, d_in: vec3<f32>, max_t: f32) -> TreeHit {
    var res: TreeHit;
    res.hit = false;
    let d = select(d_in, vec3<f32>(1e-20), abs(d_in) < vec3<f32>(1e-20));
    let inv = 1.0 / d;
    let stp = select(vec3<i32>(-1), vec3<i32>(1), d > vec3<f32>(0.0));
    let t0 = (vec3<f32>(vp.bounds_min) - o) * inv;
    let t1 = (vec3<f32>(vp.bounds_max) - o) * inv;
    let tlo = min(t0, t1);
    let thi = max(t0, t1);
    let tnear = max(max(max(tlo.x, tlo.y), tlo.z), 0.0);
    let tfar = min(min(min(thi.x, thi.y), thi.z), max_t);
    if (tnear > tfar) { return res; }
    let start_cell = vec3<i32>(floor(o));
    var cell = clamp(vec3<i32>(floor(o + d * tnear)), vp.bounds_min, vp.bounds_max - vec3<i32>(1));
    var axis = 3u;
    if (tnear > 0.0) {
        axis = select(select(2u, 1u, tlo.y >= tlo.z), 0u, tlo.x >= tlo.y && tlo.x >= tlo.z);
    }
    res.t = tnear;
    var cached = vec3<i32>(0x7fffffff);
    var root = NO_REGION;
    for (var i = 0u; i < cam.max_steps; i++) {
        let abs_cell = cell + vp.cam_block;
        let rc = abs_cell >> vec3<u32>(6u);
        if (any(rc != cached)) {
            cached = rc;
            root = lookup_region(rc);
        }
        var size = 64;
        if (root != NO_REGION) {
            let q = query(root, abs_cell & vec3<i32>(63));
            if (q.x != 0u && any(cell != start_cell)) {
                res.hit = true;
                res.cell = cell;
                res.block = q.x;
                res.axis = axis;
                return res;
            }
            size = i32(q.y);
        }
        let base = (abs_cell & vec3<i32>(-size)) - vp.cam_block;
        let bound = base + select(vec3<i32>(0), vec3<i32>(size), stp > vec3<i32>(0));
        let tb = (vec3<f32>(bound) - o) * inv;
        var a = 0u;
        if (tb.y < tb[a]) { a = 1u; }
        if (tb.z < tb[a]) { a = 2u; }
        res.t = tb[a];
        if (res.t > tfar) { return res; }
        var nc = clamp(vec3<i32>(floor(o + d * res.t)), base, base + vec3<i32>(size - 1));
        nc[a] = select(base[a] - 1, bound[a], stp[a] > 0);
        cell = nc;
        axis = a;
    }
    return res;
}

fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool {
    return trace_tree(p, d, max_t).hit;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width || gid.y >= cam.height) { return; }
    let sx = (2.0 * (f32(gid.x) + 0.5) / f32(cam.width) - 1.0) * cam.tan_half_fov * cam.aspect;
    let sy = (1.0 - 2.0 * (f32(gid.y) + 0.5) / f32(cam.height)) * cam.tan_half_fov;
    let d = normalize(cam.forward + sx * cam.right + sy * cam.up);
    let o = cam.origin;
    let h = trace_tree(o, d, vp.max_dist);
    var color = unpack4x8unorm(cam.sky);
    if (h.hit) {
        let face = face_entered(h.axis, d[h.axis] > 0.0);
        let p = o + d * h.t;
        let texel = surface_texel(h.block, face, p - vec3<f32>(h.cell));
        // A branch, not `select`: select evaluates both sides, so unlit frames paid for
        // lighting and shadow rays (unlit and lit GPU times were identical).
        color = texel;
        if (cam.mode == 1u) { color = shade_lit(texel, p, face_normal(face)); }
    }
    textureStore(out_tex, vec2<i32>(gid.xy), color);
}
