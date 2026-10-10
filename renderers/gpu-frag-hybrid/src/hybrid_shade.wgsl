// gpu-hybrid deferred lighting: one compute thread per pixel reads the rasterised
// G-buffer and DDA-traces shadow rays through the voxel world from the surface point.

@group(0) @binding(0) var g_albedo: texture_2d<f32>;
@group(0) @binding(1) var g_pos: texture_2d<f32>;
@group(0) @binding(2) var out_tex: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let size = textureDimensions(out_tex);
    if (gid.x >= size.x || gid.y >= size.y) { return; }
    let px = vec2<i32>(gid.xy);
    let albedo = textureLoad(g_albedo, px, 0);
    let pos = textureLoad(g_pos, px, 0);
    var color = albedo;   // sky pixels keep the clear colour
    if (pos.w >= 0.0) {
        color = shade_lit(albedo, pos.xyz, face_normal(u32(pos.w)));
    }
    textureStore(out_tex, px, color);
}
