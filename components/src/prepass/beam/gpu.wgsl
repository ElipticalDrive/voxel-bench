// Prepass `beam` (from gpu-beam): one conservative cone per PREPASS_TILE x PREPASS_TILE
// pixel tile. The tile's centre ray marches in steps of one top cell (2^S_TOP); at each
// step every top cell overlapping a box that covers the whole cone slab is asked for
// (`s_top`). Rays of the tile then start where the cone is provably empty. Exact.

fn beam_ray(px: vec2<f32>) -> vec3<f32> {
    let sx = (2.0 * px.x / f32(cam.width) - 1.0) * cam.tan_half_fov * cam.aspect;
    let sy = (1.0 - 2.0 * px.y / f32(cam.height)) * cam.tan_half_fov;
    return normalize(cam.forward + sx * cam.right + sy * cam.up);
}

fn beam_cell_solid(c: vec3<i32>) -> bool {
    let v = s_top(c);
    if (v == EMPTY) { return false; }
    if ((v & UNIFORM_BIT) != 0u && (v & 0xffffu) == 0u) { return false; }
    return true;
}

fn beam_tiles_x() -> u32 {
    return (cam.width + PREPASS_TILE - 1u) / PREPASS_TILE;
}

@compute @workgroup_size(8, 8)
fn prepass(@builtin(global_invocation_id) gid: vec3<u32>) {
    let tiles_y = (cam.height + PREPASS_TILE - 1u) / PREPASS_TILE;
    if (gid.x >= beam_tiles_x() || gid.y >= tiles_y) { return; }
    let p0 = vec2<f32>(gid.xy * PREPASS_TILE);
    let p1 = min(p0 + vec2<f32>(f32(PREPASS_TILE)), vec2<f32>(f32(cam.width), f32(cam.height)));
    let n = beam_ray(0.5 * (p0 + p1));
    // Max tangent of the angle between the centre ray and any ray of the tile (corners).
    var k = 0.0;
    let cs = array<vec2<f32>, 4>(p0, vec2<f32>(p1.x, p0.y), vec2<f32>(p0.x, p1.y), p1);
    for (var i = 0u; i < 4u; i++) {
        let c = clamp(dot(beam_ray(cs[i]), n), 1e-3, 1.0);
        k = max(k, sqrt(max(1.0 - c * c, 0.0)) / c);
    }
    k = k * 1.01 + 1e-4;
    let o = cam.origin;
    let top = f32(1u << S_TOP);
    let half = 0.5 * top;
    var safe = 0.0;
    for (var i = 0u; i < 64u; i++) {
        let tc = half + top * f32(i);            // slab [tc - half, tc + half] in projected depth
        if (tc - half > vp.max_dist) { safe = vp.max_dist; break; }
        let h = half + k * (tc + half) + 1.0;    // half extent covering the slab's cross-section
        if (h > 1.5 * top) { break; }
        let p = o + n * tc;
        let a = (vec3<i32>(floor(p - vec3<f32>(h))) + vp.cam_block) >> vec3<u32>(S_TOP);
        let b = (vec3<i32>(floor(p + vec3<f32>(h))) + vp.cam_block) >> vec3<u32>(S_TOP);
        var hit = false;
        for (var x = a.x; x <= b.x && !hit; x++) {
            for (var y = a.y; y <= b.y && !hit; y++) {
                for (var z = a.z; z <= b.z && !hit; z++) {
                    hit = beam_cell_solid(vec3<i32>(x, y, z));
                }
            }
        }
        if (hit) { break; }
        safe = tc + half;
    }
    // Margin for float error at the start cell.
    let skip = max(safe - 2.0, 0.0);
    beam[gid.y * beam_tiles_x() + gid.x] = skip;
    atomicAdd(&beam_stats[0], u32(skip));
    atomicAdd(&beam_stats[1], 1u);
}

fn start_t(px: vec2<u32>) -> f32 {
    let tc = px / PREPASS_TILE;
    return beam[tc.y * beam_tiles_x() + tc.x];
}
