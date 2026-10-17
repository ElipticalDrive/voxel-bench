// `placement = deferred`: the walk writes each pixel's hit record (`pack_hit`) into `records`
// (in a compute thread, or a fragment shader with `dispatch = "fragment"`), then the common
// pass (`shading/deferred.wgsl`, entry `shade`) shades it. Mesh mode: where no voxel is
// hit, the composited mesh shows (`view_miss`), as inline.

@group(0) @binding(6) var<storage, read_write> records: array<vec2<u32>>;

fn pixel_record(px: vec2<u32>) {
    let d = view_ray(px);
    let h = pixel_hit(px, d, mesh_t(px, d, cam.forward));
    var r = vec2<u32>(0u);
    if (h.hit) {
        r = pack_hit(h.t, surface_of(h, cam.origin, d));
    }
    records[px.y * cam.width + px.x] = r;
}

@compute @workgroup_size(8, 8)
fn walk_records(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width || gid.y >= cam.height) { return; }
    pixel_record(gid.xy);
}

@fragment
fn fs_cast(@builtin(position) frag: vec4<f32>) {
    pixel_record(vec2<u32>(frag.xy));
}

fn record_at(px: vec2<u32>) -> vec2<u32> { return records[px.y * cam.width + px.x]; }
fn view_origin() -> vec3<f32> { return cam.origin; }
fn view_lit() -> bool { return cam.mode == 1u; }
fn view_miss(px: vec2<u32>) -> vec4<f32> {
    if (mesh_t(px, view_ray(px), cam.forward) < 1e29) { return mesh_color(px); }
    return unpack4x8unorm(cam.sky);
}
