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
    lod_radius: u32, _la: u32, _lb: u32, _lc: u32,   // streamed Chebyshev radius per LOD (chunks)
}

struct Light { block: vec3<i32>, radius: f32, frac: vec3<f32>, intensity: f32, color: vec3<f32>, _pad: f32 }
struct LightCell { key: vec3<i32>, start: u32, count: u32, _pa: u32, _pb: u32, _pc: u32 }

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

fn hash_cell(c: vec3<i32>) -> u32 {
    let v = bitcast<vec3<u32>>(c);
    var h = (v.x * 0x8da6b343u) ^ (v.y * 0xd8163841u) ^ (v.z * 0xcb1ab31fu);
    return h ^ (h >> 16u);
}

fn lookup_chunk(c: vec3<i32>) -> u32 {
    var i = hash_cell(c) & vp.table_mask;
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

// The hit record walks return. Core: hit, t, cell, block, axis. Optional (`caps::Cap`):
// `lod`, the level of the hit cell (walks with `Lod`); walks without it leave it at 0, the
// zero a `var` of a struct starts with (that zero is the adapter: level 0).
struct Hit { hit: bool, t: f32, cell: vec3<i32>, block: u32, axis: u32, lod: u32 }

// Optional, `Steps`: the walk's loop iterations for the current ray; walks with it count
// here, consumers (`view = "steps"`) reset and read it. Unread, the counting compiles away.
var<private> ct_iters: u32;

// What shading reads from visibility (the shading components' `shade_surface`): the
// camera-relative surface point, its normal (the face's, unless a visibility strategy has
// a better one), the point within the block (texture lookup), the block and its face.
struct Surface { p: vec3<f32>, n: vec3<f32>, local: vec3<f32>, block: u32, face: u32 }

// The surface at hit `h` of ray (`o`, `d`): the adapter for ray walks.
fn surface_of(h: Hit, o: vec3<f32>, d: vec3<f32>) -> Surface {
    let face = face_entered(h.axis, d[h.axis] > 0.0);
    let p = o + d * h.t;
    // LOD cells span several blocks; textures still tile once per block.
    var local = p - vec3<f32>(h.cell);
    if (h.lod > 0u) { local = p - floor(p); }
    return Surface(p, face_normal(face), local, h.block, face);
}

// The stored hit record (the visibility buffer, PLAN step 12): what a visibility strategy
// writes per pixel when shading is deferred, and all a shading pass needs to rebuild the
// `Surface`. Core, 8 bytes (an rg32uint texel): x = `t`, the distance along the pixel's unit
// primary ray (f32 bits); y = 0 for no hit (sky), else bit 0 set, the face in bits 1-3 and
// the block in bits 4-19. Bits 20-31 belong to optional capabilities (mesh surfaces, layers
// for transparency; IDEAS §23) and are 0 in pipelines without them.
fn pack_hit(t: f32, block: u32, face: u32) -> vec2<u32> {
    return vec2<u32>(bitcast<u32>(t), 1u | (face << 1u) | ((block & 0xffffu) << 4u));
}

// The surface of record `r` on ray (`o`, `d`): the point at `t`, the face's normal and the
// cell behind the face (the normal axis from the face plane, the others from the point),
// as `face_plane_hit` finds it; a cell spans one block whatever the walk's LOD, so textures
// tile per block as `surface_of` makes them.
fn record_surface(r: vec2<u32>, o: vec3<f32>, d: vec3<f32>) -> Surface {
    let face = (r.y >> 1u) & 7u;
    let a = face / 2u;
    let p = o + d * bitcast<f32>(r.x);
    var cell = floor(p);
    cell[a] = round(p[a]) - select(0.0, 1.0, (face & 1u) == 0u);
    return Surface(p, face_normal(face), p - cell, (r.y >> 4u) & 0xffffu, face);
}

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


fn lookup_light_cell(c: vec3<i32>) -> vec2<u32> {
    var i = hash_cell(c) & vp.light_mask;
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

