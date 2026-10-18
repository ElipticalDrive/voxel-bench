//! A client of the server's stream (PLAN step 14a): keeps a replica of the chunks the
//! server sent and derives from it what its renderer holds (its own radius and LOD levels),
//! producing the renderer's load / unload / edit messages. The server streams one region
//! for every client; nothing here asks it for more.

use std::collections::{HashMap, HashSet};

use glam::IVec3;
use mr_core::chunk::{CHUNK_BITS, local_of_block};
use mr_core::{ChunkData, ChunkKey, WorldUpdate};

pub struct Client {
    radius: i32,
    max_lod: u8,
    /// Non-air chunks the server sent and has not unloaded.
    replica: HashMap<ChunkKey, ChunkData>,
    /// Chunks the renderer holds.
    held: HashSet<ChunkKey>,
    /// Chunks arrived since the last `update` (loaded there if inside).
    fresh: Vec<ChunkKey>,
    centers: Vec<Option<IVec3>>,
    /// Takes the server's whole region: the stream is its renderer's as it is (no replica).
    whole: bool,
    pub chunks_sent: u64,
}

impl Client {
    pub fn new(radius: u32, max_lod: u8) -> Self {
        Self { radius: radius as i32, max_lod, replica: HashMap::new(), held: HashSet::new(), fresh: vec![], centers: vec![None; max_lod as usize + 1], whole: false, chunks_sent: 0 }
    }

    /// A client of a server streaming `radius` and levels up to `max_lod` that will not
    /// change: one whose region covers it takes the stream as it is.
    pub fn follow(&mut self, radius: u32, max_lod: u8) {
        self.whole = self.radius >= radius as i32 && self.max_lod >= max_lod;
    }

    pub fn radius(&self) -> u32 {
        self.radius as u32
    }

    pub fn max_lod(&self) -> u8 {
        self.max_lod
    }

    /// Changes the renderer's radius; chunks outside it are unloaded on the next update.
    pub fn set_radius(&mut self, radius: u32) {
        if radius as i32 != self.radius {
            self.radius = radius as i32;
            self.centers.iter_mut().for_each(|c| *c = None);
        }
    }

    /// Changes the levels the renderer takes (the replica keeps what arrived).
    pub fn set_max_lod(&mut self, max_lod: u8) {
        if max_lod != self.max_lod {
            self.max_lod = max_lod;
            self.centers = vec![None; max_lod as usize + 1];
        }
    }

    fn inside(&self, k: &ChunkKey) -> bool {
        self.centers.get(k.lod as usize).copied().flatten().is_some_and(|c| (k.pos - c).abs().max_element() <= self.radius)
    }

    /// Takes the server's messages: the replica follows them; edits, unloads and replaced
    /// chunks the renderer holds, lights and the environment go on at once, new chunks at
    /// the next `update`.
    pub fn receive(&mut self, stream: &[WorldUpdate], out: &mut Vec<WorldUpdate>) {
        if self.whole {
            self.chunks_sent += stream.iter().filter(|u| matches!(u, WorldUpdate::ChunkLoad { .. })).count() as u64;
            out.extend_from_slice(stream);
            return;
        }
        for u in stream {
            match u {
                WorldUpdate::ChunkLoad { key, data } => {
                    self.replica.insert(*key, data.clone());
                    if self.held.contains(key) {
                        out.push(u.clone());
                        self.chunks_sent += 1;
                    } else {
                        self.fresh.push(*key);
                    }
                }
                WorldUpdate::ChunkUnload { key } => {
                    self.replica.remove(key);
                    if self.held.remove(key) {
                        out.push(u.clone());
                    }
                }
                WorldUpdate::SetBlocks(edits) => {
                    for e in edits {
                        if let Some(c) = self.replica.get_mut(&ChunkKey::of_block(e.pos)) {
                            c.set(local_of_block(e.pos), e.block);
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

    /// The renderer's messages for a camera position: chunks leaving its region unload,
    /// chunks of the replica inside it load, nearest first.
    pub fn update(&mut self, cam: glam::DVec3, out: &mut Vec<WorldUpdate>) {
        if self.whole {
            return;
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
            self.replica.keys().filter(|k| !self.held.contains(k) && self.inside(k)).copied().collect()
        } else {
            let mut seen = HashSet::new();
            std::mem::take(&mut self.fresh).into_iter().filter(|k| self.replica.contains_key(k) && !self.held.contains(k) && self.inside(k) && seen.insert(*k)).collect()
        };
        let centers = &self.centers;
        loads.sort_unstable_by_key(|k| {
            let d = k.pos - centers[k.lod as usize].unwrap();
            (((d.x * d.x + d.y * d.y + d.z * d.z) as i64) << (2 * k.lod as i64), k.lod, k.pos.to_array())
        });
        for key in loads {
            self.held.insert(key);
            out.push(WorldUpdate::ChunkLoad { key, data: self.replica[&key].clone() });
            self.chunks_sent += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::Streamer;
    use glam::DVec3;
    use mr_core::BlockId;
    use mr_core::renderer::BlockEdit;
    use mr_world::World;
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

    /// A client of a larger stream holds what a stream of its own region would give it.
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
