// Mesh surfaces (PLAN step 12d): a mesh triangle's hit, decoded from its record (the
// record's mesh capability, `pack_mesh_hit`) wherever the pixel is shaded: the ray
// caster's walk (inline) or the common deferred pass. The triangles are the mesh pass's
// vertices (`mr_gpu::meshes::GpuMeshes`, camera-block relative, three per triangle), bound
// by the including program as `mesh_verts` (`surface_wgsl(group, binding)`).

// A vertex is 7 words: position (3 f32), sRGB colour (4 x u8), the triangle's normal (3 f32).
const MESH_VERT_WORDS: u32 = 7u;

// What shading reads from a mesh hit: the camera-block-relative point, the normal facing
// the ray, the interpolated sRGB colour.
struct MeshSurface { p: vec3<f32>, n: vec3<f32>, color: vec4<f32> }

fn mesh_pos(v: u32) -> vec3<f32> {
    let w = v * MESH_VERT_WORDS;
    return vec3<f32>(bitcast<f32>(mesh_verts[w]), bitcast<f32>(mesh_verts[w + 1u]), bitcast<f32>(mesh_verts[w + 2u]));
}

// Triangle `tri` hit by ray (`o`, `d`), at `t` as recorded. The barycentrics are the ray's
// (equal to the rasteriser's perspective-correct ones at the pixel centre); the point is the
// ray's on the triangle's plane, or where the ray runs along it at `t` (when known, > 0;
// else at the distance of the triangle's first vertex).
fn mesh_surface(tri: u32, o: vec3<f32>, d: vec3<f32>, t: f32) -> MeshSurface {
    let v0 = tri * 3u;
    let p0 = mesh_pos(v0);
    let e1 = mesh_pos(v0 + 1u) - p0;
    let e2 = mesh_pos(v0 + 2u) - p0;
    let pv = cross(d, e2);
    let det = dot(e1, pv);
    var b = vec2<f32>(1.0 / 3.0);
    var th = select(length(p0 - o), t, t > 0.0);
    if (abs(det) > 1e-12) {
        let tv = o - p0;
        let qv = cross(tv, e1);
        b = clamp(vec2<f32>(dot(tv, pv), dot(d, qv)) / det, vec2<f32>(0.0), vec2<f32>(1.0));
        th = dot(e2, qv) / det;
    }
    let w = v0 * MESH_VERT_WORDS;
    let c = unpack4x8unorm(mesh_verts[w + 3u]) * (1.0 - b.x - b.y) + unpack4x8unorm(mesh_verts[w + MESH_VERT_WORDS + 3u]) * b.x + unpack4x8unorm(mesh_verts[w + 2u * MESH_VERT_WORDS + 3u]) * b.y;
    let n = vec3<f32>(bitcast<f32>(mesh_verts[w + 4u]), bitcast<f32>(mesh_verts[w + 5u]), bitcast<f32>(mesh_verts[w + 6u]));
    // Meshes need not be closed or consistently wound: the side facing the camera.
    return MeshSurface(o + d * th, select(n, -n, dot(n, d) > 0.0), vec4<f32>(c.rgb, 1.0));
}

// A mesh surface shaded: its colour unlit, the lighting model's direct light lit (with the
// mesh-shadow site's rays, through the lighting module this one imports).
fn shade_mesh(m: MeshSurface, lit: bool) -> vec4<f32> {
    if (!lit) { return m.color; }
    return shade_lit(m.color, m.p, m.n);
}
