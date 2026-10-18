//! The server's stream (PLAN step 14a): one region around the camera (a radius per LOD
//! level: the largest any client needs there), the chunks in it as load / unload / edit
//! messages, the same for every client (`mr_core::client`, in each worker, derives what its renderer holds). Also walks the world for
//! tools (pre-generation, scene features).

use std::collections::{HashMap, HashSet};

use glam::{DVec3, IVec3};
use mr_core::chunk::CHUNK_BITS;
use mr_core::renderer::BlockEdit;
use mr_core::client::Levels;
use mr_core::{ChunkData, ChunkKey, WorldUpdate};
use mr_world::World;

/// What a client takes of the stream: its renderer's radius and LOD levels, and how it gets
/// the levels. The host sizes the server's region from its clients' (the clients run in the
/// workers: PLAN step 14b).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Region {
    pub radius: u32,
    pub max_lod: u8,
    pub levels: Levels,
}

impl Region {
    /// The radius per LOD level it needs streamed.
    pub fn reach(&self) -> Vec<u32> {
        mr_core::client::reach(self.radius, self.max_lod, self.levels)
    }
}

/// The server's region for clients needing `reaches`: the largest radius any needs at each
/// level (`fallback` at LOD 0 without clients).
pub fn union(reaches: impl IntoIterator<Item = Vec<u32>>, fallback: u32) -> Vec<u32> {
    let mut out: Vec<u32> = vec![];
    for r in reaches {
        out.resize(out.len().max(r.len()), 0);
        out.iter_mut().zip(r).for_each(|(o, r)| *o = (*o).max(r));
    }
    if out.is_empty() { vec![fallback] } else { out }
}

pub struct Streamer {
    /// The radius at each LOD level streamed.
    radii: Vec<i32>,
    budget: Option<usize>,
    /// Chunks the renderer knows about; `true` if it holds data (non-air).
    resident: HashMap<ChunkKey, bool>,
    centers: Vec<Option<IVec3>>,
    pending: Vec<ChunkKey>,
    pub chunks_sent: u64,
}

impl Streamer {
    /// A server streaming `radii[lod]` chunks around the camera at each level.
    pub fn new(radii: Vec<u32>, budget: Option<usize>) -> Self {
        assert!(!radii.is_empty());
        Self {
            centers: vec![None; radii.len()],
            radii: radii.into_iter().map(|r| r as i32).collect(),
            budget,
            resident: HashMap::new(),
            pending: vec![],
            chunks_sent: 0,
        }
    }

    /// Changes the streamed region: levels above it unload now, chunks outside a changed
    /// radius on the next update.
    pub fn set_radii(&mut self, radii: &[u32], out: &mut Vec<WorldUpdate>) {
        assert!(!radii.is_empty());
        let max_lod = radii.len() as u8 - 1;
        if radii.len() < self.radii.len() {
            self.pending.retain(|k| k.lod <= max_lod);
            let gone: Vec<ChunkKey> = self.resident.keys().filter(|k| k.lod > max_lod).copied().collect();
            for k in gone {
                if self.resident.remove(&k) == Some(true) {
                    out.push(WorldUpdate::ChunkUnload { key: k });
                }
            }
        }
        self.centers.resize(radii.len(), None);
        self.radii.resize(radii.len(), 0);
        for (lod, &r) in radii.iter().enumerate() {
            if self.radii[lod] != r as i32 {
                self.radii[lod] = r as i32;
                self.centers[lod] = None;
            }
        }
    }

    /// Chunks known to be needed but not yet delivered (budgeted streaming).
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Streaming updates for a camera position.
    pub fn update(&mut self, world: &mut World, cam: DVec3, out: &mut Vec<WorldUpdate>) {
        let mut moved = false;
        for lod in 0..self.radii.len() as u8 {
            let ext = (1i64 << (CHUNK_BITS + lod as u32)) as f64;
            let center = (cam / ext).floor().as_ivec3();
            if self.centers[lod as usize] == Some(center) {
                continue;
            }
            moved = true;
            self.centers[lod as usize] = Some(center);
            let r = self.radii[lod as usize];
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
            let radii = &self.radii;
            let dist = |k: &ChunkKey| {
                let c = centers[k.lod as usize].unwrap();
                let d = k.pos - c;
                ((d.x * d.x + d.y * d.y + d.z * d.z) as i64) << (2 * k.lod as i64)
            };
            let mut seen = HashSet::new();
            self.pending.retain(|k| {
                let c = centers[k.lod as usize].unwrap();
                (k.pos - c).abs().max_element() <= radii[k.lod as usize] && seen.insert(*k)
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
            let in_range = center.is_some_and(|c| (key.pos - c).abs().max_element() <= self.radii[0]);
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
    use mr_core::chunk::{CHUNK_BITS, local_of_block};
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
        let mut server = Streamer::new(vec![4; 3], None);
        let mut own = Streamer::new(vec![2; 2], None);
        let mut client = Client::new(2, 1, Levels::Pick);
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

    /// Behind a link that delays, loses and reorders the stream, a client's renderer holds
    /// less while it streams (holes), never an edit of a chunk it does not hold, and once
    /// everything arrived exactly what a client of the free link holds (PLAN step 14c).
    #[test]
    fn client_behind_a_link() {
        use mr_core::link::{Link, LinkSpec};
        let same = |a: &HashMap<ChunkKey, ChunkData>, b: &HashMap<ChunkKey, ChunkData>| {
            a.len() == b.len() && b.iter().all(|(k, d)| a.get(k).is_some_and(|c| c == d || (0..mr_core::CHUNK_VOLUME).map(mr_core::chunk::cell_of_index).all(|p| c.get(p) == d.get(p))))
        };
        let caught_up = |a: &HashMap<ChunkKey, ChunkData>, b: &HashMap<ChunkKey, ChunkData>| a.len() == b.len();
        for (levels, reorder) in [(Levels::Pick, false), (Levels::Pick, true), (Levels::Derive, true)] {
            let mut world = World::new(GenSpec::Terrain { seed: 3, caves: true, trees: true }.build());
            let mut server = Streamer::new(mr_core::client::reach(2, 2, levels), Some(40));
            let spec = LinkSpec { latency_ms: 40.0, jitter_ms: 120.0, bandwidth_mbps: 400.0, loss: 0.2, reorder, seed: 5 };
            let mut link = Link::new(spec);
            let (mut free, mut linked) = (Client::new(2, 2, levels), Client::new(2, 2, levels));
            let (mut a, mut b) = (HashMap::new(), HashMap::new());
            let mut holes = 0;
            // Until everything arrived and the renderer caught up.
            for f in 0..600 {
                if f > 60 && link.in_flight() == 0 && caught_up(&a, &b) {
                    break;
                }
                let t = f as f64 / 60.0;
                let g = f.min(30);
                let cam = DVec3::new(g as f64 * 13.0, 60.0, g as f64 * -7.0);
                let edits: Vec<BlockEdit> = if f < 30 { (0..20).map(|i| BlockEdit { pos: IVec3::new(f * 13 + i, 40 + i % 30, -f * 7 - i), block: if i % 2 == 0 { BlockId::AIR } else { BlockId(1) } }).collect() } else { vec![] };
                let changed = world.apply_edits(&edits);
                let (mut stream, mut ups, mut direct, mut got) = (vec![], vec![], vec![], vec![]);
                server.edits(&edits, &changed, &mut stream);
                server.update(&mut world, cam, &mut stream);
                free.receive(&stream, &mut direct);
                free.update(cam, &mut direct);
                link.send(t, stream);
                link.deliver(t, &mut got);
                linked.receive(&got, &mut ups);
                linked.update(cam, &mut ups);
                apply(&mut a, &ups);
                apply(&mut b, &direct);
                holes += (a.len() < b.len()) as u32;
            }
            assert_eq!(link.in_flight(), 0, "{levels:?} reorder {reorder}");
            assert!(holes > 5 && link.stats().resent > 0, "{levels:?} reorder {reorder}: holes {holes}, {:?}", link.stats());
            assert!(same(&a, &b), "{levels:?} reorder {reorder}: {} chunks held against {}", a.len(), b.len());
        }
    }

    /// The coarse cell `cell` of level `lod` as `derive` defines it: the topmost non-air of
    /// its eight finer cells (top layer first, then x, then z), down to the world's blocks.
    fn derived(world: &mut World, lod: u8, cell: IVec3) -> BlockId {
        if lod == 0 {
            return world.block(cell);
        }
        let order = [(0, 1, 0), (1, 1, 0), (0, 1, 1), (1, 1, 1), (0, 0, 0), (1, 0, 0), (0, 0, 1), (1, 0, 1)];
        order.iter().map(|&(x, y, z)| derived(world, lod - 1, cell * 2 + IVec3::new(x, y, z))).find(|b| !b.is_air()).unwrap_or(BlockId::AIR)
    }

    /// A `derive` client builds its coarse levels from the LOD 0 it receives: each held
    /// coarse chunk matches the world downsampled, chunks it does not hold inside its region
    /// are air, edits reach every level, and LOD 0 is what a stream of its own gives
    /// (PLAN step 14b).
    #[test]
    fn client_derives_its_levels() {
        let mut world = World::new(GenSpec::Terrain { seed: 3, caves: true, trees: true }.build());
        let (radius, max_lod) = (2, 2);
        let reach = mr_core::client::reach(radius, max_lod, Levels::Derive);
        assert_eq!(reach, vec![11]);
        // The server also streams levels for a client that picks them: `derive` ignores those.
        let mut server = Streamer::new(union([reach, vec![3; 3]], 0), None);
        let mut own = Streamer::new(vec![radius], None);
        let mut client = Client::new(radius, max_lod, Levels::Derive);
        let (mut a, mut b) = (HashMap::new(), HashMap::new());
        for f in 0..24 {
            let cam = DVec3::new(f as f64 * 13.0, 60.0, f as f64 * -7.0);
            let edits: Vec<BlockEdit> = (0..20).map(|i| BlockEdit { pos: IVec3::new(f * 13 + i * 3, 30 + i * 2, -f * 7 - i), block: if i % 2 == 0 { BlockId::AIR } else { BlockId(1) } }).collect();
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
            let lod0: HashMap<_, _> = a.iter().filter(|(k, _)| k.lod == 0).collect();
            assert_eq!(lod0.len(), b.len(), "frame {f}");
            for (k, d) in &b {
                assert!(lod0[k] == d || (0..mr_core::CHUNK_VOLUME).map(mr_core::chunk::cell_of_index).all(|p| lod0[k].get(p) == d.get(p)), "frame {f}: {k:?}");
            }
            // The coarse cells over this frame's edits.
            for lod in 1..=max_lod {
                for e in &edits {
                    let cell = e.pos >> lod as i32;
                    let key = ChunkKey { pos: cell >> CHUNK_BITS as i32, lod };
                    let center = (cam / (1u64 << (CHUNK_BITS + lod as u32)) as f64).floor().as_ivec3();
                    if (key.pos - center).abs().max_element() > radius as i32 {
                        continue;
                    }
                    let want = derived(&mut world, lod, cell);
                    let got = a.get(&key).map_or(BlockId::AIR, |c| c.get(cell - key.origin() / key.extent() * mr_core::CHUNK_SIZE));
                    assert_eq!(got, want, "frame {f}: edit {:?} at lod {lod}", e.pos);
                }
            }
            if f % 6 != 5 {
                continue;
            }
            // Every coarse chunk of the region, sampled.
            for lod in 1..=max_lod {
                let center = (cam / (1u64 << (CHUNK_BITS + lod as u32)) as f64).floor().as_ivec3();
                let r = radius as i32;
                for z in -r..=r {
                    for y in -r..=r {
                        for x in -r..=r {
                            let key = ChunkKey { pos: center + IVec3::new(x, y, z), lod };
                            let base = key.pos * mr_core::CHUNK_SIZE;
                            for i in (0..mr_core::CHUNK_VOLUME).step_by(997) {
                                let l = mr_core::chunk::cell_of_index(i);
                                let got = a.get(&key).map_or(BlockId::AIR, |c| c.get(l));
                                assert_eq!(got, derived(&mut world, lod, base + l), "frame {f}: {key:?} cell {l}");
                            }
                        }
                    }
                }
                assert!(a.keys().filter(|k| k.lod == lod).all(|k| (k.pos - center).abs().max_element() <= r), "frame {f}: a level-{lod} chunk outside the region");
            }
        }
    }
}
