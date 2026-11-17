use serde::{Deserialize, Serialize};

use crate::{BlockId, ChunkData, Face, WorldUpdate};

pub const TEX_SIZE: usize = 16;

/// A 16x16 RGBA8 sRGB texture. Row 0 is the top (`v = 0`). The alpha of `texels` is always
/// 255, so a pipeline that draws every block opaque reads them as before; a see-through
/// block's coverage is `alpha`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Texture {
    pub name: String,
    pub texels: Vec<[u8; 4]>,
    /// Coverage per texel (same order as `texels`; 255 = opaque, 0 = a hole); empty = all 255.
    #[serde(default)]
    pub alpha: Vec<u8>,
}

impl Texture {
    #[inline]
    pub fn texel(&self, x: usize, y: usize) -> [u8; 4] {
        self.texels[y * TEX_SIZE + x]
    }

    /// The coverage of a texel (255 when the texture has none).
    #[inline]
    pub fn alpha(&self, x: usize, y: usize) -> u8 {
        self.alpha.get(y * TEX_SIZE + x).copied().unwrap_or(255)
    }
}

/// How a block lets light through.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockClass {
    /// A solid cube: every texel covers (what every block was before see-through ones).
    #[default]
    Opaque,
    /// Texels either cover or are holes (alpha 0 or 255): needs no ordering.
    Cutout,
    /// Texels partly cover (any alpha): composited front to back.
    Translucent,
}

impl BlockClass {
    #[inline]
    pub fn see_through(self) -> bool {
        self != BlockClass::Opaque
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockType {
    pub name: String,
    /// Texture index per face, indexed by `Face as usize`.
    pub faces: [u16; 6],
    #[serde(default)]
    pub class: BlockClass,
}

/// All block types and their textures. `blocks[0]` is air and has no meaningful faces.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BlockRegistry {
    pub textures: Vec<Texture>,
    pub blocks: Vec<BlockType>,
}

impl BlockRegistry {
    #[inline]
    pub fn texture_of(&self, block: BlockId, face: Face) -> &Texture {
        &self.textures[self.blocks[block.0 as usize].faces[face as usize] as usize]
    }

    pub fn find(&self, name: &str) -> Option<BlockId> {
        self.blocks.iter().position(|b| b.name == name).map(|i| BlockId(i as u16))
    }

    /// The block's class (an id past the registry: opaque).
    #[inline]
    pub fn class(&self, block: BlockId) -> BlockClass {
        self.blocks.get(block.0 as usize).map_or(BlockClass::Opaque, |b| b.class)
    }

    /// Whether `block` is solid and lets light through (cutout or translucent).
    #[inline]
    pub fn see_through(&self, block: BlockId) -> bool {
        block.is_solid() && self.class(block).see_through()
    }

    /// Whether any block of the registry lets light through.
    pub fn any_see_through(&self) -> bool {
        self.blocks.iter().skip(1).any(|b| b.class.see_through())
    }

    /// Whether a chunk holds a block that lets light through: what makes a world ask for
    /// transparency.
    pub fn chunk_see_through(&self, data: &ChunkData) -> bool {
        match data {
            ChunkData::Uniform(b) => self.see_through(*b),
            ChunkData::Dense(d) => self.any_see_through() && d.0.iter().any(|&b| self.see_through(b)),
        }
    }

    /// Whether an update brings a block that lets light through (a chunk or an edit).
    pub fn update_see_through(&self, u: &WorldUpdate) -> bool {
        match u {
            WorldUpdate::ChunkLoad { data, .. } => self.chunk_see_through(data),
            WorldUpdate::SetBlocks(edits) => edits.iter().any(|e| self.see_through(e.block)),
            _ => false,
        }
    }

    /// Flat `[block][face] -> texture index` table, handy for GPU upload.
    pub fn face_table(&self) -> Vec<u32> {
        self.blocks.iter().flat_map(|b| b.faces.map(u32::from)).collect()
    }

    /// All texels of all textures, each packed as little-endian RGBA8 in a `u32` (alpha 255).
    pub fn packed_texels(&self) -> Vec<u32> {
        self.textures.iter().flat_map(|t| t.texels.iter().map(|&p| u32::from_le_bytes(p))).collect()
    }
}
