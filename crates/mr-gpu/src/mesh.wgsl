// Mesh mode (mr_core::mesh): instanced shapes (local triangles with sRGB vertex colours,
// moved by each instance's matrix to camera-relative), drawn into the voxel renderer's depth
// buffer. Fragments either write the hit record's mesh
// capability (`mesh_record_fs`, `mesh_id_fs`: shaded later, where the pixel is shaded) or,
// with shader def `SHADE` (forward drawers), shade themselves through the shading
// component (`shade_mesh`, `shading::mesh`). The root provides `mesh_frame()`, the camera
// as a `MeshFrame`: the record passes' own uniform, or a forward drawer's frame.

struct MeshFrame {
    view_proj: mat4x4<f32>,
    cam_frac: vec3<f32>, mode: u32,   // camera position within vp.cam_block; 0 unlit, 1 lit
}

struct MeshIn {
    @location(0) pos: vec3<f32>,       // in the shape's local space
    @location(1) color: vec4<f32>,     // sRGB
    @location(2) normal: vec3<f32>,    // geometric triangle normal (either side), local
}

// Per instance (the meshes' instance table as an instance-rate vertex buffer): local ->
// camera-block-relative as three rows of a 3x4 matrix, the shape's first vertex and the
// instance's first global triangle.
struct MeshPlace {
    @location(3) r0: vec4<f32>,
    @location(4) r1: vec4<f32>,
    @location(5) r2: vec4<f32>,
    @location(6) first: vec2<u32>,
}

struct MeshOut {
    @builtin(position) @invariant pos: vec4<f32>,
    @location(0) rel: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) normal: vec3<f32>,
    // The global triangle (a record's id).
    @location(3) @interpolate(flat) tri: u32,
}

@vertex
fn mesh_vs(v: MeshIn, i: MeshPlace, @builtin(vertex_index) vi: u32) -> MeshOut {
    let mf = mesh_frame();
    let q = vec4<f32>(v.pos, 1.0);
    let rel = vec3<f32>(dot(i.r0, q), dot(i.r1, q), dot(i.r2, q));
    var out: MeshOut;
    out.pos = mf.view_proj * vec4<f32>(rel - mf.cam_frac, 1.0);
    out.rel = rel;
    out.color = v.color;
    // Rigid motion and uniform scale: the matrix carries the normal.
    out.normal = normalize(vec3<f32>(dot(i.r0.xyz, v.normal), dot(i.r1.xyz, v.normal), dot(i.r2.xyz, v.normal)));
    // Vertices are three per triangle, unindexed; ids are global (instance base + local).
    out.tri = i.first.y + (vi - i.first.x) / 3u;
    return out;
}

// The packed hit record (ray casters' mesh targets, `gbuffer`).
@fragment
fn mesh_record_fs(in: MeshOut) -> @location(0) vec2<u32> {
    return pack_mesh_hit(length(in.rel - mesh_frame().cam_frac), in.tri);
}

// `visbuffer`'s face-id encoding.
@fragment
fn mesh_id_fs(in: MeshOut) -> @location(0) u32 {
    return VIS_MESH | in.tri;
}

#ifdef SHADE
@fragment
fn mesh_fs(in: MeshOut) -> @location(0) vec4<f32> {
    let mf = mesh_frame();
    // Interpolated sRGB colour (perspective-correct, like the reference's barycentrics).
    let c = vec4<f32>(in.color.rgb, 1.0);
    let to = in.rel - mf.cam_frac;
    // Meshes need not be closed or consistently wound: light the side facing the camera.
    let n = select(in.normal, -in.normal, dot(in.normal, to) > 0.0);
    return shade_mesh(MeshSurface(in.rel, n, c), normalize(to), mf.mode == 1u, vec2<u32>(in.pos.xy));
}
#endif
