// Visibility `raster` (gpu-raster back end): depth-tested rasterisation of the geometry.
// Each fragment intersects its pixel's ray with the face plane (`face_hit`) and shades it;
// with `depth_prepass` (lit mode) depth is laid down first and only the front face shades.

@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    let d = pixel_ray(in.pos.xy);
    return shade_surface(face_surface(face_hit(in.info.y, in.info.x, d), in.info.x, d), d, fr.mode == 1u, vec2<u32>(in.pos.xy));
}
