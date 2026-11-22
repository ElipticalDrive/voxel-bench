//! The quadtree sampler on the CPU (discovery's pieces as components; was
//! part of discover-cpu's `discover.rs`): adaptive screen-space tiles of corner rays, see
//! [`crate::visibility::discover`] for the tile rules and the shared state.

use std::time::Instant;

use glam::{IVec3, Vec3};
use mr_core::{Camera, Face};
use rayon::prelude::*;

use crate::visibility::discover::grid::{World, ffloor};
use crate::visibility::discover::{MISS, Output, Rays, Sample, Settings, State, Verdict, cast, face_key, finalize, unpack_key};

pub(crate) fn quadtree<G: World>(mut st: State, grid: &G, cam: &Camera, w: u32, h: u32, set: &Settings) -> Output {
    let start = Instant::now();
    let rays = Rays::new(cam, w, h, set.margin);
    let (bmin, bmax) = grid.bounds_rel(rays.cb);
    let (lw, lh) = (w + 1, h + 1);
    let n = (lw * lh) as usize;
    if st.samples.len() != n {
        st.samples = vec![Sample::default(); n];
        st.stamp = vec![0; n];
        st.generation = 0;
    }
    st.generation = st.generation.wrapping_add(1);
    if st.generation == 0 {
        st.stamp.fill(0);
        st.generation = 1;
    }
    st.frame += 1;
    let generation = st.generation;
    let frame = st.frame;

    let t = set.tile.max(1);
    // Jitter: the tile grid shifted by a per-frame offset, so a thing small enough to slip
    // between one frame's corner rays is hit on another (the faces stay `keep_frames`).
    let (ox, oy) = if set.jitter {
        let mut z = (frame as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        ((z as u32) % t, ((z >> 32) as u32) % t)
    } else {
        (0, 0)
    };
    let mut tiles: Vec<[u32; 4]> = vec![];
    for y in (0..h + oy).step_by(t as usize) {
        for x in (0..w + ox).step_by(t as usize) {
            let (x0, y0) = (x.saturating_sub(ox), y.saturating_sub(oy));
            let (x1, y1) = ((x + t - ox).min(w), (y + t - oy).min(h));
            if x1 > x0 && y1 > y0 {
                tiles.push([x0, y0, x1, y1]);
            }
        }
    }
    let mut rays_cast = 0u64;
    let mut accepted = 0u64;
    let mut found: Vec<u64> = vec![];
    let mut need: Vec<u32> = vec![];
    let mut level_size = t;
    let (mut gather_ms, mut trace_ms, mut classify_ms) = (0f32, 0f32, 0f32);
    while !tiles.is_empty() {
        // 1. Trace the corners not traced yet this frame.
        let tg = Instant::now();
        need.clear();
        for tl in &tiles {
            for (x, y) in [(tl[0], tl[1]), (tl[2], tl[1]), (tl[0], tl[3]), (tl[2], tl[3])] {
                let i = (y * lw + x) as usize;
                if st.stamp[i] != generation {
                    st.stamp[i] = generation;
                    need.push(i as u32);
                }
            }
        }
        rays_cast += need.len() as u64;
        let dilate = if level_size <= set.dilate_below { set.dilate } else { 0 };
        let tt = Instant::now();
        gather_ms += (tt - tg).as_secs_f32() * 1e3;
        let traced: Vec<Sample> = need
            .par_iter()
            .map(|&i| {
                let (x, y) = (i % lw, i / lw);
                match cast(grid, &rays, bmin, bmax, x as f32 + 0.5, y as f32 + 0.5) {
                    Some((key, plane, p)) => Sample { key, plane, p },
                    None => Sample { key: MISS, plane: MISS, p: Vec3::ZERO },
                }
            })
            .collect();
        if dilate > 0 {
            // One buffer per worker, not one per ray.
            let extra = traced
                .par_iter()
                .filter(|s| s.key != MISS)
                .fold(Vec::new, |mut v, s| {
                    dilate_faces(grid, s.key, rays.cb, rays.o, dilate, &mut v);
                    v
                })
                .reduce(Vec::new, |mut a, mut b| {
                    a.append(&mut b);
                    a
                });
            found.extend(extra);
        }
        for (&i, s) in need.iter().zip(traced) {
            st.samples[i as usize] = s;
            if s.key != MISS {
                found.push(s.key);
            }
        }
        level_size = (level_size / 2).max(1);
        let tc = Instant::now();
        trace_ms += (tc - tt).as_secs_f32() * 1e3;
        // 2. Classify tiles.
        let samples = &st.samples;
        let (faces, kids, acc) = tiles
            .par_iter()
            .fold(
                || (Vec::new(), Vec::new(), 0u64),
                |(mut faces, mut kids, mut acc), tl| {
                    let c = [(tl[0], tl[1]), (tl[2], tl[1]), (tl[0], tl[3]), (tl[2], tl[3])].map(|(x, y)| samples[(y * lw + x) as usize]);
                    let (ew, eh) = (tl[2] - tl[0], tl[3] - tl[1]);
                    let verdict = if ew <= 1 && eh <= 1 {
                        Verdict::Done
                    } else if c.iter().all(|s| s.key == MISS) {
                        if ew.max(eh) <= set.sky_tile { Verdict::Done } else { Verdict::Split }
                    } else if c.iter().any(|s| s.key == MISS) {
                        Verdict::Split
                    } else if set.face_criterion {
                        if c.iter().all(|s| s.key == c[0].key) { Verdict::Done } else { Verdict::Split }
                    } else if c.iter().all(|s| s.plane == c[0].plane) {
                        if footprint(grid, rays.cb, &c, set.max_footprint, set.strict, &mut faces) { Verdict::Done } else { Verdict::Split }
                    } else {
                        Verdict::Split
                    };
                    match verdict {
                        Verdict::Done => acc += 1,
                        Verdict::Split => {
                            let xs = if ew > 1 { vec![(tl[0], (tl[0] + tl[2]) / 2), ((tl[0] + tl[2]) / 2, tl[2])] } else { vec![(tl[0], tl[2])] };
                            let ys = if eh > 1 { vec![(tl[1], (tl[1] + tl[3]) / 2), ((tl[1] + tl[3]) / 2, tl[3])] } else { vec![(tl[1], tl[3])] };
                            for &(y0, y1) in &ys {
                                for &(x0, x1) in &xs {
                                    kids.push([x0, y0, x1, y1]);
                                }
                            }
                        }
                    }
                    (faces, kids, acc)
                },
            )
            .reduce(
                || (Vec::new(), Vec::new(), 0u64),
                |mut a, mut b| {
                    a.0.append(&mut b.0);
                    a.1.append(&mut b.1);
                    (a.0, a.1, a.2 + b.2)
                },
            );
        found.extend(faces);
        accepted += acc;
        tiles = kids;
        classify_ms += tc.elapsed().as_secs_f32() * 1e3;
    }

    let t_quad = start.elapsed().as_secs_f32() * 1e3;
    let faces_found = found.len() as u64;
    let records = finalize(&mut st, found, grid, rays.cb, set.keep_frames, None);
    Output { state: st, records, rays: rays_cast, tiles_accepted: accepted, faces_found, quad_ms: [gather_ms, trace_ms, classify_ms], phase_ms: [t_quad, 0.0, start.elapsed().as_secs_f32() * 1e3 - t_quad], ms: start.elapsed().as_secs_f32() * 1e3, extra: vec![] }
}

/// Enumerates exposed faces on the common plane of the four corner samples inside the
/// bounding box of their hit points. Returns false (tile must split) if it is too large or,
/// `strict`, if a cell of it is not an exposed face of the plane (a hole or something in
/// front of the plane, seen between the corners).
fn footprint<G: World>(grid: &G, cb: IVec3, c: &[Sample; 4], max_cells: u32, strict: bool, out: &mut Vec<u64>) -> bool {
    let face = (c[0].plane & 7) as u32;
    let a = (face / 2) as usize;
    let coord = (c[0].plane >> 3) as u32 as i32;
    let (a1, a2) = ((a + 1) % 3, (a + 2) % 3);
    let (mut lo1, mut hi1, mut lo2, mut hi2) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for s in c {
        lo1 = lo1.min(s.p[a1]);
        hi1 = hi1.max(s.p[a1]);
        lo2 = lo2.min(s.p[a2]);
        hi2 = hi2.max(s.p[a2]);
    }
    let e = 1e-3;
    let (l1, h1, l2, h2) = (ffloor(lo1 - e), ffloor(hi1 + e), ffloor(lo2 - e), ffloor(hi2 + e));
    let cells = (h1 - l1 + 1) as i64 * (h2 - l2 + 1) as i64;
    if cells > max_cells as i64 {
        return false;
    }
    let n = Face::from_index(face as usize).normal();
    let mut cell = IVec3::ZERO;
    cell[a] = coord;
    let start = out.len();
    for v in l2..=h2 {
        cell[a2] = v;
        for u in l1..=h1 {
            cell[a1] = u;
            let abs = cell + cb;
            if grid.block(abs) != 0 && grid.block(abs + n) == 0 {
                out.push(face_key(abs, face));
            } else if strict {
                out.truncate(start);
                return false;
            }
        }
    }
    true
}

/// Adds the camera-facing exposed faces of the hit block (level 1: closes the cracks where a
/// pixel's ray grazes a side face of a block whose front face was found), of its 6
/// neighbours too (2) or of its 3x3x3 neighbourhood (3): sub-pixel faces that the next
/// frame's slightly different rays land on.
fn dilate_faces<G: World>(grid: &G, key: u64, cb: IVec3, o: Vec3, level: u32, out: &mut Vec<u64>) {
    let (cell, _) = unpack_key(key);
    let mut visit = |c: IVec3| {
        if grid.block(c) == 0 {
            return;
        }
        let rel = (c - cb).as_vec3() + 0.5 - o;
        for a in 0..3 {
            // Face on the camera side along axis a.
            let face = if rel[a] < 0.0 { 2 * a } else { 2 * a + 1 } as u32;
            if grid.block(c + Face::from_index(face as usize).normal()) == 0 {
                out.push(face_key(c, face));
            }
        }
    };
    if level >= 3 {
        for z in -1..=1 {
            for y in -1..=1 {
                for x in -1..=1 {
                    visit(cell + IVec3::new(x, y, z));
                }
            }
        }
    } else {
        visit(cell);
        if level == 2 {
            for f in Face::ALL {
                visit(cell + f.normal());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use glam::DVec3;
    use mr_core::chunk::cell_index;
    use mr_core::{BlockId, CHUNK_VOLUME, ChunkData, ChunkKey, DenseChunk, WorldUpdate};

    use super::*;
    use crate::visibility::discover::grid::{CpuWorld, Snapshot};

    /// One chunk: a floor, a wall with a 3x3 window, and a back wall seen through it.
    fn world() -> Arc<dyn Snapshot> {
        let mut d = DenseChunk(Box::new([BlockId::AIR; CHUNK_VOLUME]));
        for i in 0..CHUNK_VOLUME {
            let l = mr_core::chunk::cell_of_index(i);
            let window = (14..17).contains(&l.x) && (8..11).contains(&l.y);
            if l.y == 0 || (l.z == 24 && l.y < 20 && !window) || l.z == 30 {
                d.0[i] = BlockId(1);
            }
        }
        d.0[cell_index(IVec3::new(15, 9, 29))] = BlockId(2);
        let mut w = CpuWorld::new("bricks").unwrap();
        w.apply(&[WorldUpdate::ChunkLoad { key: ChunkKey { pos: IVec3::ZERO, lod: 0 }, data: ChunkData::Dense(Arc::new(d)) }]);
        w.snapshot()
    }

    fn settings(strict: bool) -> Settings {
        Settings { tile: 32, face_criterion: false, sky_tile: 8, keep_frames: 1, max_footprint: 4096, strict, jitter: false, margin: 0.0, dilate: 0, dilate_below: 2, cluster: false, seed_step: 16, sky_sparse: false }
    }

    /// A plane whose footprint has a hole (a window) is not coherent: strict tiles split and
    /// find what the window shows; without `strict` the tiles on the wall miss it.
    #[test]
    fn strict_footprints_see_through_windows() {
        let grid = world();
        let (w, h) = (160, 120);
        let cam = Camera::look_at(DVec3::new(16.0, 10.0, 2.0), DVec3::new(15.5, 9.5, 24.0), 70.0);
        let missing = |strict| {
            let out = grid.discover(State::default(), &cam, w, h, &settings(strict));
            grid.audit(&out.state.s, &cam, w, h).0
        };
        assert_eq!(missing(true), 0, "strict: pixels see faces missing from S");
        assert!(missing(false) > 0, "the scene must exercise the window case");
    }
}
