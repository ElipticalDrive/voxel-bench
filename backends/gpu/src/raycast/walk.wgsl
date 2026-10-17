// The walking lane (gpu-compute): the primary ray is a walk (`ctrace` from the traversal
// kernel over the storage component), starting where the prepass lets it.

// Prepass output (one start distance per tile) and statistics; see `prepass` components.
@group(0) @binding(8) var<storage, read_write> beam: array<f32>;
@group(0) @binding(9) var<storage, read_write> beam_stats: array<atomic<u32>, 2>;

// What the primary ray `d` through pixel `px` hits (mesh mode: rays end at the mesh
// surface, `mt`; a voxel in front of it still wins).
fn pixel_hit(px: vec2<u32>, d: vec3<f32>, mt: f32) -> Hit {
    let o = cam.origin;
    // The prepass may let the ray start further out (its tile's cone is empty up to
    // there); the walk then starts at that point, and the hit's t is shifted back.
    let t0 = start_t(px);
    var h: Hit;
    if (t0 > 0.0) {
        h = ctrace(o + d * t0, d, min(vp.max_dist, mt) - t0);
        h.t += t0;
    } else {
        h = ctrace(o, d, min(vp.max_dist, mt));
    }
    return h;
}
