//! The stored hit record on the CPU (PLAN step 12), laid out as the GPU's (`pack_hit` in
//! `mr::voxels`, so a record could cross devices): the low 32 bits are `t`, the distance
//! along the pixel's unit primary ray (f32 bits); the high 32 an id: 0 = no hit (sky), else
//! bit 0 set, the face in bits 1-3, the block in bits 4-19, the parity of the surface's cell
//! per axis in bits 20-22; bits 23-31 belong to optional capabilities and are 0 without them.

use glam::{IVec3, Vec3};
use mr_core::Face;

use crate::shading::direct::cpu::Surface;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct Record(pub u64);

impl Record {
    /// No hit: the pixel shows the sky.
    pub const SKY: Record = Record(0);

    /// The record of a hit at `t` on `face` of `block`, in `cell` (camera-relative).
    #[inline]
    pub fn hit(t: f32, block: u16, face: Face, cell: IVec3) -> Self {
        let parity = (cell.x & 1) as u32 | ((cell.y & 1) as u32) << 1 | ((cell.z & 1) as u32) << 2;
        let id = 1 | (face as u32) << 1 | (block as u32) << 4 | parity << 20;
        Self(t.to_bits() as u64 | (id as u64) << 32)
    }

    #[inline]
    pub fn is_hit(self) -> bool {
        self.0 >> 32 != 0
    }

    /// The surface on ray (`o`, unit `d`) as the strategy found it (the GPU's
    /// `record_surface`): the point at `t`, the face's normal, and the cell: the point's
    /// (`floor`) or its neighbour with the recorded parity (they differ on block edges).
    #[inline]
    pub fn surface(self, o: Vec3, d: Vec3) -> Surface {
        let id = (self.0 >> 32) as u32;
        let face = Face::ALL[((id >> 1) & 7) as usize];
        let p = o + d * f32::from_bits(self.0 as u32);
        let mut cell = p.floor();
        for k in 0..3 {
            if (cell[k] as i32 & 1) as u32 != (id >> (20 + k)) & 1 {
                cell[k] += if p[k] - cell[k] > 0.5 { 1.0 } else { -1.0 };
            }
        }
        Surface { p, n: face.normal().as_vec3(), local: p - cell, block: (id >> 4) as u16, face }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A record gives back its block, face, point and cell, also on block edges.
    #[test]
    fn round_trip() {
        let (o, d) = (Vec3::new(0.25, 0.5, 0.75), Vec3::new(1.0, 0.0, 0.0));
        // The ray enters the cell at x = 3 through its -X face, the plane x = 3.
        let r = Record::hit(2.75, 513, Face::NegX, IVec3::new(3, 0, 0));
        assert!(r.is_hit() && !Record::SKY.is_hit());
        let s = r.surface(o, d);
        assert_eq!((s.block, s.face), (513, Face::NegX));
        assert!((s.p - Vec3::new(3.0, 0.5, 0.75)).length() < 1e-6);
        assert!((s.local - Vec3::new(0.0, 0.5, 0.75)).length() < 1e-6, "{:?}", s.local);
        // The +X face of the cell at x = 2 on the same plane.
        let s = Record::hit(2.75, 1, Face::PosX, IVec3::new(2, 0, 0)).surface(o, d);
        assert!((s.local.x - 1.0).abs() < 1e-6, "{:?}", s.local);
        // On an edge (z = 2.0 exactly): the cell at z = 1 below it, or z = 2, as recorded.
        let o = Vec3::new(0.25, 0.5, 2.0);
        for (z, local) in [(1, 1.0), (2, 0.0), (-1, 1.0)] {
            let o = if z < 0 { Vec3::new(0.25, 0.5, 0.0) } else { o };
            let s = Record::hit(2.75, 1, Face::NegX, IVec3::new(3, 0, z)).surface(o, d);
            assert!((s.local.z - local).abs() < 1e-6, "{z}: {:?}", s.local);
        }
    }
}
