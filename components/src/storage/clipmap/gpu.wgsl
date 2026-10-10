// Storage `clipmap` (from gpu-clipmap): a camera-centred toroidal volume of 3D textures,
// addressed by absolute position modulo its size: no hash table, only textureLoads. Chunk
// `c` lives in slot `c mod slots`; the tag texture says which chunk owns a slot, so stale
// or foreign data reads as empty. The handle is the slot, packed x | y << 10 | z << 20.

const S_TOP: u32 = 5u;
const S_LEVELS: u32 = 0x5u;

struct Clip { slots: vec3<i32>, _p: u32 }   // chunk slots per axis
@group(2) @binding(4) var<uniform> clip: Clip;
@group(2) @binding(5) var clip_vox: texture_3d<u32>;   // block ids, slots * 32 per axis
@group(2) @binding(6) var clip_occ: texture_3d<u32>;   // 1 per non-empty 4^3 brick, slots * 8 per axis
@group(2) @binding(7) var clip_tags: texture_3d<u32>;  // per chunk slot: owner tag (0 = empty)

fn clip_tag(c: vec3<i32>) -> u32 {
    let u = bitcast<vec3<u32>>(c) & vec3<u32>(1023u);
    return u.x | (u.y << 10u) | (u.z << 20u) | 0x80000000u;
}

fn s_top(key: vec3<i32>) -> u32 {
    let s = ((key % clip.slots) + clip.slots) % clip.slots;
    if (textureLoad(clip_tags, s, 0).r != clip_tag(key)) { return EMPTY; }
    return u32(s.x | (s.y << 10u) | (s.z << 20u));
}

fn clip_slot(h: u32) -> vec3<i32> {
    return vec3<i32>(i32(h & 1023u), i32((h >> 10u) & 1023u), i32(h >> 20u));
}

fn s_block(h: u32, l: vec3<i32>) -> u32 {
    return textureLoad(clip_vox, clip_slot(h) * 32 + l, 0).r;
}

fn s_occupied(h: u32, level: u32, c: vec3<i32>) -> bool {
    if (level == 0u) { return s_block(h, c) != 0u; }
    return textureLoad(clip_occ, clip_slot(h) * 8 + (c << vec3<u32>(level - 2u)), 0).r != 0u;
}

fn s_probe(h: u32, l: vec3<i32>) -> vec2<u32> {
    if (textureLoad(clip_occ, clip_slot(h) * 8 + (l >> vec3<u32>(2u)), 0).r == 0u) { return vec2<u32>(0u, 4u); }
    return vec2<u32>(s_block(h, l), 1u);
}
