//! Geometry `points` (was gpu-splat's code): one point per block with an
//! exposed face, for visibility `splat` (`crate::visibility::splat`). A [`Point`] is the
//! block's world position and `block | face_mask << 16` (bit `i` of the mask: the
//! neighbour across `Face::ALL[i]` is air). [`Points`] is the node's state: every chunk's
//! points, rebuilt for changed chunks and their neighbours.

use std::collections::{HashMap, HashSet};

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

/// Every chunk's points (the geometry node's state).
#[derive(Default)]
pub struct Points {
    chunks: HashMap<IVec3, Vec<Point>>,
    dirty: bool,
}

impl Points {
    /// Rebuilds the points of the `changed` chunks and of their 6 neighbours (exposure
    /// depends on them); `chunk` gives a chunk's data (absent = air).
    pub fn update<'a>(&mut self, changed: impl IntoIterator<Item = IVec3>, chunk: impl Fn(IVec3) -> Option<&'a ChunkData>) {
        let mut todo = HashSet::new();
        for c in changed {
            todo.insert(c);
            for f in Face::ALL {
                todo.insert(c + f.normal());
            }
        }
        for c in todo {
            let pts = chunk(c).map_or(vec![], |center| build(center, Face::ALL.map(|f| chunk(c + f.normal())), c));
            if pts.is_empty() {
                self.dirty |= self.chunks.remove(&c).is_some();
            } else {
                self.chunks.insert(c, pts);
                self.dirty = true;
            }
        }
    }

    /// Every point, if any changed since the last call (else `None`).
    pub fn changed(&mut self) -> Option<Vec<Point>> {
        std::mem::take(&mut self.dirty).then(|| self.chunks.values().flatten().copied().collect())
    }

    pub fn bytes(&self) -> u64 {
        self.chunks.values().map(|v| v.len() as u64 * 16).sum()
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
}
