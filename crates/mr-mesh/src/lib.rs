//! Greedy face mesher producing packed quads, shared by the rasterising and
//! hardware-ray-tracing renderers.
//!
//! Quad encoding (two u32):
//! * `w0 = x | y << 5 | z << 10 | (w-1) << 15 | (h-1) << 20 | face << 25`
//! * `w1 = block | chunk_slot << 16`
//!
//! `(x, y, z)` is the block (within its chunk) owning the face's minimum corner; the quad
//! spans `w` blocks along the face's u axis and `h` along its v axis (see [`FACE_AXES`]).
//!
//! With [`Merge::Exposure`], coplanar exposed faces merge regardless of block type and the
//! quad's block is [`LOOKUP_BLOCK`]: the renderer looks the block up per fragment.

use glam::IVec3;
use mr_core::{BlockId, ChunkData, Face};

/// `(normal axis, u axis, v axis)` per face, chosen so that `u x v` is the outward normal
/// (counter-clockwise front faces). Mirrored in `raster.wgsl`.
pub const FACE_AXES: [(usize, usize, usize); 6] = [(0, 1, 2), (0, 2, 1), (1, 2, 0), (1, 0, 2), (2, 0, 1), (2, 1, 0)];

/// Block id of quads whose blocks vary across the quad (see [`Merge::Exposure`]).
pub const LOOKUP_BLOCK: u16 = 0xffff;

/// Which faces greedy meshing may merge.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Merge {
    /// Only faces of the same block type (the quad carries its block id).
    #[default]
    Block,
    /// Any coplanar exposed faces; the quad's block is [`LOOKUP_BLOCK`].
    Exposure,
}

pub struct Mesh {
    /// Quads sorted by face.
    pub quads: Vec<[u32; 2]>,
    pub counts: [u32; 6],
}

struct View<'a> {
    center: &'a ChunkData,
    neighbors: [Option<&'a ChunkData>; 6],
}

impl View<'_> {
    #[inline]
    fn get(&self, p: IVec3) -> BlockId {
        let out = |v: i32| !(0..32).contains(&v);
        if !(out(p.x) || out(p.y) || out(p.z)) {
            return self.center.get(p);
        }
        let face = if p.x > 31 {
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
        match self.neighbors[face as usize] {
            Some(c) => c.get(p & 31),
            None => BlockId::AIR,
        }
    }
}

/// Meshes one chunk. `neighbors` are indexed by [`Face`] (absent = air).
pub fn mesh_chunk(center: &ChunkData, neighbors: [Option<&ChunkData>; 6], slot: u16) -> Mesh {
    mesh_chunk_merge(center, neighbors, slot, Merge::Block)
}

/// [`mesh_chunk`] with a choice of merge criterion.
pub fn mesh_chunk_merge(center: &ChunkData, neighbors: [Option<&ChunkData>; 6], slot: u16, merge: Merge) -> Mesh {
    let mut quads = vec![];
    let mut counts = [0u32; 6];
    if center.is_air() {
        return Mesh { quads, counts };
    }
    if let ChunkData::Uniform(_) = center {
        let buried = neighbors.iter().all(|n| matches!(n, Some(ChunkData::Uniform(b)) if b.is_solid()));
        if buried {
            return Mesh { quads, counts };
        }
    }
    let view = View { center, neighbors };
    let mut mask = [0u16; 32 * 32];
    for face in Face::ALL {
        let (a, ua, va) = FACE_AXES[face as usize];
        let n = face.normal();
        let before = quads.len();
        // A uniform-solid chunk only has faces on its boundary slice.
        let slices: Vec<i32> = if matches!(center, ChunkData::Uniform(_)) {
            vec![if n[a] > 0 { 31 } else { 0 }]
        } else {
            (0..32).collect()
        };
        for d in slices {
            let mut any = false;
            for v in 0..32 {
                for u in 0..32 {
                    let mut p = IVec3::ZERO;
                    p[a] = d;
                    p[ua] = u;
                    p[va] = v;
                    let b = view.get(p);
                    let m = if b.is_solid() && view.get(p + n).is_air() {
                        if merge == Merge::Exposure { LOOKUP_BLOCK } else { b.0 }
                    } else {
                        0
                    };
                    mask[(v * 32 + u) as usize] = m;
                    any |= m != 0;
                }
            }
            if !any {
                continue;
            }
            for v in 0..32usize {
                let mut u = 0usize;
                while u < 32 {
                    let b = mask[v * 32 + u];
                    if b == 0 {
                        u += 1;
                        continue;
                    }
                    let mut w = 1;
                    while u + w < 32 && mask[v * 32 + u + w] == b {
                        w += 1;
                    }
                    let mut h = 1;
                    'grow: while v + h < 32 {
                        for k in 0..w {
                            if mask[(v + h) * 32 + u + k] != b {
                                break 'grow;
                            }
                        }
                        h += 1;
                    }
                    for dv in 0..h {
                        mask[(v + dv) * 32 + u..(v + dv) * 32 + u + w].fill(0);
                    }
                    let mut p = IVec3::ZERO;
                    p[a] = d;
                    p[ua] = u as i32;
                    p[va] = v as i32;
                    let w0 = p.x as u32
                        | (p.y as u32) << 5
                        | (p.z as u32) << 10
                        | (w as u32 - 1) << 15
                        | (h as u32 - 1) << 20
                        | (face as u32) << 25;
                    quads.push([w0, b as u32 | (slot as u32) << 16]);
                    u += w;
                }
            }
        }
        counts[face as usize] = (quads.len() - before) as u32;
    }
    Mesh { quads, counts }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_block_has_six_faces() {
        let mut c = ChunkData::Uniform(BlockId::AIR);
        c.set(IVec3::new(3, 4, 5), BlockId(1));
        let m = mesh_chunk(&c, [None; 6], 7);
        assert_eq!(m.counts, [1; 6]);
        assert!(m.quads.iter().all(|q| q[1] == 1 | 7 << 16));
    }

    #[test]
    fn decode_roundtrip() {
        let mut c = ChunkData::Uniform(BlockId::AIR);
        c.set(IVec3::new(3, 4, 5), BlockId(9));
        let m = mesh_chunk(&c, [None; 6], 7);
        for q in &m.quads {
            let d = Quad::decode(*q);
            assert_eq!((d.pos, d.w, d.h, d.block, d.slot), (IVec3::new(3, 4, 5), 1, 1, BlockId(9), 7));
            let n = d.face.normal().as_vec3();
            let [c0, c1, c2, _] = d.corners();
            assert!((c1 - c0).cross(c2 - c0).normalize().dot(n) > 0.99, "{:?} winding", d.face);
        }
    }

    #[test]
    fn exposure_merge_spans_block_types() {
        // A 32x32 floor slab alternating two block types: by-block merging cannot join
        // neighbours; by-exposure merging makes one quad per side.
        let mut c = ChunkData::Uniform(BlockId::AIR);
        for x in 0..32 {
            for z in 0..32 {
                c.set(IVec3::new(x, 0, z), BlockId(1 + ((x + z) % 2) as u16));
            }
        }
        let by_block = mesh_chunk(&c, [None; 6], 0);
        let by_exposure = mesh_chunk_merge(&c, [None; 6], 0, Merge::Exposure);
        assert!(by_block.counts[Face::PosY as usize] > 500);
        assert_eq!(by_exposure.counts, [1; 6]);
        assert!(by_exposure.quads.iter().all(|q| Quad::decode(*q).block == BlockId(LOOKUP_BLOCK)));
    }

    #[test]
    fn solid_chunk_merges_to_one_quad_per_side() {
        let m = mesh_chunk(&ChunkData::Uniform(BlockId(2)), [None; 6], 0);
        assert_eq!(m.counts, [1; 6]);
        let buried = ChunkData::Uniform(BlockId(2));
        let m = mesh_chunk(&buried, [Some(&buried); 6], 0);
        assert_eq!(m.quads.len(), 0);
    }
}

/// A decoded quad.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Quad {
    /// Block (within the chunk) owning the face's minimum corner.
    pub pos: IVec3,
    pub w: u32,
    pub h: u32,
    pub face: Face,
    pub block: BlockId,
    pub slot: u16,
}

impl Quad {
    pub fn decode(q: [u32; 2]) -> Self {
        Self {
            pos: IVec3::new((q[0] & 31) as i32, ((q[0] >> 5) & 31) as i32, ((q[0] >> 10) & 31) as i32),
            w: ((q[0] >> 15) & 31) + 1,
            h: ((q[0] >> 20) & 31) + 1,
            face: Face::from_index(((q[0] >> 25) & 7) as usize),
            block: BlockId((q[1] & 0xffff) as u16),
            slot: (q[1] >> 16) as u16,
        }
    }

    /// The four corners in chunk-local block units, in strip order
    /// `(0,0), (w,0), (0,h), (w,h)` along the face's (u, v) axes; counter-clockwise seen
    /// from outside.
    pub fn corners(&self) -> [glam::Vec3; 4] {
        let (a, ua, va) = FACE_AXES[self.face as usize];
        let mut base = self.pos.as_vec3();
        if (self.face as usize).is_multiple_of(2) {
            base[a] += 1.0;
        }
        let mut du = glam::Vec3::ZERO;
        du[ua] = self.w as f32;
        let mut dv = glam::Vec3::ZERO;
        dv[va] = self.h as f32;
        [base, base + du, base + dv, base + du + dv]
    }
}

/// Meshes the chunk at `pos` using a lookup for neighbouring chunks.
pub fn mesh_at<'a>(pos: IVec3, get: impl Fn(IVec3) -> Option<&'a ChunkData>, slot: u16) -> Option<Mesh> {
    let center = get(pos)?;
    let neighbors = Face::ALL.map(|f| get(pos + f.normal()));
    Some(mesh_chunk(center, neighbors, slot))
}
