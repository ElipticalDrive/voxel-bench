//! The server's stream (PLAN step 14a): one region around the camera (a radius at each
//! LOD level), the chunks in it as load / unload / edit messages, the same for every
//! client (`mr_core::client`, in each worker, derives what its renderer holds). Also walks the world for
//! tools (pre-generation, scene features).

use std::collections::{HashMap, HashSet};

use glam::{DVec3, IVec3};
use mr_core::chunk::CHUNK_BITS;
use mr_core::renderer::BlockEdit;
use mr_core::{ChunkData, ChunkKey, WorldUpdate};
use mr_world::World;

/// What a client takes of the stream: its renderer's radius and LOD levels. The host sizes
/// the server's region from its clients' (the clients run in the workers: PLAN step 14b).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Region {
    pub radius: u32,
    pub max_lod: u8,
}

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

    /// Changes the streamed radius; chunks outside it are unloaded on the next update.
    pub fn set_radius(&mut self, radius: u32) {
        if radius as i32 != self.radius {
            self.radius = radius as i32;
            self.centers.iter_mut().for_each(|c| *c = None);
        }
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

    /// What a client joining now receives: every chunk the stream holds.
    pub fn snapshot(&self, world: &mut World) -> Vec<WorldUpdate> {
        let keys: Vec<ChunkKey> = self.resident.iter().filter(|e| *e.1).map(|e| *e.0).collect();
        let data = world.get_many(&keys);
        keys.into_iter().zip(data).map(|(key, data)| WorldUpdate::ChunkLoad { key, data }).collect()
    }

    /// Changes the streamed levels; levels above it unload on the next update.
    pub fn set_max_lod(&mut self, max_lod: u8, out: &mut Vec<WorldUpdate>) {
        if max_lod == self.max_lod {
            return;
        }
        self.max_lod = max_lod;
        self.centers.resize(max_lod as usize + 1, None);
        self.pending.retain(|k| k.lod <= max_lod);
        let gone: Vec<ChunkKey> = self.resident.keys().filter(|k| k.lod > max_lod).copied().collect();
        for k in gone {
            if self.resident.remove(&k) == Some(true) {
                out.push(WorldUpdate::ChunkUnload { key: k });
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

#[cfg(test)]
mod tests {
    use super::*;
    use mr_core::BlockId;
    use mr_core::chunk::local_of_block;
    use mr_core::client::Client;
    use mr_world::generators::GenSpec;

    /// What a renderer holds after applying messages.
    fn apply(held: &mut HashMap<ChunkKey, ChunkData>, ups: &[WorldUpdate]) {
        for u in ups {
            match u {
                WorldUpdate::ChunkLoad { key, data } => {
                    held.insert(*key, data.clone());
                }
                WorldUpdate::ChunkUnload { key } => {
                    held.remove(key);
                }
                WorldUpdate::SetBlocks(edits) => {
                    for e in edits {
                        held.get_mut(&ChunkKey::of_block(e.pos)).expect("an edit of a chunk not held").set(local_of_block(e.pos), e.block);
                    }
                }
                _ => {}
            }
        }
    }

    /// A client of a larger stream holds what a stream of its own region would give it
    /// (PLAN step 14a; the client runs in the worker since 14b).
    #[test]
    fn client_derives_its_region() {
        let mut world = World::new(GenSpec::Terrain { seed: 3, caves: true, trees: true }.build());
        let mut server = Streamer::new(4, 2, None);
        let mut own = Streamer::new(2, 1, None);
        let mut client = Client::new(2, 1);
        let (mut a, mut b) = (HashMap::new(), HashMap::new());
        for f in 0..40 {
            let cam = DVec3::new(f as f64 * 13.0, 60.0, f as f64 * -7.0);
            let edits: Vec<BlockEdit> = (0..20).map(|i| BlockEdit { pos: IVec3::new(f * 13 + i, 40 + i % 30, -f * 7 - i), block: if i % 2 == 0 { BlockId::AIR } else { BlockId(1) } }).collect();
            let changed = world.apply_edits(&edits);
            let (mut stream, mut ups, mut direct) = (vec![], vec![], vec![]);
            server.edits(&edits, &changed, &mut stream);
            server.update(&mut world, cam, &mut stream);
            client.receive(&stream, &mut ups);
            client.update(cam, &mut ups);
            own.edits(&edits, &changed, &mut direct);
            own.update(&mut world, cam, &mut direct);
            apply(&mut a, &ups);
            apply(&mut b, &direct);
            assert_eq!(a.len(), b.len(), "frame {f}");
            for (k, d) in &b {
                let c = &a[k];
                let cells = (0..mr_core::CHUNK_VOLUME).map(mr_core::chunk::cell_of_index);
                assert!(c == d || cells.into_iter().all(|p| c.get(p) == d.get(p)), "frame {f}: {k:?}");
            }
        }
    }
}
