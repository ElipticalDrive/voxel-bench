// Visibility `raster` (raster-gpu; the forward drawer of discover-gpu and discover-cpu):
// depth-tested rasterisation of the face source's faces. Each fragment intersects its
// pixel's ray with the face plane (`fragment_hit`) and shades it; with `depth_prepass` (lit
// mode) depth is laid down first and only the front face shades.

@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    let face = in.info.x;
    let d = pixel_ray(in.pos.xy);
    let s = face_surface(fragment_hit(in, d), face, d);
    // Lit and unlit call the shading separately (a constant flag): with one call and a
    // runtime flag, unlit fragments paid for the lighting code (+4.5% on discover-cpu).
    var color: vec4<f32>;
    if (fr.mode == 1u) {
        color = shade_surface(s, d, true, vec2<u32>(in.pos.xy));
    } else {
        color = shade_surface(s, d, false, vec2<u32>(in.pos.xy));
    }
    return src_view(in.info.y, color);
}
