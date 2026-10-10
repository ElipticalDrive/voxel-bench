//! Self-directed visibility discovery: an adaptive screen-space quadtree of CPU rays that
//! tries to find every block face visible from a camera with as few rays as possible.
//!
//! Rays are cast on a lattice of pixel centres (lattice point `(x, y)` = pixel centre
//! `(x + 0.5, y + 0.5)`; the extra column/row at `w`/`h` lies half a pixel outside). A tile
//! spans lattice points `x0..=x1, y0..=y1`; its four corners are traced once each (shared
//! corners are deduplicated through a per-frame stamp array). Per tile:
//! * corners disagree (or mix hit/miss)            -> split into up to 4 children;
//! * all corners miss                              -> sky if the tile is <= `sky_tile`, else split;
//! * `face` criterion: all corners hit one face    -> accept;
//! * `plane` criterion: all corners hit one face plane (same direction and plane
//!   coordinate) -> enumerate every exposed face on that plane inside the bounding box of
//!   the four hit points ("footprint"); if the footprint exceeds `max_footprint` cells,
//!   split instead;
//! * a tile with lattice extent 1x1 is a leaf: all pixel centres it covers are samples.
//!
//! So every pixel centre is either sampled directly or covered by an accepted tile; holes
//! only come from tiles wrongly judged coherent (something small poking out between the
//! corners) — the `audit` counter measures exactly that.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::time::Instant;

use glam::{IVec3, Vec3};
use mr_core::{Camera, Face};
use rayon::prelude::*;

use crate::grid::{Grid, ffloor};

/// Multiplicative hasher for the packed u64 face keys (std SipHash is ~3x slower here).
#[derive(Default)]
pub(crate) struct KeyHasher(u64);

impl Hasher for KeyHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(5) ^ b as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
        }
    }
    fn write_u64(&mut self, k: u64) {
        let h = (k ^ (k >> 29)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        self.0 = h ^ (h >> 32);
    }
}

pub(crate) type FaceMap = HashMap<u64, u32, BuildHasherDefault<KeyHasher>>;

const MISS: u64 = u64::MAX;
const COORD_BITS: u32 = 20;
const COORD_MASK: u64 = (1 << COORD_BITS) - 1;

/// Packs absolute cell + face into a u64 key (20 bits per axis, two's complement).
#[inline]
pub(crate) fn face_key(cell: IVec3, face: u32) -> u64 {
    (cell.x as u64 & COORD_MASK) | (cell.y as u64 & COORD_MASK) << 20 | (cell.z as u64 & COORD_MASK) << 40 | (face as u64) << 60
}

#[inline]
pub(crate) fn unpack_key(k: u64) -> (IVec3, u32) {
    let s = |v: u64| ((v as i64) << (64 - COORD_BITS) >> (64 - COORD_BITS)) as i32;
    (IVec3::new(s(k & COORD_MASK), s(k >> 20 & COORD_MASK), s(k >> 40 & COORD_MASK)), (k >> 60) as u32)
}

#[derive(Clone, Debug)]
pub(crate) struct Settings {
    pub tile: u32,
    pub face_criterion: bool,
    pub sky_tile: u32,
    pub keep_frames: u32,
    pub max_footprint: u32,
    pub margin: f32,
    /// Fine-level samples also add camera-facing exposed faces around the hit cell:
    /// 0 = off, 1 = hit cell + 6 neighbours, 2 = 3x3x3 neighbourhood.
    pub dilate: u32,
    /// Tile size (px) at or below which samples are dilated.
    pub dilate_below: u32,
}

#[derive(Copy, Clone, Default)]
struct Sample {
    key: u64,
    /// Face direction | plane coordinate along its axis (camera-relative), MISS for sky.
    plane: u64,
    /// Hit point relative to the camera block.
    p: Vec3,
}

/// Persistent discovery state, moved into each background job and back.
#[derive(Default)]
pub(crate) struct State {
    /// The discovered face set S: face key -> frame it was last confirmed.
    pub s: FaceMap,
    samples: Vec<Sample>,
    stamp: Vec<u32>,
    generation: u32,
    pub frame: u32,
}

pub(crate) struct Output {
    pub state: State,
    /// Validated face records for the raster pass (see `record`).
    pub instances: Vec<[u32; 2]>,
    /// Absolute block all records are relative to.
    pub anchor: IVec3,
    pub rays: u64,
    pub tiles_accepted: u64,
    pub faces_found: u64,
    /// Phase times (ms): quadtree rays + classification, merge into S, validate + records.
    pub phase_ms: [f32; 3],
    /// Quadtree breakdown (ms): corner gather, ray tracing, tile classification.
    pub quad_ms: [f32; 3],
    pub ms: f32,
}

/// Bias for the 15-bit anchor-relative coordinates in a face record.
pub(crate) const REC_BIAS: i32 = 1 << 14;

/// 8-byte face record: w0 = x(15) | z(15) << 15 | face(bits 0-1) << 30,
/// w1 = block(16) | y(15) << 16 | face(bit 2) << 31; coordinates relative to the anchor.
#[inline]
fn record(rel: IVec3, face: u32, block: u16) -> Option<[u32; 2]> {
    let b = rel + REC_BIAS;
    let lim = 1 << 15;
    if b.x < 0 || b.y < 0 || b.z < 0 || b.x >= lim || b.y >= lim || b.z >= lim {
        return None;
    }
    Some([b.x as u32 | (b.z as u32) << 15 | (face & 3) << 30, block as u32 | (b.y as u32) << 16 | (face >> 2) << 31])
}

/// Camera basis + widened FOV used to place discovery rays.
pub(crate) struct Rays {
    pub cb: IVec3,
    pub o: Vec3,
    f: Vec3,
    r: Vec3,
    u: Vec3,
    tx: f32,
    ty: f32,
    w: f32,
    h: f32,
}

impl Rays {
    pub(crate) fn new(cam: &Camera, w: u32, h: u32, margin: f32) -> Self {
        let (cb, o) = cam.split_position();
        let ty = (cam.fov_y * 0.5).tan() * (1.0 + margin);
        Self { cb, o, f: cam.forward(), r: cam.right(), u: cam.up(), tx: ty * w as f32 / h as f32, ty, w: w as f32, h: h as f32 }
    }

    /// Direction through image position (x, y) in pixels (same formula as the ray casters).
    #[inline]
    pub(crate) fn dir(&self, x: f32, y: f32) -> Vec3 {
        let sx = (2.0 * x / self.w - 1.0) * self.tx;
        let sy = (1.0 - 2.0 * y / self.h) * self.ty;
        (self.f + sx * self.r + sy * self.u).normalize()
    }
}

/// Traces one ray; returns (face key, plane id, hit point) or a miss.
#[inline]
pub(crate) fn cast(grid: &Grid, rays: &Rays, bmin: IVec3, bmax: IVec3, x: f32, y: f32) -> Option<(u64, u64, Vec3)> {
    let d = rays.dir(x, y);
    let h = grid.trace(rays.cb, rays.o, d, bmin, bmax)?;
    if h.axis > 2 {
        return None; // started inside a block
    }
    let a = h.axis as usize;
    let face = Face::entered(a, if d[a] > 0.0 { 1 } else { -1 }) as u32;
    let key = face_key(h.cell + rays.cb, face);
    let plane = face as u64 | (h.cell[a] as u32 as u64) << 3;
    Some((key, plane, rays.o + d * h.t))
}

enum Verdict {
    Done,
    Split,
}

pub(crate) fn discover(mut st: State, grid: &Grid, cam: &Camera, w: u32, h: u32, set: &Settings) -> Output {
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
    let mut tiles: Vec<[u32; 4]> = vec![];
    for y in (0..h).step_by(t as usize) {
        for x in (0..w).step_by(t as usize) {
            tiles.push([x, y, (x + t).min(w), (y + t).min(h)]);
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
        let traced: Vec<(Sample, Vec<u64>)> = need
            .par_iter()
            .map(|&i| {
                let (x, y) = (i % lw, i / lw);
                match cast(grid, &rays, bmin, bmax, x as f32 + 0.5, y as f32 + 0.5) {
                    Some((key, plane, p)) => {
                        let mut extra = vec![];
                        if dilate > 0 {
                            dilate_faces(grid, key, rays.cb, rays.o, dilate, &mut extra);
                        }
                        (Sample { key, plane, p }, extra)
                    }
                    None => (Sample { key: MISS, plane: MISS, p: Vec3::ZERO }, vec![]),
                }
            })
            .collect();
        for (&i, (s, extra)) in need.iter().zip(traced) {
            st.samples[i as usize] = s;
            if s.key != MISS {
                found.push(s.key);
            }
            found.extend(extra);
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
                        if footprint(grid, rays.cb, &c, set.max_footprint, &mut faces) { Verdict::Done } else { Verdict::Split }
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
    // 3. Merge into the temporal set and expire stale faces.
    let faces_found = found.len() as u64;
    found.par_sort_unstable();
    found.dedup();
    for k in found {
        st.s.insert(k, frame);
    }
    let keep = set.keep_frames.max(1);
    st.s.retain(|_, f| frame.wrapping_sub(*f) < keep);

    let t_merge = start.elapsed().as_secs_f32() * 1e3;
    // 4. Validate against the current world and build raster records.
    let anchor = rays.cb;
    let instances: Vec<[u32; 2]> = st
        .s
        .par_iter()
        .filter_map(|(&k, _)| {
            let (cell, face) = unpack_key(k);
            let block = grid.block(cell);
            if block == 0 || grid.block(cell + Face::from_index(face as usize).normal()) != 0 {
                return None;
            }
            record(cell - anchor, face, block)
        })
        .collect();
    Output { state: st, instances, anchor, rays: rays_cast, tiles_accepted: accepted, faces_found, quad_ms: [gather_ms, trace_ms, classify_ms], phase_ms: [t_quad, t_merge - t_quad, start.elapsed().as_secs_f32() * 1e3 - t_merge], ms: start.elapsed().as_secs_f32() * 1e3 }
}

/// Enumerates exposed faces on the common plane of the four corner samples inside the
/// bounding box of their hit points. Returns false (tile must split) if it is too large.
fn footprint(grid: &Grid, cb: IVec3, c: &[Sample; 4], max_cells: u32, out: &mut Vec<u64>) -> bool {
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
    for v in l2..=h2 {
        cell[a2] = v;
        for u in l1..=h1 {
            cell[a1] = u;
            let abs = cell + cb;
            if grid.block(abs) != 0 && grid.block(abs + n) == 0 {
                out.push(face_key(abs, face));
            }
        }
    }
    true
}

/// Adds the camera-facing exposed faces of the cells around a hit (catches sub-pixel
/// faces that the next frame's slightly different rays land on).
fn dilate_faces(grid: &Grid, key: u64, cb: IVec3, o: Vec3, level: u32, out: &mut Vec<u64>) {
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
    if level >= 2 {
        for z in -1..=1 {
            for y in -1..=1 {
                for x in -1..=1 {
                    visit(cell + IVec3::new(x, y, z));
                }
            }
        }
    } else {
        visit(cell);
        for f in Face::ALL {
            visit(cell + f.normal());
        }
    }
}
