// Visibility `gbuffer` (gpu-raster, a drawer choice of the discovery back ends; was gpu-hybrid;
// keeps no instance, so the face source's diagnostic views do not apply): the raster pass writes the
// surface itself (camera-relative hit point and face; block id + 1, 0 = sky) instead of
// an id; a compute pass rebuilds the `Hit` from it and shades once per pixel. A fatter
// buffer than `visbuffer` (20 bytes per pixel against 4), but no refetch of the face.

@group(3) @binding(0) var g_block: texture_2d<u32>;
@group(3) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;
@group(3) @binding(2) var g_pos: texture_2d<f32>;

struct GOut { @location(0) pos: vec4<f32>, @location(1) block: u32 }

@fragment
fn fs(in: VOut) -> GOut {
    let d = pixel_ray(in.pos.xy);
    let h = fragment_hit(in, d);
    var o: GOut;
    o.pos = vec4<f32>(fr.cam_frac + d * h.t, f32(in.info.x));
    o.block = h.block + 1u;
    return o;
}

@compute @workgroup_size(8, 8)
fn shade(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(out_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }
    let id = textureLoad(g_block, vec2<i32>(gid.xy), 0).x;
    if (id == 0u) {
        textureStore(out_tex, vec2<i32>(gid.xy), fr.sky);
        return;
    }
    let g = textureLoad(g_pos, vec2<i32>(gid.xy), 0);
    let face = u32(g.w);
    let a = face / 2u;
    let d = pixel_ray(vec2<f32>(gid.xy) + 0.5);
    var h: Hit;
    h.hit = true;
    h.t = dot(g.xyz - fr.cam_frac, d);
    h.axis = a;
    h.block = id - 1u;
    // The cell behind the face, as `face_hit` computes it.
    h.cell = vec3<i32>(floor(g.xyz));
    h.cell[a] = i32(round(g.xyz[a])) - select(0, 1, (face & 1u) == 0u);
    textureStore(out_tex, vec2<i32>(gid.xy), shade_surface(face_surface(h, face, d), d, fr.mode == 1u, gid.xy));
}
