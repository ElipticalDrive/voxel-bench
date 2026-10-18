//! A client of the server's stream (PLAN step 14a; on the renderer's side of the link since
//! 14b: the graph's `client` node, run by the worker before its renderer): keeps a replica
//! of the chunks the server sent and derives from it what its renderer holds (its own
//! radius and LOD levels), producing the renderer's load / unload / edit messages. The
//! server streams one region for every client; nothing here asks it for more.
//!
//! Its LOD levels come one of two ways ([`Levels`], the node's component): `pick` takes the
//! server's (the generator's coarse chunks); `derive` builds them here from LOD 0, so the
//! server sends LOD 0 only, further out, and edits reach the coarse levels.

use std::collections::{HashMap, HashSet};

use glam::IVec3;
use serde::{Deserialize, Serialize};
use crate::chunk::{CHUNK_BITS, CHUNK_SIZE, cell_index, local_of_block};
use crate::{BlockId, ChunkData, ChunkKey, DenseChunk, WorldUpdate};

/// How a client gets its LOD levels: the graph's `client` node's component.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Levels {
    /// The server's levels, as the generator makes them.
    #[default]
    Pick,
    /// Built on the client from LOD 0: each coarse cell shows the topmost non-air of its
    /// eight finer cells.
    Derive,
}

impl Levels {
    pub fn of(component: &str) -> Option<Self> {
        match component {
            "pick" => Some(Self::Pick),
            "derive" => Some(Self::Derive),
            _ => None,
        }
    }
}

/// The radius at each LOD level a client of `radius` and levels up to `max_lod` needs the
/// server to stream: `pick` its radius at each level; `derive` LOD 0 only, far enough out
/// to cover its coarsest level's chunks.
pub fn reach(radius: u32, max_lod: u8, levels: Levels) -> Vec<u32> {
    match levels {
        Levels::Derive if max_lod > 0 => vec![((radius + 1) << max_lod) - 1],
        _ => vec![radius; max_lod as usize + 1],
    }
}

/// A box of cells, both ends inclusive.
type Cells = (IVec3, IVec3);

pub struct Client {
    radius: i32,
    max_lod: u8,
    levels: Levels,
    /// Non-air chunks the server sent and has not unloaded (`derive`: LOD 0 only).
    replica: HashMap<ChunkKey, ChunkData>,
    /// `derive`: the non-air coarse chunks built from the replica.
    coarse: HashMap<ChunkKey, ChunkData>,
    /// `derive`: per level, the coarse chunks to rebuild and the cells in them.
    dirty: Vec<HashMap<IVec3, Cells>>,
    /// Chunks the renderer holds.
    held: HashSet<ChunkKey>,
    /// Chunks arrived since the last `update` (loaded there if inside).
    fresh: Vec<ChunkKey>,
    centers: Vec<Option<IVec3>>,
    /// Takes the server's whole region: the stream is its renderer's as it is (no replica).
    whole: bool,
}

impl Client {
    pub fn new(radius: u32, max_lod: u8, levels: Levels) -> Self {
        Self {
            radius: radius as i32,
            max_lod,
            levels,
            replica: HashMap::new(),
            coarse: HashMap::new(),
            dirty: vec![HashMap::new(); max_lod as usize + 1],
            held: HashSet::new(),
            fresh: vec![],
            centers: vec![None; max_lod as usize + 1],
            whole: false,
        }
    }

    /// A client of a server streaming `radii` (per LOD level) that will not change: one
    /// whose region covers it takes the stream as it is.
    pub fn follow(&mut self, radii: &[u32]) {
        let mine = reach(self.radius as u32, self.max_lod, self.levels);
        self.whole = !self.derives() && radii.len() <= mine.len() && radii.iter().zip(&mine).all(|(s, m)| m >= s);
    }

    pub fn radius(&self) -> u32 {
        self.radius as u32
    }

    pub fn max_lod(&self) -> u8 {
        self.max_lod
    }

    /// Builds its coarse levels (`derive` with levels above LOD 0).
    fn derives(&self) -> bool {
        self.levels == Levels::Derive && self.max_lod > 0
    }

    /// Changes the renderer's radius; chunks outside it are unloaded on the next update.
    pub fn set_radius(&mut self, radius: u32) {
        if radius as i32 != self.radius {
            self.radius = radius as i32;
            self.centers.iter_mut().for_each(|c| *c = None);
        }
    }

    /// Changes the levels the renderer takes (the replica keeps what arrived; `derive`
    /// rebuilds its coarse levels).
    pub fn set_max_lod(&mut self, max_lod: u8) {
        if max_lod != self.max_lod {
            self.max_lod = max_lod;
            self.centers = vec![None; max_lod as usize + 1];
            self.dirty = vec![HashMap::new(); max_lod as usize + 1];
            self.coarse.clear();
            if self.derives() {
                let keys: Vec<ChunkKey> = self.replica.keys().filter(|k| k.lod == 0).copied().collect();
                for k in keys {
                    self.mark(k, (IVec3::ZERO, IVec3::splat(CHUNK_SIZE - 1)));
                }
            }
        }
    }

    fn inside(&self, k: &ChunkKey) -> bool {
        self.centers.get(k.lod as usize).copied().flatten().is_some_and(|c| (k.pos - c).abs().max_element() <= self.radius)
    }

    fn data(&self, k: &ChunkKey) -> Option<&ChunkData> {
        if k.lod == 0 || !self.derives() { self.replica.get(k) } else { self.coarse.get(k) }
    }

    /// `derive`: the cells of `child` that changed make the cells above them in its parent
    /// level dirty.
    fn mark(&mut self, child: ChunkKey, (lo, hi): Cells) {
        if !self.derives() || child.lod >= self.max_lod {
            return;
        }
        let half = CHUNK_SIZE / 2;
        let off = (child.pos & 1) * half;
        let (lo, hi) = (off + (lo >> 1), off + (hi >> 1));
        let e = self.dirty[child.lod as usize + 1].entry(child.pos >> 1).or_insert((lo, hi));
        *e = (e.0.min(lo), e.1.max(hi));
    }

    /// Takes the server's messages: the replica follows them; edits, unloads and replaced
    /// chunks the renderer holds, lights and the environment go on at once, new chunks at
    /// the next `update`.
    pub fn receive(&mut self, stream: &[WorldUpdate], out: &mut Vec<WorldUpdate>) {
        if self.whole {
            out.extend_from_slice(stream);
            return;
        }
        let all = (IVec3::ZERO, IVec3::splat(CHUNK_SIZE - 1));
        for u in stream {
            match u {
                // The server's levels are for clients that pick them.
                WorldUpdate::ChunkLoad { key, .. } | WorldUpdate::ChunkUnload { key } if key.lod > 0 && self.derives() => {}
                WorldUpdate::ChunkLoad { key, data } => {
                    self.replica.insert(*key, data.clone());
                    self.mark(*key, all);
                    if self.held.contains(key) {
                        out.push(u.clone());
                    } else {
                        self.fresh.push(*key);
                    }
                }
                WorldUpdate::ChunkUnload { key } => {
                    self.replica.remove(key);
                    self.mark(*key, all);
                    if self.held.remove(key) {
                        out.push(u.clone());
                    }
                }
                WorldUpdate::SetBlocks(edits) => {
                    for e in edits {
                        let key = ChunkKey::of_block(e.pos);
                        if let Some(c) = self.replica.get_mut(&key) {
                            let local = local_of_block(e.pos);
                            c.set(local, e.block);
                            self.mark(key, (local, local));
                        }
                    }
                    let held: Vec<_> = edits.iter().filter(|e| self.held.contains(&ChunkKey::of_block(e.pos))).copied().collect();
                    if !held.is_empty() {
                        out.push(WorldUpdate::SetBlocks(held));
                    }
                }
                _ => out.push(u.clone()),
            }
        }
    }

    /// `derive`: rebuilds the dirty cells of the coarse levels, finest first; held chunks
    /// that changed are sent again (or unloaded, now air), new ones wait in `fresh`.
    fn derive(&mut self, out: &mut Vec<WorldUpdate>) {
        for lod in 1..=self.max_lod {
            let dirty = std::mem::take(&mut self.dirty[lod as usize]);
            for (pos, cells) in dirty {
                let key = ChunkKey { pos, lod };
                let mut c = self.coarse.remove(&key).unwrap_or(ChunkData::Uniform(BlockId::AIR));
                if !self.build(key, cells, &mut c) {
                    if !c.is_air() {
                        self.coarse.insert(key, c);
                    }
                    continue;
                }
                self.mark(key, cells);
                let c = c.compact();
                if c.is_air() {
                    if self.held.remove(&key) {
                        out.push(WorldUpdate::ChunkUnload { key });
                    }
                    continue;
                }
                if self.held.contains(&key) {
                    out.push(WorldUpdate::ChunkLoad { key, data: c.clone() });
                } else {
                    self.fresh.push(key);
                }
                self.coarse.insert(key, c);
            }
        }
    }

    /// Fills `cells` of coarse chunk `key` from the level below; whether any changed.
    /// Each cell shows the topmost non-air of its eight (top layer first, then x, then z).
    fn build(&self, key: ChunkKey, (lo, hi): Cells, c: &mut ChunkData) -> bool {
        // The eight finer cells' offsets from the first (`cell_index` order: x, z, y).
        const ORDER: [usize; 8] = [1024, 1025, 1056, 1057, 0, 1, 32, 33];
        let half = CHUNK_SIZE / 2;
        let mut out: Option<DenseChunk> = None;
        let mut changed = false;
        // Each octant of the chunk comes from one child chunk.
        for o in 0..8 {
            let oct = IVec3::new(o & 1, (o >> 1) & 1, o >> 2);
            let (olo, ohi) = ((oct * half).max(lo), (oct * half + (half - 1)).min(hi));
            if olo.cmpgt(ohi).any() {
                continue;
            }
            let child = self.data(&ChunkKey { pos: key.pos * 2 + oct, lod: key.lod - 1 });
            let fill = match child {
                None => Some(BlockId::AIR),
                Some(ChunkData::Uniform(b)) => Some(*b),
                Some(ChunkData::Dense(_)) => None,
            };
            // A uniform chunk keeps its block where the octant brings the same.
            if let (Some(b), ChunkData::Uniform(u), None) = (fill, &*c, &out) {
                if b == *u {
                    continue;
                }
            }
            let d = out.get_or_insert_with(|| match c {
                ChunkData::Uniform(u) => DenseChunk::filled(*u),
                ChunkData::Dense(d) => DenseChunk(d.0.clone()),
            });
            for y in olo.y..=ohi.y {
                for z in olo.z..=ohi.z {
                    for x in olo.x..=ohi.x {
                        let p = IVec3::new(x, y, z);
                        let b = match (fill, child) {
                            (Some(b), _) => b,
                            (None, Some(ChunkData::Dense(src))) => {
                                let base = cell_index((p - oct * half) * 2);
                                ORDER.iter().map(|q| src.0[base + q]).find(|b| !b.is_air()).unwrap_or(BlockId::AIR)
                            }
                            _ => unreachable!(),
                        };
                        let cell = &mut d.0[cell_index(p)];
                        changed |= *cell != b;
                        *cell = b;
                    }
                }
            }
        }
        if changed {
            *c = ChunkData::Dense(std::sync::Arc::new(out.unwrap()));
        }
        changed
    }

    /// The renderer's messages for a camera position: chunks leaving its region unload,
    /// chunks of the replica inside it load, nearest first.
    pub fn update(&mut self, cam: glam::DVec3, out: &mut Vec<WorldUpdate>) {
        if self.whole {
            return;
        }
        if self.derives() {
            self.derive(out);
        }
        let mut moved = false;
        for lod in 0..=self.max_lod {
            let ext = (1i64 << (CHUNK_BITS + lod as u32)) as f64;
            let center = (cam / ext).floor().as_ivec3();
            if self.centers[lod as usize] != Some(center) {
                self.centers[lod as usize] = Some(center);
                moved = true;
            }
        }
        if moved {
            let gone: Vec<ChunkKey> = self.held.iter().filter(|k| !self.inside(k)).copied().collect();
            for k in gone {
                self.held.remove(&k);
                out.push(WorldUpdate::ChunkUnload { key: k });
            }
        }
        let mut loads: Vec<ChunkKey> = if moved {
            self.fresh.clear();
            let coarse = self.coarse.keys().filter(|_| self.derives());
            self.replica.keys().filter(|k| k.lod == 0 || !self.derives()).chain(coarse).filter(|k| !self.held.contains(k) && self.inside(k)).copied().collect()
        } else {
            let mut seen = HashSet::new();
            std::mem::take(&mut self.fresh).into_iter().filter(|k| self.data(k).is_some() && !self.held.contains(k) && self.inside(k) && seen.insert(*k)).collect()
        };
        let centers = &self.centers;
        loads.sort_unstable_by_key(|k| {
            let d = k.pos - centers[k.lod as usize].unwrap();
            (((d.x * d.x + d.y * d.y + d.z * d.z) as i64) << (2 * k.lod as i64), k.lod, k.pos.to_array())
        });
        for key in loads {
            self.held.insert(key);
            out.push(WorldUpdate::ChunkLoad { key, data: self.data(&key).unwrap().clone() });
        }
    }
}
