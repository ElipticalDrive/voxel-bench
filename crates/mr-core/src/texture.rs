use serde::{Deserialize, Serialize};

use crate::{BlockId, ChunkData, Face};

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
        self.blocks
            .iter()
            .position(|b| b.name == name)
            .map(|i| BlockId(i as u16))
    }

    /// The block's class (an id past the registry: opaque).
    #[inline]
    pub fn class(&self, block: BlockId) -> BlockClass {
        self.blocks
            .get(block.0 as usize)
            .map_or(BlockClass::Opaque, |b| b.class)
    }

    /// Whether `block` is solid and lets light through (cutout or translucent).
    #[inline]
    pub fn see_through(&self, block: BlockId) -> bool {
        block.is_solid() && self.class(block).see_through()
    }

    /// The lowest and highest id of a block that lets light through (none: `None`).
    fn see_through_ids(&self) -> Option<(u16, u16)> {
        let see = |b: &u16| self.blocks[*b as usize].class.see_through();
        let mut ids = 1..self.blocks.len() as u16;
        let lo = ids.find(see)?;
        Some((lo, ids.rev().find(see).unwrap_or(lo)))
    }

    /// Whether a chunk holds a block that lets light through: what makes an edited world ask
    /// for transparency (`mr_world::World::asks`, on the host; pipelines take the ask with
    /// each frame). A branch-free range test over the ids (vectorised), the exact test only
    /// for a chunk with ids in that range.
    pub fn chunk_see_through(&self, data: &ChunkData) -> bool {
        match data {
            ChunkData::Uniform(b) => self.see_through(*b),
            ChunkData::Dense(d) => self.see_through_ids().is_some_and(|(lo, hi)| {
                let span = hi - lo;
                d.0.chunks(64).any(|c| {
                    c.iter()
                        .fold(false, |a, b| a | (b.0.wrapping_sub(lo) <= span))
                }) && d.0.iter().any(|&b| self.see_through(b))
            }),
        }
    }

    /// Flat `[block][face] -> texture index` table, handy for GPU upload.
    pub fn face_table(&self) -> Vec<u32> {
        self.blocks
            .iter()
            .flat_map(|b| b.faces.map(u32::from))
            .collect()
    }

    /// All texels of all textures, each packed as little-endian RGBA8 in a `u32` (alpha 255).
    pub fn packed_texels(&self) -> Vec<u32> {
        self.textures
            .iter()
            .flat_map(|t| t.texels.iter().map(|&p| u32::from_le_bytes(p)))
            .collect()
    }

    /// For GPU upload after [`Self::packed_texels`]: every texture's coverage, four texels per
    /// `u32` (little-endian bytes, texel order), then one bit per block (bit `b & 31` of word
    /// `b >> 5`) set where the block lets light through.
    pub fn packed_see_through(&self) -> Vec<u32> {
        let mut out: Vec<u32> = self
            .textures
            .iter()
            .flat_map(|t| {
                (0..TEX_SIZE * TEX_SIZE / 4).map(move |w| {
                    u32::from_le_bytes(std::array::from_fn(|i| {
                        t.alpha.get(w * 4 + i).copied().unwrap_or(255)
                    }))
                })
            })
            .collect();
        let mut bits = vec![0u32; self.blocks.len().div_ceil(32)];
        for b in (0..self.blocks.len()).filter(|&b| self.see_through(BlockId(b as u16))) {
            bits[b >> 5] |= 1 << (b & 31);
        }
        out.extend(bits);
        out
    }
}
