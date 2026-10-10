// Generic fallbacks for the optional storage queries (IDEAS §9: every storage answers
// every query, slowly where it has no fast path). The assembler appends the ones the
// chosen storage does not provide.

//@s_dist
// Chebyshev distance lower bound from `s_probe`: 0 if solid, else 1 + the distance from l
// to the boundary of the empty aligned cube it reports.
fn s_dist(slot: u32, l: vec3<i32>) -> u32 {
    let p = s_probe(slot, l);
    if (p.x != 0u) { return 0u; }
    let s = i32(p.y);
    let lo = l - (l & vec3<i32>(s - 1));
    let m = min(l - lo, lo + vec3<i32>(s - 1) - l);
    return u32(min(min(m.x, m.y), m.z)) + 1u;
}

//@s_row
// Occupancy of the 32 cells from x0 along x, from 32 voxel-level queries.
fn s_row(slot: u32, x0: i32, y: i32, z: i32) -> u32 {
    var r = 0u;
    for (var x = 0; x < 32; x++) {
        if (s_occupied(slot, 0u, vec3<i32>(x0 + x, y, z))) { r |= 1u << u32(x); }
    }
    return r;
}
