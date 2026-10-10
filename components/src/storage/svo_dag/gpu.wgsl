// Storage `svo_dag` (from gpu-svo-dag): one global sparse voxel DAG in `aux` (8 child
// references per node; high bit = inline block id, 0 = air). The handle is the chunk's
// root node. Every octree level exists, so `s_occupied` answers levels 0-4.

const S_TOP: u32 = 5u;
const S_LEVELS: u32 = 0x1fu;
const DAG_LEAF: u32 = 0x80000000u;

// Descends from root `h` towards local cell `l` until a leaf or a cell of size
// 2^`stop`: (reference reached, its cell size).
fn dag_descend(h: u32, l: vec3<i32>, stop: u32) -> vec2<u32> {
    var r = h;
    var size = 32u;
    loop {
        if ((r & DAG_LEAF) != 0u || size == (1u << stop)) { return vec2<u32>(r, size); }
        size >>= 1u;
        let c = (l / i32(size)) & vec3<i32>(1);
        r = aux[r * 8u + u32(c.x | (c.y << 1u) | (c.z << 2u))];
    }
    return vec2<u32>(r, size);
}

fn s_occupied(h: u32, level: u32, c: vec3<i32>) -> bool {
    return dag_descend(h, c << vec3<u32>(level), level).x != DAG_LEAF;
}

// (block or 0, empty cell size): the descent of gpu-svo-dag's `query`.
fn s_probe(h: u32, l: vec3<i32>) -> vec2<u32> {
    var r = h;
    var size = 32u;
    loop {
        if ((r & DAG_LEAF) != 0u) {
            let b = r & 0xffffu;
            return vec2<u32>(b, select(1u, size, b == 0u));
        }
        size >>= 1u;
        let c = (l / i32(size)) & vec3<i32>(1);
        r = aux[r * 8u + u32(c.x | (c.y << 1u) | (c.z << 2u))];
    }
    return vec2<u32>(0u, 1u);
}

fn s_block(h: u32, l: vec3<i32>) -> u32 {
    return s_probe(h, l).x;
}
