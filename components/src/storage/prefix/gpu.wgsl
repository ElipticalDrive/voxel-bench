// Storage `prefix` (IDEAS §1): per slot in `aux`, 512 Morton-ordered 4^3 occupancy masks
// (u64 as lo, hi u32 pairs; bits Morton-ordered too, so each byte is a 2^3 cube), then the
// popcount prefix (513 u16, packed in pairs). An aligned cube of n bricks is a contiguous
// Morton range [a, a + n), empty exactly when prefix[a] == prefix[a + n]. Block ids are
// read from the shared pool. Same interface as `store_bricks.wgsl`.

const S_LEVELS: u32 = 0x1fu;

fn spread_bits(v: u32) -> u32 {
    return (v & 1u) | ((v & 2u) << 2u) | ((v & 4u) << 4u);
}

// Morton index of up to 3 bits per axis, x lowest (matches `prefix::morton3`).
fn morton_xyz(c: vec3<u32>) -> u32 {
    return spread_bits(c.x) | (spread_bits(c.y) << 1u) | (spread_bits(c.z) << 2u);
}

fn p_mask(slot: u32, m: u32) -> vec2<u32> {
    let b = slot * AUX_WORDS + 2u * m;
    return vec2<u32>(aux[b], aux[b + 1u]);
}

fn p_prefix(slot: u32, i: u32) -> u32 {
    let w = aux[slot * AUX_WORDS + 1024u + (i >> 1u)];
    return (w >> ((i & 1u) * 16u)) & 0xffffu;
}

// Bits of mask `w` at Morton position `k` (k a multiple of 8 for bytes), masked by `m`.
fn p_bits(w: vec2<u32>, k: u32, m: u32) -> u32 {
    return select(w.y >> (k - 32u), w.x >> k, k < 32u) & m;
}

// Whether the aligned cube of `n` bricks (8 or 64) containing brick `m` is empty.
fn p_empty(slot: u32, m: u32, n: u32) -> bool {
    let a = m & ~(n - 1u);
    return p_prefix(slot, a) == p_prefix(slot, a + n);
}

fn s_occupied(slot: u32, level: u32, c: vec3<i32>) -> bool {
    let l = vec3<u32>(c << vec3<u32>(level));
    let m = morton_xyz(l >> vec3<u32>(2u));
    switch level {
        case 0u: { return p_bits(p_mask(slot, m), morton_xyz(l & vec3<u32>(3u)), 1u) != 0u; }
        case 1u: { return p_bits(p_mask(slot, m), morton_xyz(l & vec3<u32>(3u)), 0xffu) != 0u; }
        case 2u: { let w = p_mask(slot, m); return (w.x | w.y) != 0u; }
        case 3u: { return !p_empty(slot, m, 8u); }
        default: { return !p_empty(slot, m, 64u); }
    }
}

// Bottom-up: the cell, then the largest empty aligned cube containing it.
fn s_probe(slot: u32, li: vec3<i32>) -> vec2<u32> {
    let l = vec3<u32>(li);
    let m = morton_xyz(l >> vec3<u32>(2u));
    let k = morton_xyz(l & vec3<u32>(3u));
    let w = p_mask(slot, m);
    if (p_bits(w, k, 1u) != 0u) { return vec2<u32>(voxel(slot, li), 1u); }
    if (p_bits(w, k & ~7u, 0xffu) != 0u) { return vec2<u32>(0u, 1u); }
    if ((w.x | w.y) != 0u) { return vec2<u32>(0u, 2u); }
    if (!p_empty(slot, m, 8u)) { return vec2<u32>(0u, 4u); }
    if (!p_empty(slot, m, 64u)) { return vec2<u32>(0u, 8u); }
    return vec2<u32>(0u, 16u);
}

fn s_block(slot: u32, l: vec3<i32>) -> u32 {
    return voxel(slot, l);
}
