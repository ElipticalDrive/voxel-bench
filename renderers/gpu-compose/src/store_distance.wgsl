// Storage `distance` (from gpu-sdf): per slot in `aux`, the Chebyshev distance (u8, packed
// 4 per word, dense x + 32 z + 1024 y order) from each cell to the nearest solid cell of
// its chunk; 0 = solid. Block ids from the shared pool. Fast path for `s_dist`.
// Cube queries are conservative: a cube is reported empty only when the distance at its
// first cell proves it (exact at level 0, as the interface requires).

const S_LEVELS: u32 = 0x1fu;

fn d_at(slot: u32, l: vec3<i32>) -> u32 {
    let idx = u32(l.x | (l.z << 5u) | (l.y << 10u));
    return (aux[slot * AUX_WORDS + (idx >> 2u)] >> ((idx & 3u) * 8u)) & 0xffu;
}

fn s_dist(slot: u32, l: vec3<i32>) -> u32 {
    return d_at(slot, l);
}

fn s_occupied(slot: u32, level: u32, c: vec3<i32>) -> bool {
    // Cells of the 2^level cube lie within Chebyshev distance 2^level - 1 of its first cell.
    return d_at(slot, c << vec3<u32>(level)) < (1u << level);
}

fn s_probe(slot: u32, l: vec3<i32>) -> vec2<u32> {
    let k = d_at(slot, l);
    if (k == 0u) { return vec2<u32>(voxel(slot, l), 1u); }
    // The aligned cube of size s around l is empty if s <= k.
    return vec2<u32>(0u, min(1u << firstLeadingBit(k), 16u));
}

fn s_block(slot: u32, l: vec3<i32>) -> u32 {
    return voxel(slot, l);
}
