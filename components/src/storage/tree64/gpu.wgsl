// Storage `tree64` (from gpu-tree64): a sparse 64-tree per 64^3 region (see mod.rs for the
// blob layout). The handle is the region blob's word offset in `aux`; queries descend from
// the root at the handle. Levels: voxels (0), 4^3 nodes (2), 16^3 nodes (4).

const S_TOP: u32 = 6u;
const S_LEVELS: u32 = 0x15u;

fn t64_bit(c: vec3<i32>) -> u32 {
    return u32(c.x | (c.z << 2u) | (c.y << 4u));
}

// Node `at`'s child for child bit `b`: (exists, rank among the node's children).
fn t64_child(n: vec4<u32>, b: u32) -> vec2<u32> {
    let below = (1u << (b & 31u)) - 1u;
    if (b < 32u) { return vec2<u32>((n.x >> b) & 1u, countOneBits(n.x & below)); }
    return vec2<u32>((n.y >> (b - 32u)) & 1u, countOneBits(n.x) + countOneBits(n.y & below));
}

// Leaf id `idx` (u16 index from the blob start `h`).
fn t64_leaf(h: u32, idx: u32) -> u32 {
    return (aux[h + (idx >> 1u)] >> ((idx & 1u) * 16u)) & 0xffffu;
}

// Nodes are one 16-byte load each: (mask lo, mask hi, child pointer, uniform block).
fn s_probe(h: u32, l: vec3<i32>) -> vec2<u32> {
    var n = aux_v4[h >> 2u];
    var shift = 6u;
    loop {
        if (n.w != 0u) { return vec2<u32>(n.w, 1u); }
        shift -= 2u;
        let c = t64_child(n, t64_bit((l >> vec3<u32>(shift)) & vec3<i32>(3)));
        if (c.x == 0u) { return vec2<u32>(0u, 1u << shift); }
        if (shift == 0u) { return vec2<u32>(t64_leaf(h, n.z + c.y), 1u); }
        n = aux_v4[(h + n.z) / 4u + c.y];
    }
    return vec2<u32>(0u, 1u);
}

// Whether the level-`level` (0, 2 or 4) node at level coords `c` exists (is non-empty).
fn s_occupied(h: u32, level: u32, c: vec3<i32>) -> bool {
    let l = c << vec3<u32>(level);
    var n = aux_v4[h >> 2u];
    var shift = 6u;
    loop {
        if (n.w != 0u || shift == level) { return true; }
        shift -= 2u;
        let ch = t64_child(n, t64_bit((l >> vec3<u32>(shift)) & vec3<i32>(3)));
        if (ch.x == 0u) { return false; }
        if (shift == 0u) { return true; }
        n = aux_v4[(h + n.z) / 4u + ch.y];
    }
    return false;
}

fn s_block(h: u32, l: vec3<i32>) -> u32 {
    return s_probe(h, l).x;
}
