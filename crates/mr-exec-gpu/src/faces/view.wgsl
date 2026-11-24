// The GPU sampler's writers of the post's category views into its quantity buffer
// (`quantity`, declared with the view), one thread per pixel; a pixel's category k is
// k + 1 (0 leaves the image). `tiles`: level-0 tiles by why they were discovered this frame.
// `rays`: the lattice points whose ray was cast this frame (the adaptive quadtree's samples).
// `copy_ids`: the drawn list's category raster (`fs_churn`, `fs_quads`).

struct View {
    frame: u32,      // discovery frame shown (0 = none this frame: nothing discovered)
    tile: u32, cols: u32, rows: u32,
    need_off: u32, lat_w: u32, off_x: u32, off_y: u32,
    width: u32, height: u32, _p1: u32, _p2: u32,
}

@group(0) @binding(0) var<uniform> ov: View;
@group(0) @binding(1) var<storage, read> need: array<u32>;
@group(0) @binding(2) var<storage, read> cast_frame: array<u32>;
@group(0) @binding(3) var ids: texture_2d<u32>;

@compute @workgroup_size(8, 8)
fn tiles(@builtin(global_invocation_id) p: vec3<u32>) {
    if (p.x >= ov.width || p.y >= ov.height) { return; }
    let i = p.y * ov.width + p.x;
    // Nothing discovered this frame (an event-driven static frame): everything skipped.
    if (ov.frame == 0u) { quantity[i] = 7u; return; }
    let t = (p.xy + vec2<u32>(ov.off_x, ov.off_y)) / ov.tile;
    if (t.x >= ov.cols || t.y >= ov.rows) { return; }
    let why = need[ov.need_off + t.y * ov.cols + t.x];
    // The strongest reason: full, edited block / changed chunk, entering strip, uncovered,
    // parallax, refresh; skipped.
    var c = 6u;
    if (why == 0u) { c = 7u; }
    else if ((why & 1u) != 0u) { c = 1u; }
    else if ((why & 4u) != 0u) { c = 2u; }
    else if ((why & 32u) != 0u) { c = 3u; }
    else if ((why & 8u) != 0u) { c = 4u; }
    else if ((why & 16u) != 0u) { c = 5u; }
    quantity[i] = c;
}

@compute @workgroup_size(8, 8)
fn rays(@builtin(global_invocation_id) p: vec3<u32>) {
    if (p.x >= ov.width || p.y >= ov.height) { return; }
    let q = p.xy + vec2<u32>(ov.off_x, ov.off_y);
    let was_cast = ov.frame != 0u && cast_frame[q.y * ov.lat_w + q.x] == ov.frame;
    quantity[p.y * ov.width + p.x] = select(2u, 1u, was_cast);
}

@compute @workgroup_size(8, 8)
fn copy_ids(@builtin(global_invocation_id) p: vec3<u32>) {
    if (p.x >= ov.width || p.y >= ov.height) { return; }
    quantity[p.y * ov.width + p.x] = textureLoad(ids, p.xy, 0).r;
}
