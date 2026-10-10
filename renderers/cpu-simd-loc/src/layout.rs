//! Block-id layouts inside a 32^3 chunk. All map a local cell (0..32 per axis) to an index
//! in 0..32768; they differ only in which cells share a cache line.

use std::sync::Arc;

use glam::IVec3;
use mr_core::chunk::{cell_index, cell_of_index};
use mr_core::{BlockId, CHUNK_VOLUME, DenseChunk};

/// `mr-core`'s order: x fastest, then z, then y (a 4^3 brick spans 16 lines over 4 KiB).
pub const LINEAR: u8 = 0;
/// Brick-major: the 512 bricks in the same x/z/y order, each brick's 64 ids contiguous
/// (128 B = 2 cache lines), x fastest inside the brick.
pub const BRICK: u8 = 1;
/// Full Morton (Z-order) over the chunk; also brick-contiguous (low 6 bits = 4^3 brick),
/// and neighbouring bricks are close too.
pub const MORTON: u8 = 2;

pub fn parse(s: &str) -> Option<u8> {
    match s {
        "linear" => Some(LINEAR),
        "brick" => Some(BRICK),
        "morton" => Some(MORTON),
        _ => None,
    }
}

/// Spreads 5 bits to every third bit.
const fn spread5(v: u32) -> u16 {
    let mut r = 0u32;
    let mut i = 0;
    while i < 5 {
        r |= ((v >> i) & 1) << (3 * i);
        i += 1;
    }
    r as u16
}

const MORTON_TABLE: [u16; 32] = {
    let mut t = [0u16; 32];
    let mut i = 0;
    while i < 32 {
        t[i] = spread5(i as u32);
        i += 1;
    }
    t
};

#[inline(always)]
pub fn index<const L: u8>(l: IVec3) -> usize {
    match L {
        BRICK => {
            let b: IVec3 = l >> 2;
            let bi = b.x | (b.z << 3) | (b.y << 6);
            let inner = (l.x & 3) | ((l.z & 3) << 2) | ((l.y & 3) << 4);
            ((bi << 6) | inner) as usize
        }
        MORTON => (MORTON_TABLE[(l.x & 31) as usize] | (MORTON_TABLE[(l.z & 31) as usize] << 1) | (MORTON_TABLE[(l.y & 31) as usize] << 2)) as usize,
        _ => cell_index(l),
    }
}

pub fn index_dyn(layout: u8, l: IVec3) -> usize {
    match layout {
        BRICK => index::<BRICK>(l),
        MORTON => index::<MORTON>(l),
        _ => index::<LINEAR>(l),
    }
}

/// Block ids of one chunk in some layout. Same shape (and indirection) as `DenseChunk`.
pub struct Ids(pub Box<[u16; CHUNK_VOLUME]>);

fn encode_as<const L: u8>(d: &DenseChunk) -> Ids {
    let mut out: Box<[u16; CHUNK_VOLUME]> = vec![0u16; CHUNK_VOLUME].into_boxed_slice().try_into().unwrap();
    // Walk in source order (x fastest, then z, then y) without re-deriving the cell.
    let mut i = 0;
    for y in 0..32 {
        for z in 0..32 {
            for x in 0..32 {
                out[index::<L>(IVec3::new(x, y, z))] = d.0[i].0;
                i += 1;
            }
        }
    }
    Ids(out)
}

/// Converts a chunk to `layout`. For `LINEAR` the buffer is reused when not shared.
pub fn encode_owned(d: Arc<DenseChunk>, layout: u8) -> Ids {
    match layout {
        BRICK => encode_as::<BRICK>(&d),
        MORTON => encode_as::<MORTON>(&d),
        _ => match Arc::try_unwrap(d) {
            Ok(d) => {
                // SAFETY: BlockId is repr(transparent) over u16, so the arrays have the same layout.
                Ids(unsafe { Box::from_raw(Box::into_raw(d.0) as *mut [u16; CHUNK_VOLUME]) })
            }
            Err(d) => Ids(Box::new(*d.as_u16().first_chunk::<CHUNK_VOLUME>().unwrap())),
        },
    }
}

pub fn encode(d: &DenseChunk, layout: u8) -> Ids {
    match layout {
        BRICK => encode_as::<BRICK>(d),
        MORTON => encode_as::<MORTON>(d),
        _ => encode_as::<LINEAR>(d),
    }
}

pub fn decode(ids: &Ids, layout: u8) -> DenseChunk {
    let mut d = DenseChunk::filled(BlockId(0));
    for (i, b) in d.0.iter_mut().enumerate() {
        *b = BlockId(ids.0[index_dyn(layout, cell_of_index(i))]);
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_are_bijections() {
        for layout in [LINEAR, BRICK, MORTON] {
            let mut seen = vec![false; CHUNK_VOLUME];
            for i in 0..CHUNK_VOLUME {
                let j = index_dyn(layout, cell_of_index(i));
                assert!(j < CHUNK_VOLUME && !seen[j], "layout {layout}: collision at {i}");
                seen[j] = true;
            }
        }
    }

    #[test]
    fn bricks_are_contiguous() {
        for layout in [BRICK, MORTON] {
            for i in 0..CHUNK_VOLUME {
                let l = cell_of_index(i);
                assert_eq!(index_dyn(layout, l) >> 6, index_dyn(layout, l & !3) >> 6);
            }
        }
    }

    #[test]
    fn layouts_return_identical_blocks() {
        let mut seed = 0x1234_5678_9abc_def0u64;
        let mut rnd = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..4 {
            let mut d = DenseChunk::filled(BlockId(0));
            for b in d.0.iter_mut() {
                *b = BlockId((rnd() % 7) as u16);
            }
            for layout in [LINEAR, BRICK, MORTON] {
                let ids = encode(&d, layout);
                for i in 0..CHUNK_VOLUME {
                    let l = cell_of_index(i);
                    assert_eq!(ids.0[index_dyn(layout, l)], d.0[cell_index(l)].0);
                }
                assert!(decode(&ids, layout) == d);
                let owned = encode_owned(Arc::new(d.clone()), layout);
                assert!(owned.0 == ids.0);
            }
        }
    }
}
