use glam::{IVec3, Vec2, Vec3};
use serde::{Deserialize, Serialize};

/// The six faces of a unit cube, named by their outward normal.
/// The discriminant is the index used in [`crate::BlockType::faces`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Face {
    PosX = 0,
    NegX = 1,
    PosY = 2,
    NegY = 3,
    PosZ = 4,
    NegZ = 5,
}

impl Face {
    pub const ALL: [Face; 6] = [Face::PosX, Face::NegX, Face::PosY, Face::NegY, Face::PosZ, Face::NegZ];

    pub fn normal(self) -> IVec3 {
        match self {
            Face::PosX => IVec3::X,
            Face::NegX => IVec3::NEG_X,
            Face::PosY => IVec3::Y,
            Face::NegY => IVec3::NEG_Y,
            Face::PosZ => IVec3::Z,
            Face::NegZ => IVec3::NEG_Z,
        }
    }

    /// Face of a block that a ray entering along `axis` with direction sign `step` hits.
    #[inline]
    pub fn entered(axis: usize, step: i32) -> Face {
        match (axis, step > 0) {
            (0, true) => Face::NegX,
            (0, false) => Face::PosX,
            (1, true) => Face::NegY,
            (1, false) => Face::PosY,
            (2, true) => Face::NegZ,
            _ => Face::PosZ,
        }
    }

    pub fn from_index(i: usize) -> Face {
        Self::ALL[i]
    }

    /// Texture coordinates of a point on this face, given the point's position inside the
    /// block (`local` in `[0,1]^3`). `u` grows to the right and `v` grows downwards as seen
    /// by a viewer looking at the face from outside with +Y up; for the top face +X is
    /// right and +Z is down, for the bottom face +X is right and -Z is down.
    ///
    /// This is THE convention all renderers must follow for unlit images to match.
    #[inline]
    pub fn uv(self, local: Vec3) -> Vec2 {
        match self {
            Face::PosX => Vec2::new(1.0 - local.z, 1.0 - local.y),
            Face::NegX => Vec2::new(local.z, 1.0 - local.y),
            Face::PosZ => Vec2::new(local.x, 1.0 - local.y),
            Face::NegZ => Vec2::new(1.0 - local.x, 1.0 - local.y),
            Face::PosY => Vec2::new(local.x, local.z),
            Face::NegY => Vec2::new(local.x, 1.0 - local.z),
        }
    }
}
