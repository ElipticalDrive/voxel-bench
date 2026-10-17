// `placement = deferred` (shader def `DEFERRED`): the march writes each pixel's hit record (`pack_hit`) into
// `records`, then the common pass (`shading/deferred.wgsl`, entry `shade`) shades it.
// `view_surface` gives the decoded surface the march's own details back from the record's
// cell: texture coordinates relative to the column's top block, and on lit tops the
// heightmap slope. Not exactly inline's surface: the march covers rows whose ray meets the
// top plane outside the marched column (texture coordinates clamped there), a cell the
// record's parity cannot name; the record keeps the cell the point lies in (texels differ
// at 8-19% of pixels, PSNR against the reference within 0.1 dB of inline).

@group(0) @binding(6) var<storage, read_write> records: array<vec2<u32>>;

fn record_at(px: vec2<u32>) -> vec2<u32> { return records[px.y * cam.width + px.x]; }
fn view_ray(px: vec2<u32>) -> vec3<f32> { return column_ray(f32(px.x) + 0.5, f32(px.y) + 0.5); }
fn view_origin() -> vec3<f32> { return cam.origin; }
fn view_lit() -> bool { return cam.mode == 1u; }
fn view_miss(px: vec2<u32>) -> vec4<f32> { return unpack4x8unorm(cam.sky); }
fn view_surface(s: Surface) -> Surface {
    let cell = vec2<i32>(round(s.p - s.local).xz);
    var r = s;
    r.local = clamp(s.p - vec3<f32>(f32(cell.x), hm_top(hm_at(cell)) - 1.0, f32(cell.y)), vec3<f32>(0.0), vec3<f32>(0.999));
    if (view_lit() && s.face == 2u) { r.n = slope_normal(cell); }
    return r;
}
