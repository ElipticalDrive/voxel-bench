//! Geometry components (raster back ends): turn a chunk into drawable faces on the CPU.
//!
//! Every geometry emits packed instances in the `mr_mesh` quad encoding,
//! `[x | y << 5 | z << 10 | (w-1) << 15 | (h-1) << 20 | face << 25, block | slot << 16]`,
//! with the face fields left zero where the WGSL derives them (cubes). Instances are grouped
//! per face in `counts` when `per_face` (the back end then culls face directions per chunk);
//! otherwise all are in `counts[0]`.
//!
//! GPU interface (WGSL), used by the back end's vertex shader and by visibility-buffer
//! shading:
//! * `G_VERTS`: vertices per instance (a 4-vertex strip, or a 36-vertex list);
//! * `g_face(q, vi) -> u32`: the face vertex `vi` belongs to;
//! * `g_size(q) -> vec2<f32>`: the quad's extent along the face's (u, v) axes;
//! * `g_corner(vi) -> vec2<f32>`: the vertex's corner of its face, (0|1, 0|1).

pub mod cubes;
pub mod naive;

use mr_core::ChunkData;
pub use mr_mesh::{Merge, Mesh};

/// Meshing knobs (each geometry reads the ones it has).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Knobs {
    /// greedy: which faces may merge.
    pub merge: Merge,
    /// naive / cubes: skip faces (blocks) hidden by solid neighbours.
    pub cull: bool,
}

impl Default for Knobs {
    fn default() -> Self {
        Self { merge: Merge::Block, cull: true }
    }
}

pub type MeshFn = fn(&ChunkData, [Option<&ChunkData>; 6], u16, Knobs) -> Mesh;

/// What the gpu-rt back end builds per chunk for a geometry.
#[derive(Copy, Clone)]
pub enum RtKind {
    /// Quads (the `mesh` encoding) as two triangles each.
    Triangles(MeshFn),
    /// One AABB per occupied 4^3 brick, searched by a DDA through the voxel pool.
    Boxes,
}

/// The gpu-rt implementation of a geometry: WGSL `g_rt_trace(o, d, max_t, shadow) -> Hit`
/// and what to build.
pub fn rt(name: &str) -> Option<(&'static str, RtKind)> {
    let quads = include_str!("greedy/rt.wgsl");
    Some(match name {
        "greedy" => (quads, RtKind::Triangles(|c, n, slot, k| mr_mesh::mesh_chunk_merge(c, n, slot, k.merge))),
        "naive" => (quads, RtKind::Triangles(|c, n, slot, k| naive::mesh(c, n, slot, k.cull))),
        "boxes" => (include_str!("boxes/rt.wgsl"), RtKind::Boxes),
        _ => return None,
    })
}

/// Implementation details a raster back end needs for a geometry.
pub struct GpuGeometry {
    pub wgsl: &'static str,
    /// Vertices per instance, drawn as a triangle strip (`strip`) or list.
    pub verts: u32,
    pub strip: bool,
    /// Instances are grouped by face (`counts[face]`), so face directions can be culled.
    pub per_face: bool,
    pub mesh: MeshFn,
}

const QUADS_WGSL: &str = include_str!("greedy/gpu.wgsl");

pub fn gpu(name: &str) -> Option<GpuGeometry> {
    Some(match name {
        "greedy" => GpuGeometry { wgsl: QUADS_WGSL, verts: 4, strip: true, per_face: true, mesh: |c, n, slot, k| mr_mesh::mesh_chunk_merge(c, n, slot, k.merge) },
        // One-block quads in the greedy encoding (w = h = 1): the same vertex code.
        "naive" => GpuGeometry { wgsl: QUADS_WGSL, verts: 4, strip: true, per_face: true, mesh: |c, n, slot, k| naive::mesh(c, n, slot, k.cull) },
        "cubes" => GpuGeometry { wgsl: include_str!("cubes/gpu.wgsl"), verts: 36, strip: false, per_face: false, mesh: |c, n, slot, k| cubes::mesh(c, n, slot, k.cull) },
        _ => return None,
    })
}

/// Block at `p` (chunk-local, may step one block into a neighbour; absent = air).
fn get(center: &ChunkData, neighbors: &[Option<&ChunkData>; 6], p: glam::IVec3) -> mr_core::BlockId {
    use mr_core::Face;
    let out = |v: i32| !(0..32).contains(&v);
    if !(out(p.x) || out(p.y) || out(p.z)) {
        return center.get(p);
    }
    let f = if p.x > 31 {
        Face::PosX
    } else if p.x < 0 {
        Face::NegX
    } else if p.y > 31 {
        Face::PosY
    } else if p.y < 0 {
        Face::NegY
    } else if p.z > 31 {
        Face::PosZ
    } else {
        Face::NegZ
    };
    match neighbors[f as usize] {
        Some(c) => c.get(p & 31),
        None => mr_core::BlockId::AIR,
    }
}

/// With culling, a uniform solid chunk buried in uniform solid neighbours has no faces.
fn buried(center: &ChunkData, neighbors: &[Option<&ChunkData>; 6]) -> bool {
    matches!(center, ChunkData::Uniform(_)) && neighbors.iter().all(|n| matches!(n, Some(ChunkData::Uniform(b)) if b.is_solid()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::IVec3;
    use mr_core::BlockId;

    fn total(m: &Mesh) -> u32 {
        m.counts.iter().sum()
    }

    /// Two touching blocks: naive emits the 10 exposed faces (12 without culling), cubes two
    /// instances, greedy merges the coplanar pairs.
    #[test]
    fn geometries_count_faces() {
        let mut c = ChunkData::Uniform(BlockId::AIR);
        c.set(IVec3::new(3, 4, 5), BlockId(1));
        c.set(IVec3::new(4, 4, 5), BlockId(1));
        let k = Knobs::default();
        let mesh = |name: &str, k| (gpu(name).unwrap().mesh)(&c, [None; 6], 7, k);
        assert_eq!(total(&mesh("naive", k)), 10);
        assert_eq!(total(&mesh("naive", Knobs { cull: false, ..k })), 12);
        assert_eq!(total(&mesh("cubes", k)), 2);
        assert_eq!(total(&mesh("greedy", k)), 6);
        let n = mesh("naive", k);
        assert_eq!(n.quads.len() as u32, total(&n));
        assert!(n.quads.iter().all(|q| q[1] == 1 | 7 << 16 && (q[0] >> 15) & 0x3ff == 0));
        // A buried solid chunk has nothing to draw.
        let solid = ChunkData::Uniform(BlockId(2));
        for g in ["greedy", "naive", "cubes"] {
            assert_eq!(total(&(gpu(g).unwrap().mesh)(&solid, [Some(&solid); 6], 0, k)), 0, "{g}");
        }
    }
}
