//! Self-directed visibility discovery on the CPU (discovery's pieces as
//! components; was discover-cpu's `discover.rs`): the state, face keys, rays and the merge
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
//!   or (`strict`) any of its cells is not an exposed face of the plane, split instead;
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

/// A face of S: the pass it was last confirmed in and its slot in the drawn records
/// ([`NO_SLOT`]: not drawn, i.e. not exposed in the current world).
#[derive(Clone, Copy, Debug)]
pub struct Known {
    pub frame: u32,
    slot: u32,
}

const NO_SLOT: u32 = u32::MAX;

/// The discovered face set S by face key.
pub type FaceSet = HashMap<u64, Known, BuildHasherDefault<KeyHasher>>;

pub(crate) const MISS: u64 = u64::MAX;
const COORD_BITS: u32 = 20;
const COORD_MASK: u64 = (1 << COORD_BITS) - 1;

/// Packs absolute cell + face into a u64 key (20 bits per axis, two's complement).
#[inline]
pub fn face_key(cell: IVec3, face: u32) -> u64 {
    (cell.x as u64 & COORD_MASK)
        | (cell.y as u64 & COORD_MASK) << 20
        | (cell.z as u64 & COORD_MASK) << 40
        | (face as u64) << 60
}

#[inline]
pub fn unpack_key(k: u64) -> (IVec3, u32) {
    let s = |v: u64| ((v as i64) << (64 - COORD_BITS) >> (64 - COORD_BITS)) as i32;
    (
        IVec3::new(
            s(k & COORD_MASK),
            s(k >> 20 & COORD_MASK),
            s(k >> 40 & COORD_MASK),
        ),
        (k >> 60) as u32,
    )
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub tile: u32,
    pub face_criterion: bool,
    pub sky_tile: u32,
    pub keep_frames: u32,
    pub max_footprint: u32,
    /// Plane criterion: every footprint cell must be an exposed face of the plane, else the
    /// tile splits (a hole in the plane, such as a window, shows something else).
    pub strict: bool,
    /// Quadtree: shift the tile grid by a random offset every frame (temporal coverage).
    pub jitter: bool,
    pub margin: f32,
    /// Fine-level samples also add camera-facing exposed faces around the hit cell:
    /// 0 = off, 1 = the hit cell's, 2 = + its 6 neighbours', 3 = its 3x3x3 neighbourhood's.
    pub dilate: u32,
    /// Tile size (px) at or below which samples are dilated.
    pub dilate_below: u32,
    /// `cluster`: use the face-cluster discovery (`crate::visibility::cluster`) instead of the quadtree.
    pub cluster: bool,
    /// Cluster discovery: coarsest seed lattice spacing (px).
    pub seed_step: u32,
    /// Cluster discovery: a single seed miss covers its sky cell (else all four corners).
    pub sky_sparse: bool,
    /// Plane criterion: a footprint cell's face also needs this many cells toward the eye
    /// free of solid blocks, else the tile splits (a block floating in front of the plane
    /// that the corners' rays passed by); 0 = off.
    pub occluders: u32,
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
    /// The discovered face set S.
    pub s: FaceSet,
    /// The drawn records, one per slot and dense (a freed slot takes the last record), each
    /// slot's face key, and their origin (`None`: not placed yet). The GPU's copy follows
    /// through [`Records::writes`].
    recs: Vec<[u32; 2]>,
    keys: Vec<u64>,
    anchor: Option<IVec3>,
    pub(crate) samples: Vec<Sample>,
    pub(crate) stamp: Vec<u32>,
    pub(crate) generation: u32,
    pub frame: u32,
    /// Cluster discovery: per-pixel coverage of the last pass, and how each face was found
    /// (face key, kind, cluster id) for the diagnostic views.
    pub cover: Vec<u8>,
    pub kinds: Vec<(u64, u8, u32)>,
}

/// What a pass changed in the drawn records (the face set S validated against the world).
pub struct Records {
    /// The slots written since the last pass, ascending, with their records (see `record`);
    /// all of them when the anchor moved.
    pub writes: Vec<(u32, [u32; 2])>,
    /// Slots drawn (`0..count`, no holes).
    pub count: u32,
    /// Absolute block all records are relative to.
    pub anchor: IVec3,
    /// Per slot, for the diagnostic views (cluster discovery; empty otherwise):
    /// kind << 30 | cluster id, 0 for faces carried over from earlier passes.
    pub tags: Vec<u32>,
}

impl Records {
    /// Applies the writes to a copy of the records (`mirror`), resized to `count`.
    pub fn apply(&self, mirror: &mut Vec<[u32; 2]>) {
        mirror.resize(self.count as usize, [0; 2]);
        for &(s, r) in &self.writes {
            mirror[s as usize] = r;
        }
    }
}

pub struct Output {
    pub state: State,
    /// The drawn records' changes.
    pub records: Records,
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
    Some([
        b.x as u32 | (b.z as u32) << 15 | (face & 3) << 30,
        block as u32 | (b.y as u32) << 16 | (face >> 2) << 31,
    ])
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
        Some(Vec3::new(
            (sx + 1.0) * 0.5 * self.w,
            (1.0 - sy) * 0.5 * self.h,
            z,
        ))
    }

    pub fn new(cam: &Camera, w: u32, h: u32, margin: f32) -> Self {
        let (cb, o) = cam.split_position();
        let ty = (cam.fov_y * 0.5).tan() * (1.0 + margin);
        Self {
            cb,
            o,
            f: cam.forward(),
            r: cam.right(),
            u: cam.up(),
            tx: ty * w as f32 / h as f32,
            ty,
            w: w as f32,
            h: h as f32,
        }
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
pub fn cast<G: World>(
    grid: &G,
    rays: &Rays,
    bmin: IVec3,
    bmax: IVec3,
    x: f32,
    y: f32,
) -> Option<(u64, u64, Vec3)> {
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

pub fn discover<G: World>(
    st: State,
    grid: &G,
    cam: &Camera,
    w: u32,
    h: u32,
    set: &Settings,
) -> Output {
    if set.cluster {
        return crate::visibility::cluster::cpu::discover(st, grid, cam, w, h, set);
    }
    crate::visibility::quadtree::cpu::quadtree(st, grid, cam, w, h, set)
}

/// Merges this pass's faces into the temporal set S (expiring faces not confirmed for
/// `keep_frames` passes), then validates S against the current world and hands back what
/// changed in the drawn records: a face drawn keeps its slot and record while it stays exposed,
/// so a pass writes only the faces that appeared, vanished (the last record fills the hole)
/// or changed block. The records' anchor stays put until an exposed face falls out of its
/// range but not out of `cam`'s (the camera block's), then moves to `cam` and every record is
/// written; faces out of range of both are not drawn. With `tags`, also each slot's tag (0 if
/// absent).
pub fn finalize<G: World>(
    st: &mut State,
    mut found: Vec<u64>,
    grid: &G,
    cam: IVec3,
    keep_frames: u32,
    tags: Option<&FaceMap>,
) -> Records {
    let frame = st.frame;
    found.par_sort_unstable();
    found.dedup();
    for k in found {
        st.s.entry(k)
            .and_modify(|e| e.frame = frame)
            .or_insert(Known {
                frame,
                slot: NO_SLOT,
            });
    }
    let keep = keep_frames.max(1);
    let keys = &mut st.keys;
    st.s.retain(|_, e| {
        let kept = frame.wrapping_sub(e.frame) < keep;
        if !kept && e.slot != NO_SLOT {
            keys[e.slot as usize] = MISS;
        }
        kept
    });
    let changes = match st.anchor.map(|a| changes(st, grid, a, Some(cam))) {
        Some(Ok(c)) => c,
        _ => {
            // Not placed yet, or a face out of range: everything again around the camera
            // (faces out of range of it are dropped).
            st.anchor = Some(cam);
            st.recs.clear();
            st.keys.clear();
            st.s.values_mut().for_each(|e| e.slot = NO_SLOT);
            changes(st, grid, cam, None).unwrap_or_default()
        }
    };
    // Faces no longer exposed leave their slots; then the holes are filled from the end.
    let mut written = Vec::new();
    for (k, r) in &changes {
        if r.is_none() {
            let e = st.s.get_mut(k).expect("in S");
            st.keys[e.slot as usize] = MISS;
            e.slot = NO_SLOT;
        }
    }
    let mut holes: Vec<u32> = (0..st.keys.len() as u32)
        .filter(|&i| st.keys[i as usize] == MISS)
        .collect();
    holes.reverse();
    loop {
        while st.keys.last() == Some(&MISS) {
            st.keys.pop();
            st.recs.pop();
        }
        let Some(h) = holes.pop() else { break };
        if h as usize >= st.keys.len() {
            break;
        }
        let (k, r) = (st.keys.pop().expect("live"), st.recs.pop().expect("live"));
        st.keys[h as usize] = k;
        st.recs[h as usize] = r;
        st.s.get_mut(&k).expect("in S").slot = h;
        written.push(h);
    }
    // Faces newly exposed take a slot at the end; changed ones are rewritten in place.
    for (k, r) in changes {
        let Some(r) = r else { continue };
        let e = st.s.get_mut(&k).expect("in S");
        if e.slot == NO_SLOT {
            e.slot = st.recs.len() as u32;
            st.recs.push(r);
            st.keys.push(k);
        } else {
            st.recs[e.slot as usize] = r;
        }
        written.push(e.slot);
    }
    written.sort_unstable();
    written.dedup();
    let count = st.recs.len() as u32;
    let writes = written
        .into_iter()
        .take_while(|&s| s < count)
        .map(|s| (s, st.recs[s as usize]))
        .collect();
    let tags = tags.map_or_else(Vec::new, |t| {
        st.keys
            .par_iter()
            .map(|k| t.get(k).copied().unwrap_or(0))
            .collect()
    });
    Records {
        writes,
        count,
        anchor: st.anchor.expect("placed"),
        tags,
    }
}

/// The faces of S whose drawn record changes with `anchor`: `Some` = its record (newly
/// exposed or changed block), `None` = no longer exposed (or out of range). `Err` when an
/// exposed face is out of the records' range but in range of `cam`.
fn changes<G: World>(
    st: &State,
    grid: &G,
    anchor: IVec3,
    cam: Option<IVec3>,
) -> Result<Vec<(u64, Option<[u32; 2]>)>, ()> {
    st.s.par_iter()
        .filter_map(|(&k, e)| {
            let (cell, face) = unpack_key(k);
            let block = grid.block(cell);
            let exposed =
                block != 0 && grid.block(cell + Face::from_index(face as usize).normal()) == 0;
            let rec = if exposed {
                record(cell - anchor, face, block)
            } else {
                None
            };
            match (rec, e.slot) {
                (None, _)
                    if exposed && cam.is_some_and(|c| record(cell - c, face, block).is_some()) =>
                {
                    Some(Err(()))
                }
                (None, NO_SLOT) => None,
                (None, _) => Some(Ok((k, None))),
                (Some(r), s) if s != NO_SLOT && st.recs[s as usize] == r => None,
                (Some(r), _) => Some(Ok((k, Some(r)))),
            }
        })
        .collect()
}

/// One ray per pixel for `cam`: (pixels whose face is missing from `s`, distinct missing
/// faces, pixels hitting geometry); `audit = true` measures discovery's holes with it.
pub fn audit<G: World>(grid: &G, s: &FaceSet, cam: &Camera, w: u32, h: u32) -> (u64, u64, u64) {
    let rays = Rays::new(cam, w, h, 0.0);
    let (bmin, bmax) = grid.bounds_rel(rays.cb);
    let (px, hit, mut keys) = (0..h)
        .into_par_iter()
        .map(|y| {
            let (mut miss, mut hit, mut keys) = (0u64, 0u64, vec![]);
            for x in 0..w {
                if let Some((k, _, _)) =
                    cast(grid, &rays, bmin, bmax, x as f32 + 0.5, y as f32 + 0.5)
                {
                    hit += 1;
                    if !s.contains_key(&k) {
                        miss += 1;
                        keys.push(k);
                    }
                }
            }
            (miss, hit, keys)
        })
        .reduce(
            || (0, 0, vec![]),
            |mut a, mut b| {
                a.2.append(&mut b.2);
                (a.0 + b.0, a.1 + b.1, a.2)
            },
        );
    keys.sort_unstable();
    keys.dedup();
    (px, keys.len() as u64, hit)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::visibility::discover::grid::Hit;

    /// Blocks by cell; rays are not cast here.
    struct Blocks(HashMap<IVec3, u16>);

    impl World for Blocks {
        fn trace(&self, _: IVec3, _: Vec3, _: Vec3, _: IVec3, _: IVec3) -> Option<Hit> {
            None
        }
        fn block(&self, abs: IVec3) -> u16 {
            self.0.get(&abs).copied().unwrap_or(0)
        }
        fn bounds_rel(&self, _: IVec3) -> (IVec3, IVec3) {
            (IVec3::ZERO, IVec3::ZERO)
        }
    }

    /// A buffer written only by the passes' changes holds exactly the records of S's exposed
    /// faces, each once, through finds, expiry, block edits and a camera far enough away to move
    /// the anchor; and the passes write a small part of what rewriting every record would.
    #[test]
    fn writes_only_changes() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut rnd = |n: u32| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as u32
        };
        let mut world = Blocks(HashMap::new());
        for _ in 0..4000 {
            let c = IVec3::new(rnd(40) as i32, rnd(40) as i32, rnd(40) as i32);
            world.0.insert(c, 1 + rnd(5) as u16);
        }
        let cells: Vec<IVec3> = world.0.keys().copied().collect();
        let far: Vec<IVec3> = (0..200)
            .map(|i| IVec3::new(20_000 + i % 10, i / 10, 0))
            .collect();
        for &c in &far {
            world.0.insert(c, 1);
        }
        let (mut st, mut gpu) = (State::default(), Vec::new());
        let (mut written, mut full, mut anchors) = (0usize, 0usize, Vec::new());
        for pass in 0..200u32 {
            // A far camera for a few passes finds faces out of the anchor's range and moves it
            // (the faces near the origin out of range of both are not drawn, and the anchor
            // stays), then back.
            let away = (100..104).contains(&pass);
            let cam = if away {
                IVec3::new(20_000, 0, 0)
            } else {
                IVec3::new(rnd(8) as i32, 0, 0)
            };
            let at = if away { &far } else { &cells };
            let found: Vec<u64> = (0..300)
                .map(|_| face_key(at[rnd(at.len() as u32) as usize], rnd(6)))
                .collect();
            for _ in 0..20 {
                let c = cells[rnd(cells.len() as u32) as usize];
                world.0.insert(c, rnd(6) as u16);
            }
            st.frame = pass;
            let r = finalize(&mut st, found, &world, cam, 8, None);
            assert!(r.writes.windows(2).all(|w| w[0].0 < w[1].0));
            r.apply(&mut gpu);
            written += r.writes.len();
            full += r.count as usize;
            anchors.push(r.anchor);
            if pass == 103 {
                assert_eq!(r.count as usize, gpu.len());
                assert!(
                    st.s.keys().any(|&k| unpack_key(k).0.x < 100),
                    "near faces still in S, not drawn"
                );
            }
            let mut want: Vec<[u32; 2]> =
                st.s.keys()
                    .filter_map(|&k| {
                        let (cell, face) = unpack_key(k);
                        let block = world.block(cell);
                        (block != 0
                            && world.block(cell + Face::from_index(face as usize).normal()) == 0)
                            .then(|| record(cell - r.anchor, face, block))
                            .flatten()
                    })
                    .collect();
            let mut have = gpu.clone();
            want.sort_unstable();
            have.sort_unstable();
            assert_eq!(have, want, "pass {pass}");
        }
        anchors.dedup();
        assert_eq!(
            anchors.len(),
            3,
            "the anchor moved away and back: {anchors:?}"
        );
        assert!(written * 3 < full, "{written} records written of {full}");
    }
}
