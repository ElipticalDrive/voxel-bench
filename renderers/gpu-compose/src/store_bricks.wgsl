// Storage `bricks`: the shared pool (a u16 id per voxel) with one bit per 4^3 brick.
// Interface (every storage provides it):
//   S_LEVELS            levels k (bit k) answered directly by s_occupied: aligned 2^k cubes
//   s_occupied(slot, level, c)  whether the cube at chunk-local level coords c is occupied
//   s_probe(slot, l)    (block id or 0, largest empty aligned cube size) at local cell l
//   s_block(slot, l)    block id at local cell l

const S_LEVELS: u32 = 0x5u;

fn s_occupied(slot: u32, level: u32, c: vec3<i32>) -> bool {
    if (level == 0u) { return brick_occupied(slot, c) && voxel(slot, c) != 0u; }
    return brick_occupied(slot, c << vec3<u32>(2u));
}

fn s_probe(slot: u32, l: vec3<i32>) -> vec2<u32> {
    if (!brick_occupied(slot, l)) { return vec2<u32>(0u, 4u); }
    return vec2<u32>(voxel(slot, l), 1u);
}

fn s_block(slot: u32, l: vec3<i32>) -> u32 {
    return voxel(slot, l);
}
