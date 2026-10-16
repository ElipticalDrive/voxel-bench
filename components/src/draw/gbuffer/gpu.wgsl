// Visibility `gbuffer` (gpu-raster, a drawer choice of the discovery back ends; was gpu-hybrid;
// keeps no instance, so the face source's diagnostic views do not apply): the raster pass writes
// the common hit record (`pack_hit`: distance, block and face, 8 bytes per pixel) instead of an
// id; the deferred shading pass (`shading::deferred_wgsl`) rebuilds the surface from it and shades
// once per pixel. A fatter buffer than `visbuffer` (8 bytes per pixel against 4), but no
// refetch of the face.

@fragment
fn fs(in: VOut) -> @location(0) vec2<u32> {
    let d = pixel_ray(in.pos.xy);
    let h = fragment_hit(in, d);
    return pack_hit(h.t, h.block, in.info.x);
}
