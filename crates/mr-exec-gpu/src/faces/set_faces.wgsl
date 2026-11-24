// The GPU face set's face source for the face drawer (`visibility/faces.wgsl`): one
// instanced quad per face or merged run of the list built on the GPU. The drawer's
// visibility component shades (`shade_surface`); lit-mode shadow rays walk their storage.

@group(0) @binding(8) var<storage, read> faces: array<vec4<u32>>; // key, table index, block, fresh | run length << 1 (merged runs; 0 = one face)
@group(0) @binding(9) var<storage, read> chunk_pos: array<vec4<i32>>;

// Record: key (face | chunk-local cell << 3 | chunk slot << 18), table index (or seam
// edges, merged runs), block, fresh | run length << 1 (merged runs; 0 = one face).
fn rec_cell(e: vec4<u32>) -> vec3<f32> {
    let l = (e.x >> 3u) & 32767u;
    return vec3<f32>(f32(l & 31u), f32((l >> 5u) & 31u), f32((l >> 10u) & 31u));
}

fn rec_chunk(e: vec4<u32>) -> vec3<f32> {
    return vec3<f32>(chunk_pos[e.x >> 18u].xyz * 32 - vp.cam_block);
}

fn src_vertex(ii: u32, vi: u32) -> FaceVertex {
    let e = faces[ii];
    let face = e.x & 7u;
    let ax = face_axes(face);
    let c = vec2<f32>(f32(vi & 1u), f32(vi >> 1u));
    // A merged run spans its length in cells along u (entries of unmerged lists: 1).
    let run = e.w >> 1u;
    var p = face_corner(face_origin(rec_cell(e), face), face, vec2<f32>(f32(max(1u, run)), 1.0), c);
    if (run != 0u) {
        // Merged quads meet their neighbours in T-junctions, where pixel centres exactly on
        // the seam can fall between both quads. Grow seam edges by 1/16 px at their
        // distance (well above the rasteriser's 1/256-px snapping). The overlap is
        // coplanar; the depth test and the per-cell block lookup in `face_hit` resolve it.
        let base = rec_chunk(e) + p - fr.cam_frac;
        let z = max(dot(base, fr.forward), 1e-3);
        let eps = z * 2.0 * fr.tan_half_fov / frame_size().y / 16.0;
        // Only seam edges (e.y bits: 0 -u, 1 +u, 2 -v, 3 +v); silhouettes stay exact.
        let grow_u = select(e.y & 1u, (e.y >> 1u) & 1u, c.x > 0.5);
        let grow_v = select((e.y >> 2u) & 1u, (e.y >> 3u) & 1u, c.y > 0.5);
        p[ax.y] += (c.x * 2.0 - 1.0) * eps * f32(grow_u);
        p[ax.z] += (c.y * 2.0 - 1.0) * eps * f32(grow_v);
    }
    return FaceVertex(rec_chunk(e) + p, face, select(e.z, LOOKUP_BLOCK, run != 0u));
}

fn src_origin(ii: u32, face: u32) -> vec3<f32> {
    let e = faces[ii];
    return rec_chunk(e) + face_origin(rec_cell(e), face);
}

fn src_block(ii: u32) -> u32 {
    let e = faces[ii];
    return select(e.z, LOOKUP_BLOCK, (e.w >> 1u) != 0u);
}

// Merged run: the block of the cell actually under the hit, so seams and the
// T-junction overlap texture exactly like a ray caster would.
fn src_lookup(ii: u32, face: u32, h: Hit, d: vec3<f32>) -> u32 {
    let b = s_block_at(h.cell + vp.cam_block);
    return select(faces[ii].z, b, b != 0u);
}

// Face-id raster (incremental mode): which list entry covers each pixel.
@fragment
fn fs_id(in: VOut) -> @location(0) u32 {
    return in.info.y + 1u;
}

// The post's category views of the drawn list, rasterised on their own and copied into its
// quantity buffer. `churn`: faces new to the set this frame (category 1), carried over (2).
// `quads`: one id per drawn instance (a face, or a merged run of faces).
@fragment
fn fs_churn(in: VOut) -> @location(0) u32 {
    return select(2u, 1u, (faces[in.info.y].w & 1u) != 0u);
}

@fragment
fn fs_quads(in: VOut) -> @location(0) u32 {
    return (in.info.y + 1u) << 8u | 1u;
}

#ifdef VIEW_OVERDRAW
// The post's `overdraw` view: every fragment passing the depth test (forced early where the
// device can) counts in its quantity buffer.
#ifdef EARLY_DEPTH
@fragment @early_depth_test(force)
#else
@fragment
#endif
fn fs_overdraw(in: VOut) {
    atomicAdd(&quantity[u32(in.pos.y) * fr.width + u32(in.pos.x)], 1u);
}
#endif
