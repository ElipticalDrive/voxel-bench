// Storage `hier`: the shared pool and 4^3 brick bits (as `bricks`) plus, per slot in `aux`,
// a 64-bit summary with one bit per 8^3 region (as cpu-simd-hier). Same interface as
// `store_bricks.wgsl`.

const S_LEVELS: u32 = 0xdu;

// Whether the 8^3 region containing local cell `l` holds a solid voxel.
fn h_region(slot: u32, l: vec3<i32>) -> bool {
    let r = vec3<u32>(l >> vec3<u32>(3u));
    let i = r.x | (r.z << 2u) | (r.y << 4u);
    return ((aux[slot * AUX_WORDS + (i >> 5u)] >> (i & 31u)) & 1u) != 0u;
}

fn s_occupied(slot: u32, level: u32, c: vec3<i32>) -> bool {
    if (level == 0u) { return brick_occupied(slot, c) && voxel(slot, c) != 0u; }
    if (level == 2u) { return brick_occupied(slot, c << vec3<u32>(2u)); }
    return h_region(slot, c << vec3<u32>(3u));
}

fn s_probe(slot: u32, l: vec3<i32>) -> vec2<u32> {
    if (!h_region(slot, l)) { return vec2<u32>(0u, 8u); }
    if (!brick_occupied(slot, l)) { return vec2<u32>(0u, 4u); }
    return vec2<u32>(voxel(slot, l), 1u);
}

fn s_block(slot: u32, l: vec3<i32>) -> u32 {
    return voxel(slot, l);
}
