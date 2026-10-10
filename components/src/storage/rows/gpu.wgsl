// Storage `rows` (from gpu-bitmask): the shared pool and 4^3 brick bits, plus per slot in
// `aux` 32 x 32 u32 occupancy rows (row y * 32 + z, bit x). Fast path for `s_row`.

const S_LEVELS: u32 = 0x5u;

fn s_row(slot: u32, y: i32, z: i32) -> u32 {
    return aux[slot * AUX_WORDS + u32(y * 32 + z)];
}

fn s_occupied(slot: u32, level: u32, c: vec3<i32>) -> bool {
    if (level == 0u) { return ((s_row(slot, c.y, c.z) >> u32(c.x)) & 1u) != 0u; }
    return brick_occupied(slot, c << vec3<u32>(2u));
}

fn s_probe(slot: u32, l: vec3<i32>) -> vec2<u32> {
    if (!brick_occupied(slot, l)) { return vec2<u32>(0u, 4u); }
    if (((s_row(slot, l.y, l.z) >> u32(l.x)) & 1u) == 0u) { return vec2<u32>(0u, 1u); }
    return vec2<u32>(voxel(slot, l), 1u);
}

fn s_block(slot: u32, l: vec3<i32>) -> u32 {
    return voxel(slot, l);
}
