//! Face-cluster discovery (IDEAS §10, the user's design; PLAN step 5).
//!
//! Instead of judging screen tiles by their corner rays (the quadtree in `discover`),
//! discovery grows over the geometry itself:
//! 1. *Seeds.* A pixel lattice is walked coarse to fine (`seed_step`, halving to 1 px);
//!    every lattice pixel not yet covered casts a ray. Sky, at spacing <= `sky_tile`:
//!    with `sky = corners` a lattice cell is covered as sky when the seeds at all four of
//!    its corners miss; with `sky = sparse` (the user's sparse pass) a single miss covers
//!    its cell, and the lattice shifts every frame so small things show up within a few
//!    frames.
//! 2. *Clusters.* From the hit face, a flood fill crosses shared edges to exposed faces on
//!    the same plane.
//! 3. *Corner rays.* Each face is confirmed by rays to its four corners. A corner is
//!    visible if nothing is hit before reaching it; visibility is a property of the point,
//!    so each lattice point is cast once per frame however many faces (of any plane) share
//!    it. A face with all four corners visible is *confirmed*: added to S, its projection
//!    covers the pixels it spans, and the fill continues through it. A face with an
//!    occluded corner is added to S but neither covers nor continues the fill.
//! 4. Repeat until every pixel is covered or has been sampled by a seed ray.
//!
//! Completeness: an occluder in front of a face projects onto the face's plane as a region
//! containing a square of side > 1 (IDEAS §5), and any such square overlapping a unit face
//! contains one of that face's corners. The occluder's full shadow is the hull of its near
//! and far faces, though, and a sliver of it can overlap a face without covering a corner;
//! so this is not proven complete. `audit` measures it.

use glam::{IVec3, Vec3};
use mr_core::{Camera, Face};
use rayon::prelude::*;

use crate::visibility::discover::{FaceMap, Output, Rays, Settings, State, cast, face_key, finalize};
use crate::visibility::discover::grid::World;

/// How a face of S was found this frame (`view = kinds` colours them).
pub const KIND_CONFIRMED: u8 = 1;
pub const KIND_PARTIAL: u8 = 2;

/// Smallest chunk of a flood-fill wave handed to one thread.
const MIN_LEN: usize = 256;

/// Coverage value of a pixel whose own seed ray missed (sky); other covered pixels are 1.
const SKY_RAY: u8 = 2;

struct Ctx<'a, G: World> {
    grid: &'a G,
    rays: &'a Rays,
    bmin: IVec3,
    bmax: IVec3,
    w: u32,
    h: u32,
}

/// Exposed face (cell solid, neighbour along the normal air)?
#[inline]
fn exposed<G: World>(grid: &G, cell: IVec3, face: u32) -> bool {
    grid.block(cell) != 0 && grid.block(cell + Face::from_index(face as usize).normal()) == 0
}

/// The four corners (absolute lattice points) of a face, in cyclic order.
fn corners(cell: IVec3, face: u32) -> [IVec3; 4] {
    let n = Face::from_index(face as usize).normal();
    let a = (face / 2) as usize;
    let (u, v) = ((a + 1) % 3, (a + 2) % 3);
    let mut base = cell;
    if n[a] > 0 {
        base[a] += 1;
    }
    let mut du = IVec3::ZERO;
    du[u] = 1;
    let mut dv = IVec3::ZERO;
    dv[v] = 1;
    [base, base + du, base + du + dv, base + dv]
}

/// Whether the lattice point `p` (absolute) is visible from the camera.
fn corner_visible<G: World>(cx: &Ctx<G>, p: IVec3) -> bool {
    let rel = (p - cx.rays.cb).as_vec3();
    let d = rel - cx.rays.o;
    let dist = d.length();
    if dist < 1e-4 {
        return true;
    }
    match cx.grid.trace(cx.rays.cb, cx.rays.o, d / dist, cx.bmin, cx.bmax) {
        None => true,
        // A hit before the point (beyond a small tolerance) occludes it.
        Some(h) => h.t >= dist - (1e-3 + 1e-4 * dist),
    }
}

/// Pixel indices whose centres lie inside the projection of a face (appended to `out`);
/// false if a corner lies behind the camera.
fn covered_pixels<G: World>(cx: &Ctx<G>, c: &[IVec3; 4], out: &mut Vec<u32>) -> bool {
    let mut q = [Vec3::ZERO; 4];
    for k in 0..4 {
        match cx.rays.project((c[k] - cx.rays.cb).as_vec3()) {
            Some(p) => q[k] = p,
            None => return false,
        }
    }
    let (lo, hi) = q.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    let x0 = (lo.x - 0.5).ceil().max(0.0) as i32;
    let y0 = (lo.y - 0.5).ceil().max(0.0) as i32;
    let x1 = ((hi.x - 0.5).floor() as i32).min(cx.w as i32 - 1);
    let y1 = ((hi.y - 0.5).floor() as i32).min(cx.h as i32 - 1);
    if x0 > x1 || y0 > y1 {
        return true;
    }
    // Convex quad: a pixel centre is inside if it is on the same side of all four edges.
    let edge = |a: Vec3, b: Vec3, x: f32, y: f32| (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let e = [edge(q[0], q[1], px, py), edge(q[1], q[2], px, py), edge(q[2], q[3], px, py), edge(q[3], q[0], px, py)];
            if e.iter().all(|&v| v >= 0.0) || e.iter().all(|&v| v <= 0.0) {
                out.push(y as u32 * cx.w + x as u32);
            }
        }
    }
    true
}

pub fn discover<G: World>(mut st: State, grid: &G, cam: &Camera, w: u32, h: u32, set: &Settings) -> Output {
    let start = std::time::Instant::now();
    let rays = Rays::new(cam, w, h, set.margin);
    let (bmin, bmax) = grid.bounds_rel(rays.cb);
    let cx = Ctx { grid, rays: &rays, bmin, bmax, w, h };
    st.frame += 1;
    let frame = st.frame;
    let n = (w * h) as usize;
    let mut cov = std::mem::take(&mut st.cover);
    cov.clear();
    cov.resize(n, 0);

    // Faces handled this frame (-> cluster id) and corner visibility cache.
    let mut done: FaceMap = FaceMap::default();
    // Corner visibility by packed lattice point (fast hasher; points share the face-key
    // packing with face 0).
    let mut corner_vis: FaceMap = FaceMap::with_capacity_and_hasher(st.kinds.len() * 2, Default::default());
    done.reserve(st.kinds.len() * 2);
    let mut kinds: Vec<(u64, u8, u32)> = vec![];
    let (mut seed_rays, mut corner_rays, mut clusters, mut confirmed, mut partial, mut waves) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let mut found: Vec<u64> = vec![];
    let (mut seed_ms, mut corner_ms, mut fill_ms) = (0f32, 0f32, 0f32);

    let step0 = set.seed_step.max(1).next_power_of_two();
    // Per-frame lattice shift, so sparse sky sampling covers everything over time.
    let shift = (frame.wrapping_mul(0x9E37_79B9) >> 16) % step0;
    let (ox, oy) = (shift, (shift * 7 + frame) % step0);
    let mut k = step0;
    loop {
        // 1. Seeds: uncovered lattice pixels at spacing k.
        let t0 = std::time::Instant::now();
        let mut seeds = vec![];
        let (sx, sy) = if k > 1 { (ox % k, oy % k) } else { (0, 0) };
        let mut y = sy;
        while y < h {
            let mut x = sx;
            while x < w {
                if cov[(y * w + x) as usize] == 0 {
                    seeds.push((x, y));
                }
                x += k;
            }
            y += k;
        }
        seed_rays += seeds.len() as u64;
        let hits: Vec<Option<u64>> = seeds.par_iter().map(|&(x, y)| cast(grid, &rays, bmin, bmax, x as f32 + 0.5, y as f32 + 0.5).map(|(key, _, _)| key)).collect();
        // Record this level's misses first (at any level a missed seed keeps SKY_RAY), so the
        // four-corner sky rule sees corners cast at this or a coarser spacing.
        for (&(x, y), hit) in seeds.iter().zip(&hits) {
            if hit.is_none() {
                cov[(y * w + x) as usize] = SKY_RAY;
            }
        }
        let miss_at = |cov: &[u8], x: u32, y: u32| x >= w || y >= h || cov[(y * w + x) as usize] == SKY_RAY;
        let mut frontier: Vec<(u64, u32)> = vec![];
        for (&(x, y), hit) in seeds.iter().zip(hits) {
            match hit {
                Some(key) => {
                    found.push(key);
                    cov[(y * w + x) as usize] = 1;
                    if !done.contains_key(&key) {
                        done.insert(key, clusters as u32);
                        frontier.push((key, clusters as u32));
                        clusters += 1;
                    }
                }
                // Sky: once the spacing is small, a miss covers its lattice cell (`sparse`),
                // or only if the seeds at the cell's other three corners miss too.
                None if k <= set.sky_tile && (set.sky_sparse || (miss_at(&cov, x + k, y) && miss_at(&cov, x, y + k) && miss_at(&cov, x + k, y + k))) => {
                    for yy in y..(y + k).min(h) {
                        for xx in x..(x + k).min(w) {
                            let c = &mut cov[(yy * w + xx) as usize];
                            if *c == 0 {
                                *c = 1;
                            }
                        }
                    }
                }
                None => {}
            }
        }
        seed_ms += t0.elapsed().as_secs_f32() * 1e3;

        // 2. Grow clusters: batched multi-source flood fill over coplanar exposed faces.
        while !frontier.is_empty() {
            waves += 1;
            // a. Corners of the frontier not yet known: gathered and deduplicated in
            //    parallel, then cast in parallel.
            let t1 = std::time::Instant::now();
            // Waves are many and mostly small (a plane needs as many waves as its radius in
            // faces): split work only in chunks of MIN_LEN, so small waves stay on one thread.
            let mut need: Vec<u64> = frontier
                .par_iter()
                .with_min_len(MIN_LEN)
                .flat_map_iter(|&(key, _)| {
                    let (cell, face) = crate::visibility::discover::unpack_key(key);
                    corners(cell, face).map(|c| face_key(c, 0))
                })
                .collect();
            if need.len() > 8 * MIN_LEN {
                need.par_sort_unstable();
            } else {
                need.sort_unstable();
            }
            need.dedup();
            need.retain(|c| !corner_vis.contains_key(c));
            corner_rays += need.len() as u64;
            let vis: Vec<bool> = need.par_iter().with_min_len(MIN_LEN / 4).map(|&c| corner_visible(&cx, crate::visibility::discover::unpack_key(c).0)).collect();
            for (c, v) in need.into_iter().zip(vis) {
                corner_vis.insert(c, v as u32);
            }
            let t2 = std::time::Instant::now();
            corner_ms += (t2 - t1).as_secs_f32() * 1e3;
            // b. Classify the frontier in parallel: confirmed faces bring their covered
            //    pixels and exposed coplanar neighbours along.
            let cv = &corner_vis;
            let results: Vec<(u64, u32, bool, Vec<u32>, Vec<u64>)> = frontier
                .par_iter()
                .with_min_len(MIN_LEN)
                .map(|&(key, id)| {
                    let (cell, face) = crate::visibility::discover::unpack_key(key);
                    let c = corners(cell, face);
                    let mut px = vec![];
                    if !c.iter().all(|p| cv.get(&face_key(*p, 0)) == Some(&1)) || !covered_pixels(&cx, &c, &mut px) {
                        return (key, id, false, px, vec![]);
                    }
                    let a = (face / 2) as usize;
                    let mut nbs = vec![];
                    for axis in [(a + 1) % 3, (a + 2) % 3] {
                        for s in [-1, 1] {
                            let mut nb = cell;
                            nb[axis] += s;
                            if exposed(grid, nb, face) {
                                nbs.push(face_key(nb, face));
                            }
                        }
                    }
                    (key, id, true, px, nbs)
                })
                .collect();
            // c. Apply serially: coverage, the face set, and the next frontier.
            let mut next = vec![];
            for (key, id, ok, px, nbs) in results {
                found.push(key);
                if !ok {
                    partial += 1;
                    kinds.push((key, KIND_PARTIAL, id));
                    continue;
                }
                confirmed += 1;
                kinds.push((key, KIND_CONFIRMED, id));
                for p in px {
                    cov[p as usize] = 1;
                }
                for nk in nbs {
                    if !done.contains_key(&nk) {
                        done.insert(nk, id);
                        next.push((nk, id));
                    }
                }
            }
            frontier = next;
            fill_ms += t2.elapsed().as_secs_f32() * 1e3;
        }
        if k == 1 {
            break;
        }
        k /= 2;
    }
    let covered = cov.iter().filter(|&&c| c != 0).count();
    st.cover = cov;
    st.kinds = kinds;
    let t_disc = start.elapsed().as_secs_f32() * 1e3;
    let faces_found = found.len() as u64;
    let tag_map: FaceMap = st.kinds.iter().map(|&(k, kind, id)| (k, (kind as u32) << 30 | (id & 0x3fff_ffff))).collect();
    let (instances, tags) = finalize(&mut st, found, grid, rays.cb, set.keep_frames, Some(&tag_map));
    let mut out = Output {
        state: st,
        instances,
        anchor: rays.cb,
        rays: seed_rays + corner_rays,
        tiles_accepted: confirmed,
        faces_found,
        phase_ms: [t_disc, 0.0, start.elapsed().as_secs_f32() * 1e3 - t_disc],
        quad_ms: [seed_ms, corner_ms, fill_ms],
        ms: start.elapsed().as_secs_f32() * 1e3,
        extra: vec![],
        tags,
    };
    out.extra = vec![
        ("seed_rays".into(), seed_rays as f64),
        ("corner_rays".into(), corner_rays as f64),
        ("clusters".into(), clusters as f64),
        ("fill_waves".into(), waves as f64),
        ("faces_confirmed".into(), confirmed as f64),
        ("faces_partial".into(), partial as f64),
        ("covered_pct".into(), 100.0 * covered as f64 / n.max(1) as f64),
        ("seed_ms".into(), seed_ms as f64),
        ("corner_ms".into(), corner_ms as f64),
        ("fill_ms".into(), fill_ms as f64),
    ];
    out
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use glam::DVec3;
    use mr_core::chunk::cell_index;
    use mr_core::{BlockId, CHUNK_VOLUME, ChunkData, ChunkKey, DenseChunk, WorldUpdate};

    use super::*;
    use crate::visibility::discover::Settings;
    use crate::visibility::discover::grid::{CpuWorld, Snapshot};

    /// One chunk: a floor, a back wall, a pillar in front of the wall, and a floating
    /// block — the pillar-in-front-of-a-wall case of IDEAS §5.
    fn world() -> Arc<dyn Snapshot> {
        let mut d = DenseChunk(Box::new([BlockId::AIR; CHUNK_VOLUME]));
        for i in 0..CHUNK_VOLUME {
            let l = mr_core::chunk::cell_of_index(i);
            let floor = l.y == 0;
            let wall = l.z == 24 && l.y < 20;
            let pillar = (12..14).contains(&l.x) && (16..18).contains(&l.z) && l.y < 14;
            if floor || wall || pillar {
                d.0[i] = BlockId(1);
            }
        }
        d.0[cell_index(IVec3::new(20, 8, 18))] = BlockId(2);
        let mut w = CpuWorld::new("bricks").unwrap();
        w.apply(&[WorldUpdate::ChunkLoad { key: ChunkKey { pos: IVec3::ZERO, lod: 0 }, data: ChunkData::Dense(Arc::new(d)) }]);
        w.snapshot()
    }

    fn settings(seed_step: u32) -> Settings {
        Settings { tile: 32, face_criterion: false, sky_tile: 16, keep_frames: 1, max_footprint: 4096, margin: 0.0, dilate: 0, dilate_below: 2, cluster: true, seed_step, sky_sparse: false }
    }

    /// Every face visible at a pixel centre is in S (one pass, from several viewpoints).
    #[test]
    fn complete_on_occluded_scene() {
        let grid = world();
        let (w, h) = (160, 120);
        for (pos, target) in [([16.0, 6.0, 2.0], [16.0, 6.0, 24.0]), ([3.0, 12.0, 4.0], [18.0, 4.0, 20.0]), ([28.5, 2.5, 9.0], [10.0, 8.0, 22.0])] {
            let cam = Camera::look_at(DVec3::from_array(pos), DVec3::from_array(target), 70.0);
            for step in [16, 4] {
                let out = grid.discover(State::default(), &cam, w, h, &settings(step));
                let (missing, _, _) = grid.audit(&out.state.s, &cam, w, h);
                assert_eq!(missing, 0, "camera at {pos:?}, seed_step {step}: {missing} pixels see faces missing from S");
                // And it must actually save rays over one per pixel.
                assert!(out.rays < (w * h) as u64, "{} rays for {} pixels", out.rays, w * h);
            }
        }
    }
}
