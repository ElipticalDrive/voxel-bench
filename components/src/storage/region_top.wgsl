// Top level of region storages: a hash table from region coordinates to the word offset
// of the region's blob in `aux` (or UNIFORM_BIT | block). regions[0] = (mask, max probe);
// slots follow (see `gpu_data::StorageData::upload_table`). S_TOP comes from the storage.

fn s_top(key: vec3<i32>) -> u32 {
    let head = regions[0];
    var i = hash3(key) & head.x;
    for (var n = 0u; n <= head.y; n++) {
        let e = regions[1u + i];
        if (e.w == EMPTY) { return EMPTY; }
        if (all(bitcast<vec3<i32>>(e.xyz) == key)) { return e.w; }
        i = (i + 1u) & head.x;
    }
    return EMPTY;
}
