//! Storage `tree64` (from gpu-tree64): each 64^3 region is a sparse three-level 64-tree
//! (64 -> 16 -> 4 -> voxels). Every node has a 4x4x4 child mask; children are stored
//! contiguously and addressed by popcount; uniform subtrees collapse to one node; leaves
//! store only their solid voxels' block ids. A region storage (no voxel pool): about 15% of
//! the pool's VRAM on terrain.
//!
//! Blob layout (u32 words, offsets relative to the blob): nodes of 4 words `[mask_lo,
//! mask_hi, child, block]`, the root at word 0. `block != 0`: a uniform node. Otherwise
//! `child` is the word offset of the first child node, or for nodes over voxels the u16
//! index (from the blob start) of the first leaf id. Leaf ids follow the nodes, two u16
//! per word.

use glam::IVec3;
use mr_core::BlockId;

use super::RegionBlob;

pub const GPU_WGSL: &str = include_str!("gpu.wgsl");

#[derive(Copy, Clone, Default)]
struct Node {
    mask: u64,
    child: u32,
    block: u32,
}

enum Built {
    Empty,
    Uniform(u16),
    /// A node, and whether its children are leaf ids.
    Node(Node, bool),
}

struct Builder<'a> {
    get: &'a dyn Fn(IVec3) -> BlockId,
    /// Non-root nodes (children before their parents), with whether they point at leaves.
    nodes: Vec<(Node, bool)>,
    ids: Vec<u16>,
}

/// Child bit `b` holds child cell `c` (0..4 per axis) with b = x | z << 2 | y << 4.
fn cell_of_bit(b: u32) -> IVec3 {
    IVec3::new((b & 3) as i32, ((b >> 4) & 3) as i32, ((b >> 2) & 3) as i32)
}

impl Builder<'_> {
    fn build(&mut self, origin: IVec3, size: i32) -> Built {
        let cs = size / 4;
        let mut mask = 0u64;
        if cs == 1 {
            let mut ids = vec![];
            for b in 0..64u32 {
                let id = (self.get)(origin + cell_of_bit(b));
                if id.is_solid() {
                    ids.push(id.0);
                    mask |= 1 << b;
                }
            }
            if ids.is_empty() {
                return Built::Empty;
            }
            if ids.len() == 64 && ids.iter().all(|&i| i == ids[0]) {
                return Built::Uniform(ids[0]);
            }
            let first = self.ids.len() as u32;
            self.ids.extend(ids);
            return Built::Node(Node { mask, child: first, block: 0 }, true);
        }
        let mut children = vec![];
        let mut uniform: Option<Option<u16>> = None; // Some(Some(b)) while all children are uniform b
        for b in 0..64u32 {
            let r = self.build(origin + cell_of_bit(b) * cs, cs);
            uniform = match (&r, uniform) {
                (Built::Uniform(u), None) => Some(Some(*u)),
                (Built::Uniform(u), Some(Some(v))) if *u == v => Some(Some(v)),
                _ => Some(None),
            };
            match r {
                Built::Empty => {}
                Built::Uniform(u) => {
                    mask |= 1 << b;
                    children.push((Node { block: u as u32, ..Default::default() }, false));
                }
                Built::Node(n, leaf) => {
                    mask |= 1 << b;
                    children.push((n, leaf));
                }
            }
        }
        if mask == 0 {
            return Built::Empty;
        }
        if let (u64::MAX, Some(Some(u))) = (mask, uniform) {
            return Built::Uniform(u);
        }
        let first = self.nodes.len() as u32;
        self.nodes.extend(children);
        Built::Node(Node { mask, child: first, block: 0 }, false)
    }
}

/// Builds one region's blob; `get` reads blocks at region-local coordinates (0..64).
pub fn build(get: &dyn Fn(IVec3) -> BlockId) -> RegionBlob {
    let mut b = Builder { get, nodes: vec![], ids: vec![] };
    let (root, leaf) = match b.build(IVec3::ZERO, 64) {
        Built::Empty => return RegionBlob::Empty,
        Built::Uniform(u) => return RegionBlob::Uniform(u),
        Built::Node(n, leaf) => (n, leaf),
    };
    // Root at word 0, node k (of `b.nodes`) at word 4 (k + 1), leaf ids after the nodes.
    let ids_word = 4 * (b.nodes.len() as u32 + 1);
    let fix = |n: Node, leaf: bool| -> [u32; 4] {
        let child = if n.block != 0 { 0 } else if leaf { 2 * ids_word + n.child } else { 4 * (n.child + 1) };
        [n.mask as u32, (n.mask >> 32) as u32, child, n.block]
    };
    let mut words = Vec::with_capacity(ids_word as usize + b.ids.len().div_ceil(2));
    words.extend(fix(root, leaf));
    for &(n, l) in &b.nodes {
        words.extend(fix(n, l));
    }
    words.extend(b.ids.chunks(2).map(|p| p[0] as u32 | (*p.get(1).unwrap_or(&0) as u32) << 16));
    RegionBlob::Words(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CPU mirror of the shader's `t64_probe` (block id only).
    fn query(w: &[u32], l: IVec3) -> u16 {
        let mut at = 0usize;
        let mut shift = 6;
        loop {
            let (mask, child, block) = (w[at] as u64 | (w[at + 1] as u64) << 32, w[at + 2], w[at + 3]);
            if block != 0 {
                return block as u16;
            }
            shift -= 2;
            let c: IVec3 = (l >> shift) & 3;
            let b = (c.x | (c.z << 2) | (c.y << 4)) as u32;
            if mask >> b & 1 == 0 {
                return 0;
            }
            let rank = (mask & ((1u64 << b) - 1)).count_ones();
            if shift == 0 {
                let idx = child + rank;
                return (w[idx as usize >> 1] >> ((idx & 1) * 16)) as u16;
            }
            at = (child + 4 * rank) as usize;
        }
    }

    #[test]
    fn region_tree_queries_match_source() {
        let get = |p: IVec3| {
            if p.y < 10 || (p.x + p.z) % 7 == 0 && p.y < 30 { BlockId(1 + (p.x % 3) as u16) } else { BlockId::AIR }
        };
        let RegionBlob::Words(w) = build(&get) else { panic!("expected a tree") };
        for y in 0..64 {
            for z in (0..64).step_by(3) {
                for x in 0..64 {
                    let p = IVec3::new(x, y, z);
                    assert_eq!(query(&w, p), get(p).0, "{p}");
                }
            }
        }
        assert!(matches!(build(&|_| BlockId::AIR), RegionBlob::Empty));
        assert!(matches!(build(&|_| BlockId(4)), RegionBlob::Uniform(4)));
    }
}
