// Top level of pool storages: 32^3 chunks in the shared voxel pool; the handle is the
// pool slot (or EMPTY / UNIFORM_BIT | block) from the voxel world's chunk table. Pool
// storages hold LOD chunks too: `s_top_lod` finds the chunk of level `lod` (cells 2^lod
// blocks wide) at key = abs >> (S_TOP + lod).

const S_TOP: u32 = 5u;
const S_HAS_LOD: bool = true;

fn s_top_lod(key: vec3<i32>, lod: u32) -> u32 {
    return lookup_chunk(chunk_key(key, lod));
}

fn s_top(key: vec3<i32>) -> u32 {
    return lookup_chunk(chunk_key(key, 0u));
}
