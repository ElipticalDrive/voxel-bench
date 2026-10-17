// The ray-casting executor's root (PLAN step 13e2), after `view.wgsl`: one thread per pixel
// casts the primary ray (`pixel_hit`, the lane's: a walk over a storage, or a hardware ray
// query) and colours the hit with the shading component (`shade_surface`), in a compute
// thread or (`FRAGMENT`) a fragment shader over a full-screen triangle.

// Mesh mode (`MESHES`): the distance along `d` to the mesh surface at pixel `px`, where its
// ray stops (1e30: none).
fn pixel_mesh_t(px: vec2<u32>, d: vec3<f32>) -> f32 {
#ifdef MESHES
    return mesh_t(px, d, cam.forward);
#else
    return 1e30;
#endif
}

// The colour of pixel `px` (or, `VIEW_STEPS`, its primary ray's loop iterations), shaded
// inline.
fn pixel_color(px: vec2<u32>) -> vec4<f32> {
    let d = view_ray(px);
#ifdef VIEW_STEPS
    ct_iters = 0u;
#endif
    let mt = pixel_mesh_t(px, d);
    let h = pixel_hit(px, d, mt);
#ifdef VIEW_STEPS
    // On the shared colour scale (log; white above the top).
    return ramp_value(f32(ct_iters), f32(cam.view_max), true);
#else
    var color = unpack4x8unorm(cam.sky);
#ifdef MESHES
    // No voxel in front of the mesh: its record's surface.
    if (!h.hit && mt < 1e29) {
        let m = mesh_record(px);
        color = shade_mesh(mesh_surface(record_mesh_tri(m), cam.origin, d, bitcast<f32>(m.x)), cam.mode == 1u);
    }
#endif
    if (h.hit) { color = shade_surface(surface_of(h, cam.origin, d), d, cam.mode == 1u, px); }
    return color;
#endif
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width || gid.y >= cam.height) { return; }
    textureStore(out_tex, vec2<i32>(gid.xy), pixel_color(gid.xy));
}

#ifdef FRAGMENT
// `dispatch = "fragment"` (was gpu-frag-dda): the same per pixel, as a fragment shader over
// a full-screen triangle (render pipeline instead of compute).
@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_full(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    // frag.xy is the pixel centre (x + 0.5, y + 0.5), y down: the same pixel as gid.
    return pixel_color(vec2<u32>(frag.xy));
}
#endif
