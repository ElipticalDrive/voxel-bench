// Visibility `visbuffer` (gpu-raster back end): the raster pass writes only an id
// (instance << 3 | face, plus one; 0 = sky) and depth. A compute pass then reads the id,
// reconstructs the hit (`face_hit`) and shades once per pixel, independent of overdraw.

@group(3) @binding(0) var vis_tex: texture_2d<u32>;
@group(3) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;

@fragment
fn fs(in: VOut) -> @location(0) u32 {
    return ((in.info.y << 3u) | in.info.x) + 1u;
}

@compute @workgroup_size(8, 8)
fn shade(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(out_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }
    let id = textureLoad(vis_tex, vec2<i32>(gid.xy), 0).x;
    if (id == 0u) {
        textureStore(out_tex, vec2<i32>(gid.xy), fr.sky);
        return;
    }
    let d = pixel_ray(vec2<f32>(gid.xy) + 0.5);
    let h = face_hit((id - 1u) >> 3u, (id - 1u) & 7u, d);
    textureStore(out_tex, vec2<i32>(gid.xy), shade_hit(h, fr.cam_frac, d, fr.mode == 1u));
}
