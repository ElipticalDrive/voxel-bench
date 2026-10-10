// gpu-beam: conservative per-tile beam prepass + full-resolution DDA starting at the
// beam's safe distance.

struct Camera {
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, mode: u32,
    origin: vec3<f32>, sky: u32,
    width: u32, height: u32, tile: u32, tiles_x: u32,
}

@group(0) @binding(0) var<uniform> cam: Camera;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<storage, read_write> beam: array<f32>;
@group(0) @binding(3) var<storage, read_write> stats: array<atomic<u32>, 2>; // sum of skipped blocks, tiles

fn ray_dir(px: vec2<f32>) -> vec3<f32> {
    let sx = (2.0 * px.x / f32(cam.width) - 1.0) * cam.tan_half_fov * cam.aspect;
    let sy = (1.0 - 2.0 * px.y / f32(cam.height)) * cam.tan_half_fov;
    return normalize(cam.forward + sx * cam.right + sy * cam.up);
}

fn chunk_solid(c: vec3<i32>) -> bool {
    let v = lookup_chunk(chunk_key(c, 0u));
    if (v == EMPTY) { return false; }
    if ((v & UNIFORM_BIT) != 0u && (v & 0xffffu) == 0u) { return false; }
    return true;
}

// Pass 1: one thread per tile. March the tile's centre ray in steps of 32 blocks (at most 64 steps); at each
// sample, every chunk overlapping a box that covers the whole cone slab for that step is
// checked. Returns the depth up to which the cone is provably empty.
@compute @workgroup_size(8, 8)
fn prepass(@builtin(global_invocation_id) gid: vec3<u32>) {
    let tiles_y = (cam.height + cam.tile - 1u) / cam.tile;
    if (gid.x >= cam.tiles_x || gid.y >= tiles_y) { return; }
    let p0 = vec2<f32>(gid.xy * cam.tile);
    let p1 = min(p0 + vec2<f32>(f32(cam.tile)), vec2<f32>(f32(cam.width), f32(cam.height)));
    let n = ray_dir(0.5 * (p0 + p1));
    // Max tangent of the angle between the centre ray and any ray of the tile (corners).
    var k = 0.0;
    let cs = array<vec2<f32>, 4>(p0, vec2<f32>(p1.x, p0.y), vec2<f32>(p0.x, p1.y), p1);
    for (var i = 0u; i < 4u; i++) {
        let c = clamp(dot(ray_dir(cs[i]), n), 1e-3, 1.0);
        k = max(k, sqrt(max(1.0 - c * c, 0.0)) / c);
    }
    k = k * 1.01 + 1e-4;
    let o = cam.origin;
    var safe = 0.0;
    let max_d = vp.max_dist;
    for (var i = 0u; i < 64u; i++) {
        let tc = 16.0 + 32.0 * f32(i);         // slab [tc-16, tc+16] in projected depth
        let lo_d = tc - 16.0;
        if (lo_d > max_d) { safe = max_d; break; }
        let h = 16.0 + k * (tc + 16.0) + 1.0;    // half extent covering the slab's cross-section
        if (h > 48.0) { break; }
        let p = o + n * tc;
        let a = (vec3<i32>(floor(p - vec3<f32>(h))) + vp.cam_block) >> vec3<u32>(5u);
        let b = (vec3<i32>(floor(p + vec3<f32>(h))) + vp.cam_block) >> vec3<u32>(5u);
        var hit = false;
        for (var x = a.x; x <= b.x && !hit; x++) {
            for (var y = a.y; y <= b.y && !hit; y++) {
                for (var z = a.z; z <= b.z && !hit; z++) {
                    hit = chunk_solid(vec3<i32>(x, y, z));
                }
            }
        }
        if (hit) { break; }
        safe = tc + 16.0;
    }
    // Margin for float error at the start cell.
    let skip = max(safe - 2.0, 0.0);
    beam[gid.y * cam.tiles_x + gid.x] = skip;
    atomicAdd(&stats[0], u32(skip));
    atomicAdd(&stats[1], 1u);
}

// `trace` from voxels.wgsl with a minimum start distance `tmin`.
fn trace_from(o: vec3<f32>, d_in: vec3<f32>, max_t: f32, tmin: f32) -> Hit {
    var res: Hit;
    res.hit = false;
    let d = select(d_in, vec3<f32>(1e-20), abs(d_in) < vec3<f32>(1e-20));
    let inv = 1.0 / d;
    let stp = select(vec3<i32>(-1), vec3<i32>(1), d > vec3<f32>(0.0));
    let t0 = (vec3<f32>(vp.bounds_min) - o) * inv;
    let t1 = (vec3<f32>(vp.bounds_max) - o) * inv;
    let tlo = min(t0, t1);
    let thi = max(t0, t1);
    let tbox = max(max(max(tlo.x, tlo.y), tlo.z), 0.0);
    let tnear = max(tbox, tmin);
    let tfar = min(min(min(thi.x, thi.y), thi.z), max_t);
    if (tnear > tfar) { return res; }

    let start_cell = vec3<i32>(floor(o));
    var cell = clamp(vec3<i32>(floor(o + d * tnear)), vp.bounds_min, vp.bounds_max - vec3<i32>(1));
    var axis = 3u;
    if (tbox > 0.0 && tnear == tbox) {
        axis = select(select(2u, 1u, tlo.y >= tlo.z), 0u, tlo.x >= tlo.y && tlo.x >= tlo.z);
    }
    var cached = vec3<i32>(0x7fffffff);
    var val = EMPTY;
    res.t = tnear;
    for (var i = 0u; i < vp.max_steps; i++) {
        let abs_cell = cell + vp.cam_block;
        let key = chunk_key(abs_cell >> vec3<u32>(5u), 0u);
        if (any(key != cached)) {
            cached = key;
            val = lookup_chunk(key);
        }
        var size = 32;
        if (val != EMPTY) {
            let local = abs_cell & vec3<i32>(31);
            var b = 0u;
            if ((val & UNIFORM_BIT) != 0u) {
                b = val & 0xffffu;
                size = 1;
            } else if (brick_occupied(val, local)) {
                b = voxel(val, local);
                size = 1;
            } else {
                size = 4;
            }
            if (b != 0u && any(cell != start_cell)) {
                res.hit = true;
                res.cell = cell;
                res.block = b;
                res.axis = axis;
                res.lod = 0u;
                return res;
            }
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

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width || gid.y >= cam.height) { return; }
    let d = ray_dir(vec2<f32>(gid.xy) + vec2<f32>(0.5));
    let o = cam.origin;
    var tmin = 0.0;
    if (cam.tile > 1u) {
        let tc = gid.xy / cam.tile;
        tmin = beam[tc.y * cam.tiles_x + tc.x];
    }
    // Mesh mode: rays end at the mesh surface; a voxel in front of it still wins.
    let mt = mesh_t(gid.xy, d, cam.forward);
    let h = trace_from(o, d, min(vp.max_dist, mt), tmin);
    var color = unpack4x8unorm(cam.sky);
    if (!h.hit && mt < 1e29) { color = mesh_color(gid.xy); }
    if (h.hit) {
        let face = face_entered(h.axis, d[h.axis] > 0.0);
        let p = o + d * h.t;
        let local = p - vec3<f32>(h.cell);
        let texel = surface_texel(h.block, face, local);
        // A branch, not `select`: select evaluates both sides, so unlit frames paid for
        // lighting and shadow rays (unlit and lit GPU times were identical).
        color = texel;
        if (cam.mode == 1u) { color = shade_lit(texel, p, face_normal(face)); }
    }
    textureStore(out_tex, vec2<i32>(gid.xy), color);
}
