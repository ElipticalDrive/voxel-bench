// GPU voxel world shared by GPU renderers (bind group 1). Positions are relative to the
// camera's integer block `vp.cam_block`, so precision does not degrade far from the origin.

struct VoxelParams {
    cam_block: vec3<i32>, max_steps: u32,
    bounds_min: vec3<i32>, table_mask: u32,   // loaded region, camera-relative blocks
    bounds_max: vec3<i32>, max_probe: u32,
    sun_dir: vec3<f32>, has_sun: u32,         // direction towards the sun
    sun_color: vec3<f32>, max_dist: f32,
    ambient: vec3<f32>, exposure: f32,
    light_mask: u32, light_max_probe: u32, shadows: u32, max_lod: u32, // shadows: 0 none, 1 sun, 2 all
    lod_radius: u32, _l0: u32, _l1: u32, _l2: u32,   // streamed Chebyshev radius per LOD (chunks)
}

struct Light { block: vec3<i32>, radius: f32, frac: vec3<f32>, intensity: f32, color: vec3<f32>, _pad: f32 }
struct LightCell { key: vec3<i32>, start: u32, count: u32, _p0: u32, _p1: u32, _p2: u32 }

@group(1) @binding(0) var<uniform> vp: VoxelParams;
@group(1) @binding(1) var<storage, read> table: array<vec4<u32>>;
@group(1) @binding(2) var<storage, read> pool: array<u32>;
@group(1) @binding(3) var<storage, read> bricks: array<u32>;
@group(1) @binding(4) var<storage, read> face_tex: array<u32>;
@group(1) @binding(5) var<storage, read> texels: array<u32>;
@group(1) @binding(6) var<storage, read> lights: array<Light>;
@group(1) @binding(7) var<storage, read> light_cells: array<LightCell>;
@group(1) @binding(8) var<storage, read> light_idx: array<u32>;

const EMPTY: u32 = 0xffffffffu;
const TOMB: u32 = 0xfffffffeu;
const UNIFORM_BIT: u32 = 0x80000000u;

fn hash3(c: vec3<i32>) -> u32 {
    let v = bitcast<vec3<u32>>(c);
    var h = (v.x * 0x8da6b343u) ^ (v.y * 0xd8163841u) ^ (v.z * 0xcb1ab31fu);
    return h ^ (h >> 16u);
}

fn lookup_chunk(c: vec3<i32>) -> u32 {
    var i = hash3(c) & vp.table_mask;
    for (var n = 0u; n <= vp.max_probe; n++) {
        let e = table[i];
        if (e.w == EMPTY) { return EMPTY; }
        if (e.w != TOMB && all(bitcast<vec3<i32>>(e.xyz) == c)) { return e.w; }
        i = (i + 1u) & vp.table_mask;
    }
    return EMPTY;
}

fn voxel(slot: u32, l: vec3<i32>) -> u32 {
    let idx = u32(l.x | (l.z << 5u) | (l.y << 10u));
    let w = pool[slot * 16384u + (idx >> 1u)];
    return (w >> ((idx & 1u) * 16u)) & 0xffffu;
}

fn brick_occupied(slot: u32, l: vec3<i32>) -> bool {
    let b = l >> vec3<u32>(2u);
    let bi = u32(b.x | (b.z << 3u) | (b.y << 6u));
    return ((bricks[slot * 16u + (bi >> 5u)] >> (bi & 31u)) & 1u) != 0u;
}

struct Hit { hit: bool, t: f32, cell: vec3<i32>, block: u32, axis: u32, lod: u32 }

// Chunk-table key: LOD folded into y, mirroring `voxels::table_key`.
fn chunk_key(c: vec3<i32>, lod: u32) -> vec3<i32> {
    return vec3<i32>(c.x, c.y * 8 + i32(lod), c.z);
}

// Finest LOD whose streamed region (a cube of `lod_radius` chunks at that LOD around the
// camera) contains `abs_cell`.
fn lod_of(abs_cell: vec3<i32>) -> u32 {
    if (vp.max_lod == 0u) { return 0u; }
    let r = vec3<i32>(i32(vp.lod_radius));
    for (var l = 0u; l < vp.max_lod; l++) {
        let s = vec3<u32>(5u + l);
        if (all(abs((abs_cell >> s) - (vp.cam_block >> s)) <= r)) { return l; }
    }
    return vp.max_lod;
}

// Ignores the cell containing `o`, like the reference.
fn trace(o: vec3<f32>, d_in: vec3<f32>, max_t: f32) -> Hit {
    var res: Hit;
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
    var cached = vec3<i32>(0x7fffffff);
    var val = EMPTY;
    res.t = tnear; // entry distance, for hits in the very first cell
    for (var i = 0u; i < vp.max_steps; i++) {
        let abs_cell = cell + vp.cam_block;
        // With LOD, cells (and chunks, bricks) grow 2x per level away from the camera.
        let lod = lod_of(abs_cell);
        let key = chunk_key(abs_cell >> vec3<u32>(5u + lod), lod);
        if (any(key != cached)) {
            cached = key;
            val = lookup_chunk(key);
        }
        var size = 32 << lod;
        if (val != EMPTY) {
            let local = (abs_cell >> vec3<u32>(lod)) & vec3<i32>(31);
            var b = 0u;
            if ((val & UNIFORM_BIT) != 0u) {
                b = val & 0xffffu;
                size = 1 << lod;
            } else if (brick_occupied(val, local)) {
                b = voxel(val, local);
                size = 1 << lod;
            } else {
                size = 4 << lod;
            }
            if (b != 0u && any(cell != start_cell)) {
                res.hit = true;
                res.cell = cell;
                res.block = b;
                res.axis = axis;
                res.lod = lod;
                return res;
            }
        }
        // Step to the exit of the current cell at this level.
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

fn lookup_light_cell(c: vec3<i32>) -> vec2<u32> {
    var i = hash3(c) & vp.light_mask;
    for (var n = 0u; n <= vp.light_max_probe; n++) {
        let e = light_cells[i];
        if (e.count == 0u) { return vec2<u32>(0u); }
        if (all(e.key == c)) { return vec2<u32>(e.start, e.count); }
        i = (i + 1u) & vp.light_mask;
    }
    return vec2<u32>(0u);
}

// Texel colour (sRGB, as stored) of `block`'s `face` at block-local position `local`.
fn surface_texel(block: u32, face: u32, local: vec3<f32>) -> vec4<f32> {
    let uv = face_uv(face, local);
    let tex = face_tex[block * 6u + face];
    return unpack4x8unorm(texels[tex * 256u + texel_coord(uv.y) * 16u + texel_coord(uv.x)]);
}

