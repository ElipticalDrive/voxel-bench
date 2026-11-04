//! Geometry `cubes` (from gpu-cubes): one cube instance per surface block.

use glam::IVec3;
use mr_core::{ChunkData, Face};
use mr_mesh::Mesh;

/// One instance per surface block (solid with at least one air neighbour), packed as
/// `[x | y << 5 | z << 10, block | slot << 16]`, all in `counts[0]`. Without `cull` every
/// solid block is emitted.
pub fn mesh(center: &ChunkData, neighbors: [Option<&ChunkData>; 6], slot: u16, cull: bool) -> Mesh {
    let mut quads = vec![];
    let mut counts = [0u32; 6];
    if center.is_air() || (cull && super::buried(center, &neighbors)) {
        return Mesh { quads, counts };
    }
    let uniform = matches!(center, ChunkData::Uniform(_));
    for z in 0..32 {
        for y in 0..32 {
            for x in 0..32 {
                let p = IVec3::new(x, y, z);
                let interior = (1..31).contains(&x) && (1..31).contains(&y) && (1..31).contains(&z);
                // Uniform solid chunks: only boundary blocks can touch air.
                if cull && uniform && interior {
                    continue;
                }
                let b = center.get(p);
                if !b.is_solid() || (cull && Face::ALL.iter().all(|f| !super::get(center, &neighbors, p + f.normal()).is_air())) {
                    continue;
                }
                quads.push([x as u32 | (y as u32) << 5 | (z as u32) << 10, b.0 as u32 | (slot as u32) << 16]);
            }
        }
    }
    counts[0] = quads.len() as u32;
    Mesh { quads, counts }
}
