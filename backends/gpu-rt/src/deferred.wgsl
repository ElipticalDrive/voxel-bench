// `placement = deferred`: the ray queries write each pixel's hit record (`pack_hit`) into
// `records`, then the common pass (`shading/deferred.wgsl`, entry `shade`) shades it.

@group(0) @binding(7) var<storage, read_write> records: array<vec2<u32>>;

@compute @workgroup_size(8, 8)
fn walk_records(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width || gid.y >= cam.height) { return; }
    let d = view_ray(gid.xy);
    let h = ctrace(cam.origin, d, vp.max_dist);
    var r = vec2<u32>(0u);
    if (h.hit) { r = pack_hit(h.t, surface_of(h, cam.origin, d)); }
    records[gid.y * cam.width + gid.x] = r;
}

fn record_at(px: vec2<u32>) -> vec2<u32> { return records[px.y * cam.width + px.x]; }
fn view_origin() -> vec3<f32> { return cam.origin; }
fn view_lit() -> bool { return cam.mode == 1u; }
fn view_surface(s: Surface) -> Surface { return s; }
fn view_miss(px: vec2<u32>) -> vec4<f32> { return unpack4x8unorm(cam.sky); }
