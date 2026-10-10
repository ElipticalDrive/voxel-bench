// gpu-discover diagnostic overlays, blended over the rendered image by a fullscreen pass.
// `tiles`: level-0 tiles tinted by why they were discovered this frame. `rays`: a dot at
// every lattice point whose ray was cast this frame (the adaptive quadtree's samples).

struct Overlay {
    view: u32,       // 2 tiles, 3 rays
    frame: u32,      // discovery frame shown (0 = none this frame: nothing discovered)
    tile: u32, cols: u32,
    rows: u32, need_off: u32, lat_w: u32, off_x: u32,
    off_y: u32, _p0: u32, _p1: u32, _p2: u32,
}

@group(0) @binding(0) var<uniform> ov: Overlay;
@group(0) @binding(1) var<storage, read> need: array<u32>;
@group(0) @binding(2) var<storage, read> cast_frame: array<u32>;

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<u32>(pos.xy);
    // Nothing discovered this frame (an event-driven static frame): everything skipped.
    if (ov.frame == 0u) { return vec4<f32>(0.0, 0.0, 0.0, 0.45); }
    if (ov.view == 3u) {
        let q = p + vec2<u32>(ov.off_x, ov.off_y);
        if (cast_frame[q.y * ov.lat_w + q.x] == ov.frame) { return vec4<f32>(1.0, 1.0, 0.2, 1.0); }
        return vec4<f32>(0.0, 0.0, 0.0, 0.35);
    }
    let t = (p + vec2<u32>(ov.off_x, ov.off_y)) / ov.tile;
    if (t.x >= ov.cols || t.y >= ov.rows) { return vec4<f32>(0.0); }
    let why = need[ov.need_off + t.y * ov.cols + t.x];
    if (why == 0u) { return vec4<f32>(0.0, 0.0, 0.0, 0.45); }
    // One colour per reason, strongest first: full white, chunk blue, strip cyan,
    // uncovered red, parallax yellow, refresh green.
    var c = vec3<f32>(0.2, 0.9, 0.3);
    if ((why & 16u) != 0u) { c = vec3<f32>(1.0, 0.85, 0.1); }
    if ((why & 8u) != 0u) { c = vec3<f32>(1.0, 0.2, 0.15); }
    if ((why & 32u) != 0u) { c = vec3<f32>(0.1, 0.9, 1.0); }
    if ((why & 4u) != 0u) { c = vec3<f32>(0.25, 0.4, 1.0); }
    if ((why & 1u) != 0u) { c = vec3<f32>(1.0); }
    // Tile borders a little stronger, so the lattice is visible.
    let e = (p + vec2<u32>(ov.off_x, ov.off_y)) % ov.tile;
    let a = select(0.35, 0.6, e.x == 0u || e.y == 0u);
    return vec4<f32>(c, a);
}
