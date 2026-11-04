// Top level of region (and DAG) storages: a hash table from region coordinates to the
// region's handle (blob word offset, root node, or UNIFORM_BIT | block); its mask and
// longest probe are in `region_head` (see `gpu_data::StorageData::upload_table`). S_TOP
// comes from the storage.

fn s_top(key: vec3<i32>) -> u32 {
    var i = hash_cell(key) & region_head.x;
    for (var n = 0u; n <= region_head.y; n++) {
        let e = regions[i];
        if (e.w == EMPTY) { return EMPTY; }
        if (all(bitcast<vec3<i32>>(e.xyz) == key)) { return e.w; }
        i = (i + 1u) & region_head.x;
    }
    return EMPTY;
}
