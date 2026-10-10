use serde::{Deserialize, Serialize};

use crate::{BlockId, Face};

pub const TEX_SIZE: usize = 16;

/// A 16x16 RGBA8 sRGB texture. Row 0 is the top (`v = 0`). Alpha is always 255.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Texture {
    pub name: String,
    pub texels: Vec<[u8; 4]>,
}

impl Texture {
    #[inline]
    pub fn texel(&self, x: usize, y: usize) -> [u8; 4] {
        self.texels[y * TEX_SIZE + x]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockType {
    pub name: String,
    /// Texture index per face, indexed by `Face as usize`.
    pub faces: [u16; 6],
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

    /// Flat `[block][face] -> texture index` table, handy for GPU upload.
    pub fn face_table(&self) -> Vec<u32> {
        self.blocks.iter().flat_map(|b| b.faces.map(u32::from)).collect()
    }

    /// All texels of all textures, each packed as little-endian RGBA8 in a `u32`.
    pub fn packed_texels(&self) -> Vec<u32> {
        self.textures.iter().flat_map(|t| t.texels.iter().map(|&p| u32::from_le_bytes(p))).collect()
    }
}
