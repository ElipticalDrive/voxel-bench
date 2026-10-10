use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use glam::{DVec3, IVec3};
use mr_core::chunk::local_of_block;
use mr_core::renderer::BlockEdit;
use mr_core::{BlockId, CHUNK_SIZE, ChunkData, ChunkKey};
use rayon::prelude::*;

use crate::generators::WorldGen;

/// The authoritative world owned by the host: a generator plus a cache of generated
/// chunks with edits applied.
///
/// Edits only affect LOD 0; coarser LODs always reflect the unedited generator output.
#[derive(Clone)]
pub struct World {
    pub generator: Arc<dyn WorldGen>,
    chunks: HashMap<ChunkKey, ChunkData>,
    /// LOD-0 chunks holding edits. These are never evicted.
    edited: HashSet<IVec3>,
}

impl World {
    pub fn new(generator: Arc<dyn WorldGen>) -> Self {
        Self { generator, chunks: HashMap::new(), edited: HashSet::new() }
    }

    /// Whether a chunk can be non-air: inside the generator's vertical range, or edited.
    pub fn may_contain_blocks(&self, key: ChunkKey) -> bool {
        if key.lod == 0 && self.edited.contains(&key.pos) {
            return true;
        }
        let (y0, y1) = self.generator.chunk_y_range();
        let ext = key.extent();
        let oy = key.pos.y * ext;
        oy < y1 * CHUNK_SIZE && oy + ext > y0 * CHUNK_SIZE
    }

    /// Fetches chunks, generating missing ones in parallel.
    pub fn get_many(&mut self, keys: &[ChunkKey]) -> Vec<ChunkData> {
        let missing: Vec<ChunkKey> = keys.iter().copied().filter(|k| !self.chunks.contains_key(k)).collect();
        if !missing.is_empty() {
            let generated: Vec<(ChunkKey, ChunkData)> = missing
                .par_iter()
                .map(|&k| {
                    let data =
                        if self.may_contain_blocks(k) { self.generator.chunk(k) } else { ChunkData::Uniform(BlockId::AIR) };
                    (k, data)
                })
                .collect();
            self.chunks.extend(generated);
        }
        keys.iter().map(|k| self.chunks[k].clone()).collect()
    }

    pub fn get(&mut self, key: ChunkKey) -> ChunkData {
        self.get_many(&[key]).pop().unwrap()
    }

    pub fn block(&mut self, p: IVec3) -> BlockId {
        let key = ChunkKey::of_block(p);
        if let Some(c) = self.chunks.get(&key) {
            return c.get(local_of_block(p));
        }
        self.get(key).get(local_of_block(p))
    }

    /// Applies edits and returns the LOD-0 chunks that changed, with their new contents.
    pub fn apply_edits(&mut self, edits: &[BlockEdit]) -> Vec<(ChunkKey, ChunkData)> {
        let mut changed = HashSet::new();
        for e in edits {
            let key = ChunkKey::of_block(e.pos);
            if !self.chunks.contains_key(&key) {
                self.get(key);
            }
            let chunk = self.chunks.get_mut(&key).unwrap();
            let local = local_of_block(e.pos);
            if chunk.get(local) != e.block {
                chunk.set(local, e.block);
                changed.insert(key);
                self.edited.insert(key.pos);
            }
        }
        changed
            .into_iter()
            .map(|k| {
                let c = self.chunks.remove(&k).unwrap().compact();
                self.chunks.insert(k, c.clone());
                (k, c)
            })
            .collect()
    }

    /// Drops cached chunks for which `keep` is false (edited chunks are always kept).
    pub fn evict(&mut self, keep: impl Fn(ChunkKey) -> bool) {
        let edited = &self.edited;
        self.chunks.retain(|k, _| keep(*k) || (k.lod == 0 && edited.contains(&k.pos)));
    }

    /// First solid block along a ray (voxel DDA), skipping the cell containing `origin`.
    /// Returns the block and the normal of the face that was hit.
    pub fn raycast(&mut self, origin: DVec3, dir: DVec3, max_dist: f64) -> Option<(IVec3, IVec3)> {
        let d = dir.normalize_or_zero();
        if d == DVec3::ZERO {
            return None;
        }
        let mut cell = origin.floor().as_ivec3();
        let step = d.signum().as_ivec3();
        let inv = d.recip().abs();
        let mut t_max = DVec3::ZERO;
        for a in 0..3 {
            let edge = if d[a] > 0.0 { cell[a] as f64 + 1.0 } else { cell[a] as f64 };
            t_max[a] = if d[a] == 0.0 { f64::INFINITY } else { (edge - origin[a]) / d[a] };
        }
        loop {
            let a = if t_max.x <= t_max.y && t_max.x <= t_max.z { 0 } else if t_max.y <= t_max.z { 1 } else { 2 };
            if t_max[a] > max_dist {
                return None;
            }
            cell[a] += step[a];
            t_max[a] += inv[a];
            if self.block(cell).is_solid() {
                let mut n = IVec3::ZERO;
                n[a] = -step[a];
                return Some((cell, n));
            }
        }
    }

    pub fn cached_chunks(&self) -> usize {
        self.chunks.len()
    }
}
