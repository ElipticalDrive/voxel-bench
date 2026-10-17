// Visibility `visbuffer` (gpu-raster; a drawer choice of gpu-discover and cpu-discover): the raster pass writes only an id
// (instance << 3 | face, plus one; 0 = sky) and depth: an encoding of the hit record (4 bytes
// per pixel against the packed record's 8) that the face source decodes (`face_hit`) in the
// common deferred pass (`shading/deferred.wgsl`), which shades once per pixel, independent
// of overdraw. The source's diagnostic views apply (`record_view`).

@group(3) @binding(0) var vis_tex: texture_2d<u32>;
@group(3) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;

@fragment
fn fs(in: VOut) -> @location(0) u32 {
    return ((in.info.y << 3u) | in.info.x) + 1u;
}

fn record_at(px: vec2<u32>) -> u32 { return textureLoad(vis_tex, vec2<i32>(px), 0).x; }
fn record_hit(id: u32) -> bool { return id != 0u; }
fn record_decode(id: u32, d: vec3<f32>) -> Surface {
    let face = (id - 1u) & 7u;
    return face_surface(face_hit((id - 1u) >> 3u, face, d), face, d);
}
fn record_view(id: u32, color: vec4<f32>) -> vec4<f32> { return src_view((id - 1u) >> 3u, color); }
