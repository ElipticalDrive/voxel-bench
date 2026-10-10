//! Geometry `naive` (from gpu-raster-naive): one 1x1 quad per block face, no merging.

use glam::IVec3;
use mr_core::{ChunkData, Face};
use mr_mesh::Mesh;

/// One quad per block face (the `mr_mesh` encoding with w = h = 1, sorted by face). With
/// `cull`, faces whose neighbour is solid are skipped; otherwise all six faces of every
/// solid block are emitted.
pub fn mesh(center: &ChunkData, neighbors: [Option<&ChunkData>; 6], slot: u16, cull: bool) -> Mesh {
    let mut quads = vec![];
    let mut counts = [0u32; 6];
    if center.is_air() || (cull && super::buried(center, &neighbors)) {
        return Mesh { quads, counts };
    }
    let uniform = matches!(center, ChunkData::Uniform(_));
    for face in Face::ALL {
        let n = face.normal();
        let before = quads.len();
        for z in 0..32 {
            for y in 0..32 {
                for x in 0..32 {
                    let p = IVec3::new(x, y, z);
                    // Uniform solid chunks: only boundary blocks can have visible faces.
                    let q = p + n;
                    if cull && uniform && (0..32).contains(&q.x) && (0..32).contains(&q.y) && (0..32).contains(&q.z) {
                        continue;
                    }
                    let b = center.get(p);
                    if !b.is_solid() || (cull && !super::get(center, &neighbors, q).is_air()) {
                        continue;
                    }
                    let w0 = x as u32 | (y as u32) << 5 | (z as u32) << 10 | (face as u32) << 25;
                    quads.push([w0, b.0 as u32 | (slot as u32) << 16]);
                }
            }
        }
        counts[face as usize] = (quads.len() - before) as u32;
    }
    Mesh { quads, counts }
}
