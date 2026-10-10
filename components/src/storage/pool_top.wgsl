// Top level of pool storages: 32^3 chunks in the shared voxel pool; the handle is the
// pool slot (or EMPTY / UNIFORM_BIT | block) from the voxel world's chunk table.

const S_TOP: u32 = 5u;

fn s_top(key: vec3<i32>) -> u32 {
    return lookup_chunk(chunk_key(key, 0u));
}
