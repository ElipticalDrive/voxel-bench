// Mesh surfaces: a mesh triangle's hit, decoded from its record (the
// record's mesh capability, `pack_mesh_hit`) wherever the pixel is shaded: the ray
// caster's walk (inline) or the common deferred pass; the shading component shades it
// (`shade_mesh`, `shading::mesh`). The triangles are the mesh pass's vertices
// (`mr_gpu::meshes::GpuMeshes`, camera-block relative, three per triangle), bound by the
// including program as `mesh_verts` (`surface_wgsl(group, binding)`).

// The buffer (`GpuMeshes::prepare`, `mesh_words`): a header of MESH_HEAD words (the first
// holds where the BVH starts), the vertices, the BVH. A vertex is 7 words: position (3 f32),
// sRGB colour (4 x u8), the triangle's normal (3 f32).
const MESH_HEAD: u32 = 4u;
const MESH_VERT_WORDS: u32 = 7u;

// What shading reads from a mesh hit: the camera-block-relative point, the normal facing
// the ray, the interpolated sRGB colour.
struct MeshSurface { p: vec3<f32>, n: vec3<f32>, color: vec4<f32> }

fn mesh_pos(v: u32) -> vec3<f32> {
    let w = MESH_HEAD + v * MESH_VERT_WORDS;
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
    let w = MESH_HEAD + v0 * MESH_VERT_WORDS;
    let c = unpack4x8unorm(mesh_verts[w + 3u]) * (1.0 - b.x - b.y) + unpack4x8unorm(mesh_verts[w + MESH_VERT_WORDS + 3u]) * b.x + unpack4x8unorm(mesh_verts[w + 2u * MESH_VERT_WORDS + 3u]) * b.y;
    let n = vec3<f32>(bitcast<f32>(mesh_verts[w + 4u]), bitcast<f32>(mesh_verts[w + 5u]), bitcast<f32>(mesh_verts[w + 6u]));
    // Meshes need not be closed or consistently wound: the side facing the camera.
    return MeshSurface(o + d * th, select(n, -n, dot(n, d) > 0.0), vec4<f32>(c.rgb, 1.0));
}

// Rays against the triangles (the lighting model's shadow rays and bounces) through their
// BVH: node k (the root k = 0) is the 8 words from `mesh_verts[0] + 8k`: its box (lo, hi:
// 6 f32), then a leaf's first triangle and count, or an inner node's two children (the
// second in bits 0-28, the split axis in bits 29-30, bit 31 set; the first child on the
// axis' lower side, walked first when the ray runs up the axis).

const MESH_NONE: u32 = 0xffffffffu;
// Hits nearer than this are the ray's own surface (shadow rays start 1e-3 off it).
const MESH_TMIN: f32 = 1e-4;

struct MeshTrace { t: f32, tri: u32 }

// Möller–Trumbore, two-sided: the ray's t at triangle `tri`, or -1.
fn mesh_tri_t(tri: u32, o: vec3<f32>, d: vec3<f32>) -> f32 {
    let p0 = mesh_pos(tri * 3u);
    let e1 = mesh_pos(tri * 3u + 1u) - p0;
    let e2 = mesh_pos(tri * 3u + 2u) - p0;
    let pv = cross(d, e2);
    let det = dot(e1, pv);
    if (abs(det) < 1e-12) { return -1.0; }
    let inv = 1.0 / det;
    let tv = o - p0;
    let u = dot(tv, pv) * inv;
    if (u < 0.0 || u > 1.0) { return -1.0; }
    let qv = cross(tv, e1);
    let v = dot(d, qv) * inv;
    if (v < 0.0 || u + v > 1.0) { return -1.0; }
    return dot(e2, qv) * inv;
}

fn mesh_word_f(w: u32) -> f32 {
    return bitcast<f32>(mesh_verts[w]);
}

// The nearest triangle with MESH_TMIN < t < max_t on ray (`o`, unit `d`), or with `any`
// the first found; `tri` = MESH_NONE when there is none.
fn mesh_walk(o: vec3<f32>, d: vec3<f32>, max_t: f32, any: bool) -> MeshTrace {
    let nodes = mesh_verts[0];
    var best = MeshTrace(max_t, MESH_NONE);
    let inv = 1.0 / select(d, vec3<f32>(1e-20), abs(d) < vec3<f32>(1e-20));
    var stack: array<u32, 32>;
    var n = 1u;
    stack[0] = 0u;
    while (n > 0u) {
        n -= 1u;
        let w = nodes + 8u * stack[n];
        let lo = vec3<f32>(mesh_word_f(w), mesh_word_f(w + 1u), mesh_word_f(w + 2u));
        let hi = vec3<f32>(mesh_word_f(w + 3u), mesh_word_f(w + 4u), mesh_word_f(w + 5u));
        let t0 = (lo - o) * inv;
        let t1 = (hi - o) * inv;
        let tn = min(t0, t1);
        let tf = max(t0, t1);
        if (max(max(tn.x, tn.y), max(tn.z, 0.0)) > min(min(tf.x, tf.y), min(tf.z, best.t))) { continue; }
        let a = mesh_verts[w + 6u];
        let b = mesh_verts[w + 7u];
        if ((b & 0x80000000u) != 0u) {
            if (n < 31u) {
                let up = d[(b >> 29u) & 3u] >= 0.0;
                stack[n] = select(a, b & 0x1fffffffu, up);
                stack[n + 1u] = select(b & 0x1fffffffu, a, up);
                n += 2u;
            }
            continue;
        }
        for (var i = a; i < a + b; i++) {
            let t = mesh_tri_t(i, o, d);
            if (t > MESH_TMIN && t < best.t) {
                best = MeshTrace(t, i);
                if (any) { return best; }
            }
        }
    }
    return best;
}

fn mesh_occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool {
    return mesh_walk(p, d, max_t, true).tri != MESH_NONE;
}

fn mesh_trace(o: vec3<f32>, d: vec3<f32>, max_t: f32) -> MeshTrace {
    return mesh_walk(o, d, max_t, false);
}
