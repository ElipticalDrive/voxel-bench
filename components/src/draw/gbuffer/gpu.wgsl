// Visibility `gbuffer` (gpu-raster, a drawer choice of the discovery back ends; was gpu-hybrid;
// keeps no instance, so the face source's diagnostic views do not apply): the raster pass writes
// the common hit record (`pack_hit`: distance, block, face and cell parity, 8 bytes per pixel) instead of an
// id; the deferred shading pass (`shading::deferred_wgsl`) rebuilds the surface from it and shades
// once per pixel. A fatter buffer than `visbuffer` (8 bytes per pixel against 4), but no
// refetch of the face.

@group(3) @binding(0) var rec_tex: texture_2d<u32>;
@group(3) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;

fn record_at(px: vec2<u32>) -> vec2<u32> { return textureLoad(rec_tex, vec2<i32>(px), 0).xy; }

@fragment
fn fs(in: VOut) -> @location(0) vec2<u32> {
    let d = pixel_ray(in.pos.xy);
    let h = fragment_hit(in, d);
    return pack_hit(h.t, face_surface(h, in.info.x, d));
}
