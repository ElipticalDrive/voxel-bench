use std::sync::Arc;

use glam::IVec3;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const CHUNK_BITS: u32 = 5;
pub const CHUNK_SIZE: i32 = 1 << CHUNK_BITS;
pub const CHUNK_VOLUME: usize = (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize;

/// Block type index into the [`crate::BlockRegistry`]. `0` is air (empty, transparent);
/// every other block is an opaque unit cube.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(transparent)]
pub struct BlockId(pub u16);

impl BlockId {
    pub const AIR: Self = Self(0);

    #[inline]
    pub fn is_air(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub fn is_solid(self) -> bool {
        self.0 != 0
    }
}

/// Identifies a chunk of the world at a level of detail.
///
/// A chunk always holds `32^3` cells. At `lod = L` a cell covers a `2^L` cube of blocks,
/// so the chunk covers blocks `[pos * 32 * 2^L, (pos + 1) * 32 * 2^L)` on every axis.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkKey {
    pub pos: IVec3,
    pub lod: u8,
}

impl ChunkKey {
    pub const fn lod0(pos: IVec3) -> Self {
        Self { pos, lod: 0 }
    }

    /// Chunk (at LOD 0) containing a block position.
    #[inline]
    pub fn of_block(block: IVec3) -> Self {
        Self::lod0(block >> CHUNK_BITS as i32)
    }

    /// Edge length in blocks.
    pub fn extent(self) -> i32 {
        CHUNK_SIZE << self.lod
    }

    /// Minimum block corner.
    pub fn origin(self) -> IVec3 {
        self.pos * self.extent()
    }
}

/// Position of a block inside its chunk.
#[inline]
pub fn local_of_block(block: IVec3) -> IVec3 {
    block & (CHUNK_SIZE - 1)
}

/// Linear index of a cell within a chunk: `x + 32 * (z + 32 * y)` (x fastest, then z, then y).
#[inline]
pub fn cell_index(local: IVec3) -> usize {
    (local.x | (local.z << CHUNK_BITS) | (local.y << (2 * CHUNK_BITS))) as usize
}

#[inline]
pub fn cell_of_index(i: usize) -> IVec3 {
    let i = i as i32;
    IVec3::new(i & 31, i >> 10, (i >> 5) & 31)
}

#[derive(Clone, PartialEq, Eq)]
pub struct DenseChunk(pub Box<[BlockId; CHUNK_VOLUME]>);

impl DenseChunk {
    pub fn filled(block: BlockId) -> Self {
        Self(vec![block; CHUNK_VOLUME].into_boxed_slice().try_into().unwrap())
    }

    #[inline]
    pub fn get(&self, local: IVec3) -> BlockId {
        self.0[cell_index(local)]
    }

    #[inline]
    pub fn set(&mut self, local: IVec3, b: BlockId) {
        self.0[cell_index(local)] = b;
    }

    pub fn as_u16(&self) -> &[u16] {
        bytemuck::cast_slice(&self.0[..])
    }

    /// Returns the block if every cell holds the same one.
    pub fn uniform(&self) -> Option<BlockId> {
        let first = self.0[0];
        self.0.iter().all(|&b| b == first).then_some(first)
    }
}

unsafe impl bytemuck::Zeroable for BlockId {}
unsafe impl bytemuck::Pod for BlockId {}

impl std::fmt::Debug for DenseChunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DenseChunk(..)")
    }
}

/// Contents of a chunk. Absent chunks are air by convention, so `Uniform(AIR)` is rarely sent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChunkData {
    Uniform(BlockId),
    Dense(Arc<DenseChunk>),
}

impl ChunkData {
    #[inline]
    pub fn get(&self, local: IVec3) -> BlockId {
        match self {
            ChunkData::Uniform(b) => *b,
            ChunkData::Dense(d) => d.get(local),
        }
    }

    pub fn set(&mut self, local: IVec3, b: BlockId) {
        match self {
            ChunkData::Uniform(u) if *u == b => {}
            ChunkData::Uniform(u) => {
                let mut d = DenseChunk::filled(*u);
                d.set(local, b);
                *self = ChunkData::Dense(Arc::new(d));
            }
            ChunkData::Dense(d) => Arc::make_mut(d).set(local, b),
        }
    }

    /// Collapses a dense chunk into a uniform one when possible.
    pub fn compact(self) -> Self {
        match &self {
            ChunkData::Dense(d) => d.uniform().map(ChunkData::Uniform).unwrap_or(self),
            _ => self,
        }
    }

    pub fn is_air(&self) -> bool {
        matches!(self, ChunkData::Uniform(BlockId::AIR))
    }
}

// Dense chunks travel as one raw little-endian byte blob rather than 32768 serde elements.
const _: () = assert!(cfg!(target_endian = "little"));

impl Serialize for DenseChunk {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(bytemuck::cast_slice(&*self.0))
    }
}

impl<'de> Deserialize<'de> for DenseChunk {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = DenseChunk;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{} bytes of chunk data", CHUNK_VOLUME * 2)
            }
            fn visit_bytes<E: serde::de::Error>(self, v: &[u8]) -> Result<DenseChunk, E> {
                if v.len() != CHUNK_VOLUME * 2 {
                    return Err(E::invalid_length(v.len(), &self));
                }
                let mut c = DenseChunk::filled(BlockId::AIR);
                bytemuck::cast_slice_mut::<BlockId, u8>(&mut *c.0).copy_from_slice(v);
                Ok(c)
            }
        }
        d.deserialize_bytes(V)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_roundtrip() {
        for i in [0, 1, 31, 32, 1023, 1024, 32767] {
            assert_eq!(cell_index(cell_of_index(i)), i);
        }
        assert_eq!(ChunkKey::of_block(IVec3::new(-1, 32, 31)).pos, IVec3::new(-1, 1, 0));
        assert_eq!(local_of_block(IVec3::new(-1, 32, 31)), IVec3::new(31, 0, 31));
    }
}
