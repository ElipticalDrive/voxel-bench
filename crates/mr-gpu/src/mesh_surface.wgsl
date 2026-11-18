// Mesh surfaces: a mesh triangle's hit, decoded from its record (the
// record's mesh capability, `pack_mesh_hit`) wherever the pixel is shaded: the ray
// caster's walk (inline) or the common deferred pass; the shading component shades it
// (`shade_mesh`, `shading::mesh`). The triangles are the meshes' two-level buffer
// (`mr_gpu::meshes::layout`), bound by the including program as `mesh_verts`
// (`surface_wgsl(group, binding)`).

// The buffer: a header of MESH_HEAD words (where the BLAS nodes, the instance table, the
// TLAS nodes and the TLAS items start, and the instance count), the shapes' vertices (local
// space, three per triangle, each shape's in its BLAS's leaf order), the BLAS nodes, the
// instance table, the TLAS. A vertex is 7 words: position (3 f32), sRGB colour (4 x u8), the
// triangle's normal (3 f32). An instance is MESH_INST_WORDS words: local -> camera-block-
// relative (three rows of a 3x4 matrix), its inverse likewise, its shape's BLAS root, first
// vertex, first global triangle (a record's triangle id = that + the index in the shape).
const MESH_HEAD: u32 = 8u;
const MESH_VERT_WORDS: u32 = 7u;
const MESH_INST_WORDS: u32 = 32u;
const MESH_INST_INVERSE: u32 = 12u;
const MESH_INST_ROOT: u32 = 24u;
const MESH_INST_FIRST_VERTEX: u32 = 25u;
const MESH_INST_BASE: u32 = 26u;

// What shading reads from a mesh hit: the camera-block-relative point, the normal facing
// the ray, the interpolated sRGB colour.
struct MeshSurface { p: vec3<f32>, n: vec3<f32>, color: vec4<f32> }

fn mesh_word_f(w: u32) -> f32 {
    return bitcast<f32>(mesh_verts[w]);
}

fn mesh_vec(w: u32) -> vec3<f32> {
    return vec3<f32>(mesh_word_f(w), mesh_word_f(w + 1u), mesh_word_f(w + 2u));
}

// The local position of the shapes' vertex `v`.
fn mesh_pos(v: u32) -> vec3<f32> {
    return mesh_vec(MESH_HEAD + v * MESH_VERT_WORDS);
}

// The 3x4 matrix of rows at word `w` applied to (`p`, `one`).
fn mesh_apply(w: u32, p: vec3<f32>, one: f32) -> vec3<f32> {
    let q = vec4<f32>(p, one);
    let r0 = vec4<f32>(mesh_vec(w), mesh_word_f(w + 3u));
    let r1 = vec4<f32>(mesh_vec(w + 4u), mesh_word_f(w + 7u));
    let r2 = vec4<f32>(mesh_vec(w + 8u), mesh_word_f(w + 11u));
    return vec3<f32>(dot(r0, q), dot(r1, q), dot(r2, q));
}

// The table word of the instance holding global triangle `tri` (a binary search over the
// instances' first triangles, ascending in the table).
fn mesh_instance(tri: u32) -> u32 {
    var lo = 0u;
    var hi = mesh_verts[2];
    while (hi - lo > 1u) {
        let mid = (lo + hi) / 2u;
        if (mesh_verts[mesh_verts[1] + MESH_INST_WORDS * mid + MESH_INST_BASE] <= tri) { lo = mid; } else { hi = mid; }
    }
    return mesh_verts[1] + MESH_INST_WORDS * lo;
}

// Triangle `tri` hit by ray (`o`, `d`), at `t` as recorded. The barycentrics are the ray's
// (equal to the rasteriser's perspective-correct ones at the pixel centre); the point is the
// ray's on the triangle's plane, or where the ray runs along it at `t` (when known, > 0;
// else at the distance of the triangle's first vertex).
fn mesh_surface(tri: u32, o: vec3<f32>, d: vec3<f32>, t: f32) -> MeshSurface {
    let iw = mesh_instance(tri);
    let v0 = mesh_verts[iw + MESH_INST_FIRST_VERTEX] + 3u * (tri - mesh_verts[iw + MESH_INST_BASE]);
    let p0 = mesh_apply(iw, mesh_pos(v0), 1.0);
    let e1 = mesh_apply(iw, mesh_pos(v0 + 1u), 1.0) - p0;
    let e2 = mesh_apply(iw, mesh_pos(v0 + 2u), 1.0) - p0;
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
    // Rigid motion and uniform scale: the matrix carries the normal.
    let n = normalize(mesh_apply(iw, mesh_vec(w + 4u), 0.0));
    // Meshes need not be closed or consistently wound: the side facing the camera.
    return MeshSurface(o + d * th, select(n, -n, dot(n, d) > 0.0), vec4<f32>(c.rgb, 1.0));
}

// Rays against the triangles (the lighting model's shadow rays and bounces) through the two
// levels: the TLAS over the instances' boxes, then at each instance the ray moved into the
// shape's local space (its direction not normalised, so t is shared) through the shape's
// BLAS. BVH node words (both levels): its box (lo, hi: 6 f32), then a leaf's first item and
// count, or an inner node's two children (the second in bits 0-28, the split axis in bits
// 29-30, bit 31 set; the first child on the axis' lower side, walked first when the ray runs
// up the axis). TLAS leaves name items (instance indices), BLAS leaves triangles of their
// shape.

const MESH_NONE: u32 = 0xffffffffu;
// Hits nearer than this are the ray's own surface (shadow rays start 1e-3 off it).
const MESH_TMIN: f32 = 1e-4;

struct MeshTrace { t: f32, tri: u32 }

// Möller–Trumbore, two-sided: the ray's t at the shapes' triangle with first vertex `v`, or -1.
fn mesh_tri_t(v: u32, o: vec3<f32>, d: vec3<f32>) -> f32 {
    let p0 = mesh_pos(v);
    let e1 = mesh_pos(v + 1u) - p0;
    let e2 = mesh_pos(v + 2u) - p0;
    let pv = cross(d, e2);
    let det = dot(e1, pv);
    if (abs(det) < 1e-12) { return -1.0; }
    let inv = 1.0 / det;
    let tv = o - p0;
    let u = dot(tv, pv) * inv;
    if (u < 0.0 || u > 1.0) { return -1.0; }
    let qv = cross(tv, e1);
    let v2 = dot(d, qv) * inv;
    if (v2 < 0.0 || u + v2 > 1.0) { return -1.0; }
    return dot(e2, qv) * inv;
}

fn mesh_inv(d: vec3<f32>) -> vec3<f32> {
    return 1.0 / select(d, vec3<f32>(1e-20), abs(d) < vec3<f32>(1e-20));
}

// Whether ray (`o`, 1 / `inv`) enters node word `w`'s box before `max_t`.
fn mesh_enters(w: u32, o: vec3<f32>, inv: vec3<f32>, max_t: f32) -> bool {
    let t0 = (mesh_vec(w) - o) * inv;
    let t1 = (mesh_vec(w + 3u) - o) * inv;
    let tn = min(t0, t1);
    let tf = max(t0, t1);
    return max(max(tn.x, tn.y), max(tn.z, 0.0)) <= min(min(tf.x, tf.y), min(tf.z, max_t));
}

// Stack entries besides node indices (< 2^29): an instance to enter (its index), and the
// return from an instance's BLAS to the TLAS.
const MESH_ENTER: u32 = 0x80000000u;
const MESH_LEAVE: u32 = 0x40000000u;

// The nearest triangle with MESH_TMIN < t < max_t on ray (`o`, unit `d`), or with `any`
// the first found; `tri` = MESH_NONE when there is none. One loop and one stack for both
// levels: a TLAS leaf pushes its instances (MESH_ENTER); entering one moves the ray into
// its local space and pushes MESH_LEAVE under its BLAS root; popping MESH_LEAVE (after
// every node of that BLAS) moves the ray back.
fn mesh_walk(o: vec3<f32>, d: vec3<f32>, max_t: f32, any: bool) -> MeshTrace {
    var best = MeshTrace(max_t, MESH_NONE);
    if (mesh_verts[2] == 0u) { return best; }
    let winv = mesh_inv(d);
    // The current level: the TLAS (world ray) or an instance's BLAS (local ray).
    var ro = o;
    var rd = d;
    var inv = winv;
    var nodes = mesh_verts[3];
    var local = false;
    var first = 0u;
    var base = 0u;
    var stack: array<u32, 48>;
    var n = 1u;
    stack[0] = 0u;
    while (n > 0u) {
        n -= 1u;
        let k = stack[n];
        if (k == MESH_LEAVE) {
            ro = o;
            rd = d;
            inv = winv;
            nodes = mesh_verts[3];
            local = false;
            continue;
        }
        if ((k & MESH_ENTER) != 0u) {
            let iw = mesh_verts[1] + MESH_INST_WORDS * (k & ~MESH_ENTER);
            ro = mesh_apply(iw + MESH_INST_INVERSE, o, 1.0);
            rd = mesh_apply(iw + MESH_INST_INVERSE, d, 0.0);
            inv = mesh_inv(rd);
            nodes = mesh_verts[0];
            local = true;
            first = mesh_verts[iw + MESH_INST_FIRST_VERTEX];
            base = mesh_verts[iw + MESH_INST_BASE];
            stack[n] = MESH_LEAVE;
            stack[n + 1u] = mesh_verts[iw + MESH_INST_ROOT];
            n += 2u;
            continue;
        }
        let w = nodes + 8u * k;
        if (!mesh_enters(w, ro, inv, best.t)) { continue; }
        let a = mesh_verts[w + 6u];
        let b = mesh_verts[w + 7u];
        if ((b & 0x80000000u) != 0u) {
            if (n < 46u) {
                let up = rd[(b >> 29u) & 3u] >= 0.0;
                stack[n] = select(a, b & 0x1fffffffu, up);
                stack[n + 1u] = select(b & 0x1fffffffu, a, up);
                n += 2u;
            }
            continue;
        }
        if (local) {
            for (var i = a; i < a + b; i++) {
                let t = mesh_tri_t(first + 3u * i, ro, rd);
                if (t > MESH_TMIN && t < best.t) {
                    best = MeshTrace(t, base + i);
                    if (any) { return best; }
                }
            }
        } else {
            // Room kept for entering the last pushed: MESH_LEAVE and its root.
            for (var i = a; i < a + b && n < 46u; i++) {
                stack[n] = MESH_ENTER | mesh_verts[mesh_verts[4] + i];
                n += 1u;
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
