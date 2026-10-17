// `placement = deferred`: the rays write each pixel's hit record (`pack_hit`) into `records`
// (`records.wgsl`; in a compute thread, or a fragment shader with `FRAGMENT`), then the common
// pass (`shading/deferred.wgsl`, entry `shade`) shades it, here or on another GPU. Mesh mode:
// where no voxel is in front of the mesh, the pixel's record is the mesh's.

fn pixel_record(px: vec2<u32>) {
    let d = view_ray(px);
    let mt = pixel_mesh_t(px, d);
    let h = pixel_hit(px, d, mt);
    var r = vec2<u32>(0u);
    if (h.hit) {
        r = pack_hit(h.t, surface_of(h, cam.origin, d));
    }
#ifdef MESHES
    else if (mt < 1e29) {
        r = mesh_record(px);
    }
#endif
    records[px.y * cam.width + px.x] = r;
}

@compute @workgroup_size(8, 8)
fn walk_records(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width || gid.y >= cam.height) { return; }
    pixel_record(gid.xy);
}

#ifdef FRAGMENT
@fragment
fn fs_cast(@builtin(position) frag: vec4<f32>) {
    pixel_record(vec2<u32>(frag.xy));
}
#endif
