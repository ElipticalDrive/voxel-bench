//! Per-renderer chunk streaming: tracks what each renderer holds and produces the
//! load/unload/edit messages to keep it in sync with the camera and the world.

use std::collections::{HashMap, HashSet};

use glam::{DVec3, IVec3};
use mr_core::chunk::CHUNK_BITS;
use mr_core::renderer::BlockEdit;
use mr_core::{ChunkData, ChunkKey, WorldUpdate};
use mr_world::World;

pub struct Streamer {
    radius: i32,
    max_lod: u8,
    budget: Option<usize>,
    /// Chunks the renderer knows about; `true` if it holds data (non-air).
    resident: HashMap<ChunkKey, bool>,
    centers: Vec<Option<IVec3>>,
    pending: Vec<ChunkKey>,
    pub chunks_sent: u64,
}

impl Streamer {
    pub fn new(radius: u32, max_lod: u8, budget: Option<usize>) -> Self {
        Self {
            radius: radius as i32,
            max_lod,
            budget,
            resident: HashMap::new(),
            centers: vec![None; max_lod as usize + 1],
            pending: vec![],
            chunks_sent: 0,
        }
    }

    pub fn radius(&self) -> u32 {
        self.radius as u32
    }

    /// Changes the streamed radius; chunks outside it are unloaded on the next update.
    pub fn set_radius(&mut self, radius: u32) {
        if radius as i32 != self.radius {
            self.radius = radius as i32;
            self.centers.iter_mut().for_each(|c| *c = None);
        }
    }

    pub fn set_budget(&mut self, budget: Option<usize>) {
        self.budget = budget;
    }

    /// Chunks known to be needed but not yet delivered (budgeted streaming).
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Streaming updates for a camera position.
    pub fn update(&mut self, world: &mut World, cam: DVec3, out: &mut Vec<WorldUpdate>) {
        let mut moved = false;
        for lod in 0..=self.max_lod {
            let ext = (1i64 << (CHUNK_BITS + lod as u32)) as f64;
            let center = (cam / ext).floor().as_ivec3();
            if self.centers[lod as usize] == Some(center) {
                continue;
            }
            moved = true;
            self.centers[lod as usize] = Some(center);
            let r = self.radius;
            let inside = |k: &ChunkKey| k.lod != lod || (k.pos - center).abs().max_element() <= r;
            let unloads: Vec<ChunkKey> = self.resident.keys().filter(|k| !inside(k)).copied().collect();
            for k in unloads {
                if self.resident.remove(&k) == Some(true) {
                    out.push(WorldUpdate::ChunkUnload { key: k });
                }
            }
            for z in -r..=r {
                for y in -r..=r {
                    for x in -r..=r {
                        let key = ChunkKey { pos: center + IVec3::new(x, y, z), lod };
                        if !self.resident.contains_key(&key) && world.may_contain_blocks(key) {
                            self.pending.push(key);
                        }
                    }
                }
            }
        }
        if moved {
            // Drop stale entries, dedupe, and order nearest-first (far end of the vector
            // is popped first, so sort descending).
            let centers = self.centers.clone();
            let r = self.radius;
            let dist = |k: &ChunkKey| {
                let c = centers[k.lod as usize].unwrap();
                let d = k.pos - c;
                ((d.x * d.x + d.y * d.y + d.z * d.z) as i64) << (2 * k.lod as i64)
            };
            let mut seen = HashSet::new();
            self.pending.retain(|k| {
                let c = centers[k.lod as usize].unwrap();
                (k.pos - c).abs().max_element() <= r && seen.insert(*k)
            });
            self.pending.sort_unstable_by_key(|k| std::cmp::Reverse(dist(k)));
        }
        if self.pending.is_empty() {
            return;
        }
        let n = self.budget.unwrap_or(usize::MAX).min(self.pending.len());
        let batch: Vec<ChunkKey> = self.pending.split_off(self.pending.len() - n);
        let data = world.get_many(&batch);
        for (key, data) in batch.into_iter().zip(data) {
            let solid = !data.is_air();
            self.resident.insert(key, solid);
            if solid {
                out.push(WorldUpdate::ChunkLoad { key, data });
                self.chunks_sent += 1;
            }
        }
    }

    /// Forwards edits: block edits for chunks the renderer holds, full chunk loads for
    /// chunks it knew as air. `changed` are the LOD-0 chunks the edits modified.
    pub fn edits(&mut self, edits: &[BlockEdit], changed: &[(ChunkKey, ChunkData)], out: &mut Vec<WorldUpdate>) {
        if edits.is_empty() {
            return;
        }
        let held: Vec<BlockEdit> =
            edits.iter().filter(|e| self.resident.get(&ChunkKey::of_block(e.pos)) == Some(&true)).copied().collect();
        if !held.is_empty() {
            out.push(WorldUpdate::SetBlocks(held));
        }
        let center = self.centers[0];
        for (key, data) in changed {
            // Chunks it knew as air, or never considered (e.g. built above the generator's
            // height range) but within range.
            let in_range = center.is_some_and(|c| (key.pos - c).abs().max_element() <= self.radius);
            let unknown_in_range = !self.resident.contains_key(key) && in_range && !self.pending.contains(key);
            if (self.resident.get(key) == Some(&false) || unknown_in_range) && !data.is_air() {
                self.resident.insert(*key, true);
                out.push(WorldUpdate::ChunkLoad { key: *key, data: data.clone() });
                self.chunks_sent += 1;
            }
        }
    }
}
