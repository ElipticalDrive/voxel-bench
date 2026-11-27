//! Geometry `points` (was splat-gpu's code): one point per block with an
//! exposed face, for visibility `splat` (`crate::visibility::splat`). A [`Point`] is the
//! block's world position and `block | face_mask << 16` (bit `i` of the mask: the
//! neighbour across `Face::ALL[i]` is air). [`Points`] is the node's state: every chunk's
//! points in a slot of one array (the GPU buffer's mirror), rebuilt for changed chunks and
//! their neighbours; only the slots that changed are written.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Range;

use glam::IVec3;
use mr_core::{CHUNK_SIZE, ChunkData, Face};

pub type Point = [u32; 4];

/// The points of chunk `center` at chunk position `pos` (`neighbors` in `Face::ALL` order;
/// absent = air).
pub fn build(center: &ChunkData, neighbors: [Option<&ChunkData>; 6], pos: IVec3) -> Vec<Point> {
    if center.is_air() {
        return vec![];
    }
    let base = pos * CHUNK_SIZE;
    let mut out = vec![];
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let l = IVec3::new(x, y, z);
                let b = center.get(l);
                if b.is_air() {
                    continue;
                }
                let mut mask = 0u32;
                for (i, f) in Face::ALL.iter().enumerate() {
                    if super::get(center, &neighbors, l + f.normal()).is_air() {
                        mask |= 1 << i;
                    }
                }
                if mask != 0 {
                    let p = base + l;
                    out.push([p.x as u32, p.y as u32, p.z as u32, b.0 as u32 | mask << 16]);
                }
            }
        }
    }
    out
}

/// The geometry's half of [`Points::update`]: the points of the `changed` chunks and of
/// their 6 neighbours (exposure depends on them), in a fixed order; `chunk` gives a chunk's
/// data (absent = air).
pub fn rebuild<'a>(
    changed: impl IntoIterator<Item = IVec3>,
    chunk: impl Fn(IVec3) -> Option<&'a ChunkData>,
) -> Vec<(IVec3, Vec<Point>)> {
    let mut todo = HashSet::new();
    for c in changed {
        todo.insert(c);
        for f in Face::ALL {
            todo.insert(c + f.normal());
        }
    }
    let mut todo: Vec<IVec3> = todo.into_iter().collect();
    todo.sort_unstable_by_key(|c| (c.x, c.y, c.z));
    todo.into_iter()
        .map(|c| {
            let pts = chunk(c).map_or(vec![], |center| {
                build(center, Face::ALL.map(|f| chunk(c + f.normal())), c)
            });
            (c, pts)
        })
        .collect()
}

/// A slot's place in the points' array: `cap` entries from `off`, the first `len` the chunk's
/// points, the rest [`DEAD`].
#[derive(Clone, Copy, Debug)]
struct Slot {
    off: usize,
    cap: usize,
    len: usize,
}

/// A hole in the points' array: no face exposed, so the splat passes skip it.
pub const DEAD: Point = [0; 4];

/// Every chunk's points (the geometry node's state), kept as the GPU buffer holds them: one
/// array of per-chunk slots (room to grow for the chunks that outgrew theirs; holes
/// [`DEAD`]), so a change writes the
/// slots it touched, not the array. The order of the points is not fixed (splatting breaks
/// depth ties by position, not by index).
#[derive(Default)]
pub struct Points {
    slots: HashMap<IVec3, Slot>,
    /// The array (the GPU buffer's contents).
    all: Vec<Point>,
    /// The holes not at the end, by offset (length).
    free: BTreeMap<usize, usize>,
    /// The ranges of `all` written since the last [`Points::changes`].
    dirty: Vec<Range<usize>>,
    /// Every entry to be written (compacted since the last [`Points::changes`]).
    whole: bool,
    live: usize,
    /// Points written to the GPU so far (counter).
    pub uploaded: u64,
}

/// What [`Points::changes`] hands the transfer: the array and the ranges of it to write.
pub struct Changes<'a> {
    pub all: &'a [Point],
    pub ranges: Vec<Range<usize>>,
}

impl Points {
    /// Rebuilds the points of the `changed` chunks and of their 6 neighbours (exposure
    /// depends on them); `chunk` gives a chunk's data (absent = air). A chunk whose points
    /// did not change is not written.
    pub fn update<'a>(
        &mut self,
        changed: impl IntoIterator<Item = IVec3>,
        chunk: impl Fn(IVec3) -> Option<&'a ChunkData>,
    ) {
        self.apply(rebuild(changed, chunk));
    }

    /// The array's half of [`Points::update`]: each chunk's points become the given ones
    /// (none: the chunk is gone).
    pub fn apply(&mut self, chunks: impl IntoIterator<Item = (IVec3, Vec<Point>)>) {
        for (c, pts) in chunks {
            self.set(c, pts);
        }
        // Holes past a quarter of the array (the room to grow is at most a ninth): packed again,
        // so the splat passes do not run over them.
        if self.all.len() > 4096 && self.live * 4 < self.all.len() * 3 {
            self.compact();
        }
    }

    /// Chunk `c`'s points become `pts`.
    fn set(&mut self, c: IVec3, pts: Vec<Point>) {
        match self.slots.get(&c).copied() {
            Some(s) if pts.len() <= s.cap && !pts.is_empty() => {
                if self.all[s.off..s.off + s.len] == pts[..] {
                    return;
                }
                self.all[s.off..s.off + pts.len()].copy_from_slice(&pts);
                self.all[s.off + pts.len()..s.off + s.len.max(pts.len())].fill(DEAD);
                self.dirty.push(s.off..s.off + s.len.max(pts.len()));
                self.live = self.live + pts.len() - s.len;
                self.slots.insert(
                    c,
                    Slot {
                        len: pts.len(),
                        ..s
                    },
                );
            }
            old => {
                if let Some(s) = old {
                    self.slots.remove(&c);
                    self.release(s);
                }
                if !pts.is_empty() {
                    // Room to grow for a chunk that outgrew its slot; none on the first
                    // placement (a world that is not edited has no holes).
                    let room = old.map_or(0, |_| pts.len() / 8);
                    let s = self.place(pts.len(), room);
                    self.all[s.off..s.off + s.len].copy_from_slice(&pts);
                    // The room to grow too: the GPU buffer may hold stale points there.
                    self.dirty.push(s.off..s.off + s.cap);
                    self.live += s.len;
                    self.slots.insert(c, s);
                }
            }
        }
    }

    /// A slot for `n` points and `room` more: the first hole it fits, else the end.
    fn place(&mut self, n: usize, room: usize) -> Slot {
        let cap = n + room;
        let hole = self
            .free
            .iter()
            .find(|&(_, &len)| len >= cap)
            .map(|(&off, &len)| (off, len));
        let off = match hole {
            Some((off, len)) => {
                self.free.remove(&off);
                if len > cap {
                    self.free.insert(off + cap, len - cap);
                }
                off
            }
            None => {
                let off = self.all.len();
                self.all.resize(off + cap, DEAD);
                off
            }
        };
        Slot { off, cap, len: n }
    }

    /// Slot `s` becomes a hole (merged with its neighbours; at the end, the array shrinks).
    fn release(&mut self, s: Slot) {
        self.live -= s.len;
        self.all[s.off..s.off + s.len].fill(DEAD);
        self.dirty.push(s.off..s.off + s.len);
        let (mut off, mut len) = (s.off, s.cap);
        if let Some((&o, &l)) = self.free.range(..off).next_back()
            && o + l == off
        {
            self.free.remove(&o);
            (off, len) = (o, l + len);
        }
        if let Some(l) = self.free.remove(&(off + len)) {
            len += l;
        }
        if off + len == self.all.len() {
            self.all.truncate(off);
        } else {
            self.free.insert(off, len);
        }
    }

    /// Every chunk's points packed again, chunk by chunk in position order, room to grow
    /// kept for the slots that had some; all of it is written.
    fn compact(&mut self) {
        let mut keys: Vec<IVec3> = self.slots.keys().copied().collect();
        keys.sort_unstable_by_key(|c| (c.x, c.y, c.z));
        let old = std::mem::take(&mut self.all);
        self.free.clear();
        for c in keys {
            let s = self.slots[&c];
            let (off, room) = (self.all.len(), if s.cap > s.len { s.len / 8 } else { 0 });
            self.all.extend_from_slice(&old[s.off..s.off + s.len]);
            self.all.resize(off + s.len + room, DEAD);
            self.slots.insert(
                c,
                Slot {
                    off,
                    cap: s.len + room,
                    len: s.len,
                },
            );
        }
        self.whole = true;
    }

    /// The array and the ranges written since the last call (merged), if any.
    pub fn changes(&mut self) -> Option<Changes<'_>> {
        let mut ranges = std::mem::take(&mut self.dirty);
        if std::mem::take(&mut self.whole) {
            ranges.clear();
            ranges.push(0..self.all.len());
        }
        ranges.retain_mut(|r| {
            r.end = r.end.min(self.all.len());
            r.start < r.end
        });
        if ranges.is_empty() {
            return None;
        }
        ranges.sort_unstable_by_key(|r| r.start);
        let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
        for r in ranges {
            match merged.last_mut() {
                // Ranges a few points apart go as one write.
                Some(m) if r.start <= m.end + 64 => m.end = m.end.max(r.end),
                _ => merged.push(r),
            }
        }
        self.uploaded += merged.iter().map(|r| r.len() as u64).sum::<u64>();
        Some(Changes {
            all: &self.all,
            ranges: merged,
        })
    }

    /// Entries in the array (points and holes): what the splat passes run over.
    pub fn len(&self) -> usize {
        self.all.len()
    }

    pub fn is_empty(&self) -> bool {
        self.all.is_empty()
    }

    /// The points (holes not counted).
    pub fn live(&self) -> usize {
        self.live
    }

    pub fn bytes(&self) -> u64 {
        self.all.len() as u64 * 16
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mr_core::BlockId;

    /// Two touching blocks: both are points, each with the five faces not facing the other.
    #[test]
    fn points_have_exposed_faces() {
        let mut c = ChunkData::Uniform(BlockId::AIR);
        c.set(IVec3::new(3, 4, 5), BlockId(1));
        c.set(IVec3::new(4, 4, 5), BlockId(1));
        let pts = build(&c, [None; 6], IVec3::new(1, 0, 0));
        assert_eq!(pts.len(), 2);
        assert_eq!(pts[0][..3], [35, 4, 5]);
        for p in &pts {
            assert_eq!((p[3] >> 16).count_ones(), 5);
            assert_eq!(p[3] & 0xffff, 1);
        }
        let solid = ChunkData::Uniform(BlockId(2));
        assert!(build(&solid, [Some(&solid); 6], IVec3::ZERO).is_empty());
    }

    /// The GPU buffer as the transfer leaves it, kept by applying every change.
    fn apply(gpu: &mut Vec<Point>, p: &mut Points) {
        if let Some(ch) = p.changes() {
            gpu.resize(gpu.len().max(ch.all.len()), [9; 4]);
            for r in ch.ranges {
                gpu[r.clone()].copy_from_slice(&ch.all[r]);
            }
        }
        gpu.truncate(p.len());
    }

    /// Edits, streaming in and unloading, at random: what the GPU holds (written by the
    /// changes alone) is every chunk's points, each once, and holes; edits write a
    /// tenth of the points or less.
    #[test]
    fn writes_only_changes() {
        use std::collections::HashMap;
        let mut world: HashMap<IVec3, ChunkData> = HashMap::new();
        let mut rng = 0x2545f4914f6cdd1du64;
        let mut next = move |n: u64| {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng % n
        };
        let ground = |c: IVec3| {
            let mut d = ChunkData::Uniform(BlockId::AIR);
            for x in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    for y in 0..(4 + (x + z + c.x * 3) % 7) {
                        d.set(IVec3::new(x, y, z), BlockId(1 + (x % 3) as u16));
                    }
                }
            }
            d
        };
        let (mut p, mut gpu) = (Points::default(), Vec::new());
        // Points the edits wrote, and what writing every point per change would have.
        let (mut edits_wrote, mut edits_all) = (0u64, 0u64);
        for step in 0..300 {
            let c = IVec3::new(next(6) as i32 - 3, next(2) as i32 - 1, next(6) as i32 - 3);
            let before = p.uploaded;
            let edit = world.contains_key(&c) && next(3) > 0;
            match (world.contains_key(&c), next(4)) {
                (true, 0) => {
                    world.remove(&c);
                }
                (false, _) => {
                    world.insert(c, ground(c));
                }
                _ => {
                    let l = IVec3::new(next(32) as i32, next(12) as i32, next(32) as i32);
                    let d = world.get_mut(&c).unwrap();
                    let b = if d.get(l).is_air() {
                        BlockId(2)
                    } else {
                        BlockId::AIR
                    };
                    d.set(l, b);
                }
            }
            p.update([c], |k| world.get(&k));
            apply(&mut gpu, &mut p);
            if edit && step >= 50 {
                (edits_wrote, edits_all) = (
                    edits_wrote + p.uploaded - before,
                    edits_all + p.live() as u64,
                );
            }
            if step % 10 != 9 {
                continue;
            }
            let mut want: Vec<Point> = world
                .keys()
                .flat_map(|&k| {
                    build(
                        &world[&k],
                        Face::ALL.map(|f| world.get(&(k + f.normal()))),
                        k,
                    )
                })
                .collect();
            let mut have: Vec<Point> = gpu.iter().copied().filter(|q| *q != DEAD).collect();
            assert!(gpu.iter().all(|q| *q != [9; 4]), "an entry never written");
            want.sort_unstable();
            have.sort_unstable();
            assert_eq!(have.len(), p.live());
            assert!(
                have == want,
                "step {step}: the GPU's points differ ({} vs {})",
                have.len(),
                want.len()
            );
        }
        assert!(
            edits_wrote * 10 < edits_all,
            "edits wrote {edits_wrote} points of {edits_all}"
        );
    }
}
