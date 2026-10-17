//! Self-directed visibility discovery on the CPU (PLAN step 13b: discovery's pieces as
//! components; was cpu-discover's `discover.rs`): the state, face keys, rays and the merge
//! into the face set shared by the samplers, [`crate::visibility::quadtree::cpu`] (an
//! adaptive screen-space quadtree of CPU rays that tries to find every block face visible
//! from a camera with as few rays as possible) and [`crate::visibility::cluster::cpu`]; and
//! the CPU world their rays walk ([`grid`]).
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

use glam::{IVec3, Vec3};
use mr_core::{Camera, Face};
use rayon::prelude::*;

pub mod grid;

use grid::World;

/// Multiplicative hasher for the packed u64 face keys (std SipHash is ~3x slower here).
#[derive(Default)]
pub struct KeyHasher(u64);

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

pub type FaceMap = HashMap<u64, u32, BuildHasherDefault<KeyHasher>>;

pub(crate) const MISS: u64 = u64::MAX;
const COORD_BITS: u32 = 20;
const COORD_MASK: u64 = (1 << COORD_BITS) - 1;

/// Packs absolute cell + face into a u64 key (20 bits per axis, two's complement).
#[inline]
pub fn face_key(cell: IVec3, face: u32) -> u64 {
    (cell.x as u64 & COORD_MASK) | (cell.y as u64 & COORD_MASK) << 20 | (cell.z as u64 & COORD_MASK) << 40 | (face as u64) << 60
}

#[inline]
pub fn unpack_key(k: u64) -> (IVec3, u32) {
    let s = |v: u64| ((v as i64) << (64 - COORD_BITS) >> (64 - COORD_BITS)) as i32;
    (IVec3::new(s(k & COORD_MASK), s(k >> 20 & COORD_MASK), s(k >> 40 & COORD_MASK)), (k >> 60) as u32)
}

#[derive(Clone, Debug)]
pub struct Settings {
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
    /// `cluster`: use the face-cluster discovery (`crate::visibility::cluster`) instead of the quadtree.
    pub cluster: bool,
    /// Cluster discovery: coarsest seed lattice spacing (px).
    pub seed_step: u32,
    /// Cluster discovery: a single seed miss covers its sky cell (else all four corners).
    pub sky_sparse: bool,
}

#[derive(Copy, Clone, Default)]
pub(crate) struct Sample {
    pub(crate) key: u64,
    /// Face direction | plane coordinate along its axis (camera-relative), MISS for sky.
    pub(crate) plane: u64,
    /// Hit point relative to the camera block.
    pub(crate) p: Vec3,
}

/// Persistent discovery state, moved into each background job and back.
#[derive(Default)]
pub struct State {
    /// The discovered face set S: face key -> frame it was last confirmed.
    pub s: FaceMap,
    pub(crate) samples: Vec<Sample>,
    pub(crate) stamp: Vec<u32>,
    pub(crate) generation: u32,
    pub frame: u32,
    /// Cluster discovery: per-pixel coverage of the last pass, and how each face was found
    /// (face key, kind, cluster id) for the diagnostic views.
    pub cover: Vec<u8>,
    pub kinds: Vec<(u64, u8, u32)>,
}

pub struct Output {
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
    /// Algorithm-specific counters.
    pub extra: Vec<(String, f64)>,
    /// Per instance, for the diagnostic views (cluster discovery; empty otherwise):
    /// kind << 30 | cluster id, 0 for faces carried over from earlier passes.
    pub tags: Vec<u32>,
}

/// Bias for the 15-bit anchor-relative coordinates in a face record.
pub const REC_BIAS: i32 = 1 << 14;

/// 8-byte face record: w0 = x(15) | z(15) << 15 | face(bits 0-1) << 30,
/// w1 = block(16) | y(15) << 16 | face(bit 2) << 31; coordinates relative to the anchor.
#[inline]
pub(crate) fn record(rel: IVec3, face: u32, block: u16) -> Option<[u32; 2]> {
    let b = rel + REC_BIAS;
    let lim = 1 << 15;
    if b.x < 0 || b.y < 0 || b.z < 0 || b.x >= lim || b.y >= lim || b.z >= lim {
        return None;
    }
    Some([b.x as u32 | (b.z as u32) << 15 | (face & 3) << 30, block as u32 | (b.y as u32) << 16 | (face >> 2) << 31])
}

/// Camera basis + widened FOV used to place discovery rays.
pub struct Rays {
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
    /// Pixel position (x, y) and view depth (z) of a camera-block-relative point, or `None`
    /// behind the camera.
    #[inline]
    pub fn project(&self, rel: Vec3) -> Option<Vec3> {
        let v = rel - self.o;
        let z = v.dot(self.f);
        if z <= 1e-3 {
            return None;
        }
        let sx = v.dot(self.r) / (z * self.tx);
        let sy = v.dot(self.u) / (z * self.ty);
        Some(Vec3::new((sx + 1.0) * 0.5 * self.w, (1.0 - sy) * 0.5 * self.h, z))
    }

    pub fn new(cam: &Camera, w: u32, h: u32, margin: f32) -> Self {
        let (cb, o) = cam.split_position();
        let ty = (cam.fov_y * 0.5).tan() * (1.0 + margin);
        Self { cb, o, f: cam.forward(), r: cam.right(), u: cam.up(), tx: ty * w as f32 / h as f32, ty, w: w as f32, h: h as f32 }
    }

    /// Direction through image position (x, y) in pixels (same formula as the ray casters).
    #[inline]
    pub fn dir(&self, x: f32, y: f32) -> Vec3 {
        let sx = (2.0 * x / self.w - 1.0) * self.tx;
        let sy = (1.0 - 2.0 * y / self.h) * self.ty;
        (self.f + sx * self.r + sy * self.u).normalize()
    }
}

/// Traces one ray; returns (face key, plane id, hit point) or a miss.
#[inline]
pub fn cast<G: World>(grid: &G, rays: &Rays, bmin: IVec3, bmax: IVec3, x: f32, y: f32) -> Option<(u64, u64, Vec3)> {
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

pub(crate) enum Verdict {
    Done,
    Split,
}

pub fn discover<G: World>(st: State, grid: &G, cam: &Camera, w: u32, h: u32, set: &Settings) -> Output {
    if set.cluster {
        return crate::visibility::cluster::cpu::discover(st, grid, cam, w, h, set);
    }
    crate::visibility::quadtree::cpu::quadtree(st, grid, cam, w, h, set)
}

/// Merges this pass's faces into the temporal set S (expiring faces not confirmed for
/// `keep_frames` passes), then validates S against the current world and builds the raster
/// records relative to `anchor`; with `tags`, also each record's tag (0 if absent).
pub fn finalize<G: World>(st: &mut State, mut found: Vec<u64>, grid: &G, anchor: IVec3, keep_frames: u32, tags: Option<&FaceMap>) -> (Vec<[u32; 2]>, Vec<u32>) {
    let frame = st.frame;
    found.par_sort_unstable();
    found.dedup();
    for k in found {
        st.s.insert(k, frame);
    }
    let keep = keep_frames.max(1);
    st.s.retain(|_, f| frame.wrapping_sub(*f) < keep);
    st.s
        .par_iter()
        .filter_map(|(&k, _)| {
            let (cell, face) = unpack_key(k);
            let block = grid.block(cell);
            if block == 0 || grid.block(cell + Face::from_index(face as usize).normal()) != 0 {
                return None;
            }
            record(cell - anchor, face, block).map(|r| (r, tags.and_then(|t| t.get(&k).copied()).unwrap_or(0)))
        })
        .unzip()
}

/// One ray per pixel for `cam`: (pixels whose face is missing from `s`, distinct missing
/// faces, pixels hitting geometry); `audit = true` measures discovery's holes with it.
pub fn audit<G: World>(grid: &G, s: &FaceMap, cam: &Camera, w: u32, h: u32) -> (u64, u64, u64) {
    let rays = Rays::new(cam, w, h, 0.0);
    let (bmin, bmax) = grid.bounds_rel(rays.cb);
    let (px, hit, mut keys) = (0..h)
        .into_par_iter()
        .map(|y| {
            let (mut miss, mut hit, mut keys) = (0u64, 0u64, vec![]);
            for x in 0..w {
                if let Some((k, _, _)) = cast(grid, &rays, bmin, bmax, x as f32 + 0.5, y as f32 + 0.5) {
                    hit += 1;
                    if !s.contains_key(&k) {
                        miss += 1;
                        keys.push(k);
                    }
                }
            }
            (miss, hit, keys)
        })
        .reduce(|| (0, 0, vec![]), |mut a, mut b| {
            a.2.append(&mut b.2);
            (a.0 + b.0, a.1 + b.1, a.2)
        });
    keys.sort_unstable();
    keys.dedup();
    (px, keys.len() as u64, hit)
}
