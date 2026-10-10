// Mesh mode (mr_core::mesh): camera-relative triangles with sRGB vertex colours, drawn into
// the voxel renderer's depth buffer. Lit mode uses the shared lighting (voxel shadow rays).

struct MeshFrame {
    view_proj: mat4x4<f32>,
    cam_frac: vec3<f32>, mode: u32,   // camera position within vp.cam_block; 0 unlit, 1 lit
}

@group(0) @binding(0) var<uniform> mf: MeshFrame;

struct MeshIn {
    @location(0) pos: vec3<f32>,       // relative to vp.cam_block
    @location(1) color: vec4<f32>,     // sRGB
    @location(2) normal: vec3<f32>,    // geometric triangle normal (either side)
}

struct MeshOut {
    @builtin(position) @invariant pos: vec4<f32>,
    @location(0) rel: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) normal: vec3<f32>,
}

@vertex
fn mesh_vs(v: MeshIn) -> MeshOut {
    var out: MeshOut;
    out.pos = mf.view_proj * vec4<f32>(v.pos - mf.cam_frac, 1.0);
    out.rel = v.pos;
    out.color = v.color;
    out.normal = v.normal;
    return out;
}

@fragment
fn mesh_fs(in: MeshOut) -> @location(0) vec4<f32> {
    // Interpolated sRGB colour (perspective-correct, like the reference's barycentrics).
    let c = vec4<f32>(in.color.rgb, 1.0);
    if (mf.mode == 0u) { return c; }
    // Meshes need not be closed or consistently wound: light the side facing the camera.
    let n = select(in.normal, -in.normal, dot(in.normal, in.rel - mf.cam_frac) > 0.0);
    return shade_lit(c, in.rel, n);
}
