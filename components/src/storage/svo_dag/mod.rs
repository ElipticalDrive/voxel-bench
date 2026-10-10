//! Storage `svo_dag` (from gpu-svo-dag; Kampe et al. 2013): each 32^3 chunk is a binary
//! octree whose leaves are block ids, and interior nodes (8 child references) are
//! hash-consed in one global table shared by all chunks, so identical subtrees (stone
//! interiors, repeated structures) are stored once. A child reference with the high bit set
//! is an inline value (block id, 0 = air): uniform subtrees collapse to it. A *shared*
//! storage: the DAG lives in `aux` (8 words per node), chunk roots in the `regions` table.
//! Nodes are append-only; garbage from edits is reclaimed by a full rebuild once the DAG
//! has doubled.

use std::collections::{HashMap, HashSet};

use glam::IVec3;
use mr_core::chunk::local_of_block;
use mr_core::{BlockId, ChunkData};
use rayon::prelude::*;

pub const GPU_WGSL: &str = include_str!("gpu.wgsl");

/// Inline child reference (block id); equal to `UNIFORM_BIT`, so a uniform root is already
/// a `s_top` uniform handle.
pub const LEAF: u32 = 0x8000_0000;

/// One chunk's octree with chunk-local hash-consing. Children precede their parent, so
/// merging into the global DAG in order needs a single remap pass.
struct LocalDag {
    nodes: Vec<[u32; 8]>,
    map: HashMap<[u32; 8], u32>,
}

impl LocalDag {
    fn build(&mut self, get: &dyn Fn(IVec3) -> BlockId, origin: IVec3, size: i32) -> u32 {
        if size == 1 {
            return LEAF | get(origin).0 as u32;
        }
        let h = size / 2;
        let ch: [u32; 8] = std::array::from_fn(|i| self.build(get, origin + IVec3::new((i & 1) as i32, ((i >> 1) & 1) as i32, (i >> 2) as i32) * h, h));
        if ch[0] & LEAF != 0 && ch.iter().all(|&c| c == ch[0]) {
            return ch[0];
        }
        if let Some(&i) = self.map.get(&ch) {
            return i;
        }
        let i = self.nodes.len() as u32;
        self.nodes.push(ch);
        self.map.insert(ch, i);
        i
    }
}

fn build_chunk(chunk: &ChunkData) -> (Vec<[u32; 8]>, u32) {
    let mut d = LocalDag { nodes: vec![], map: HashMap::new() };
    let root = d.build(&|p| chunk.get(local_of_block(p)), IVec3::ZERO, 32);
    (d.nodes, root)
}

/// The global DAG on the host.
#[derive(Default)]
pub struct Dag {
    pub nodes: Vec<[u32; 8]>,
    map: HashMap<[u32; 8], u32>,
    /// Nodes already on the GPU.
    pub uploaded: usize,
    /// DAG size right after the last full rebuild (garbage-collection trigger).
    base: usize,
    /// Per chunk: its tree size with only chunk-local sharing (for the dedup ratio).
    local_nodes: HashMap<IVec3, u32>,
    /// Chunk roots (non-air chunks only).
    pub roots: HashMap<IVec3, u32>,
}

impl Dag {
    /// Rebuilds the `dirty` chunks (all chunks after a garbage collection, which also
    /// resets `uploaded`). Returns the chunks whose root changed or vanished.
    pub fn rebuild<'a>(&mut self, dirty: &mut HashSet<IVec3>, chunk: &(dyn Fn(IVec3) -> Option<&'a ChunkData> + Sync)) -> Vec<IVec3> {
        if dirty.is_empty() {
            return vec![];
        }
        if self.base > 0 && self.nodes.len() > 2 * self.base + (1 << 16) {
            self.nodes.clear();
            self.map.clear();
            self.uploaded = 0;
            self.base = 0;
            dirty.extend(self.roots.keys().copied());
        }
        let work: Vec<IVec3> = dirty.drain().collect();
        let built: Vec<(IVec3, Option<(Vec<[u32; 8]>, u32)>)> = work.par_iter().map(|&c| (c, chunk(c).map(build_chunk))).collect();
        for (c, tree) in built {
            self.roots.remove(&c);
            self.local_nodes.remove(&c);
            let Some((nodes, root)) = tree else { continue };
            let remap = |r: u32, map: &[u32]| if r & LEAF != 0 { r } else { map[r as usize] };
            let mut map = Vec::with_capacity(nodes.len());
            for n in &nodes {
                let n = n.map(|r| remap(r, &map));
                let i = *self.map.entry(n).or_insert_with(|| {
                    self.nodes.push(n);
                    self.nodes.len() as u32 - 1
                });
                map.push(i);
            }
            let root = remap(root, &map);
            if root == LEAF {
                continue; // all air
            }
            self.local_nodes.insert(c, nodes.len() as u32);
            self.roots.insert(c, root);
        }
        if self.base == 0 {
            self.base = self.nodes.len();
        }
        work
    }

    /// Tree nodes with chunk-local sharing only, over DAG nodes.
    pub fn dedup_ratio(&self) -> f64 {
        self.local_nodes.values().map(|&n| n as f64).sum::<f64>() / self.nodes.len().max(1) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CPU mirror of the shader's `s_probe` (block id only).
    fn query(nodes: &[[u32; 8]], root: u32, l: IVec3) -> u16 {
        let (mut r, mut size) = (root, 32);
        while r & LEAF == 0 {
            size /= 2;
            let c = (l / size) & 1;
            r = nodes[r as usize][(c.x | (c.y << 1) | (c.z << 2)) as usize];
        }
        (r & 0xffff) as u16
    }

    /// Two chunks with the same content share every node; queries reproduce the blocks.
    #[test]
    fn dag_dedups_across_chunks_and_matches_source() {
        let mut c = ChunkData::Uniform(BlockId::AIR);
        for x in 0..32 {
            for z in 0..32 {
                for y in 0..(x + z) % 9 {
                    c.set(IVec3::new(x, y, z), BlockId(1 + (y % 2) as u16));
                }
            }
        }
        let (a, b) = (IVec3::ZERO, IVec3::X);
        let mut dag = Dag::default();
        let mut dirty: HashSet<IVec3> = [a].into();
        dag.rebuild(&mut dirty, &|p| (p == a || p == b).then_some(&c));
        let after_one = dag.nodes.len();
        let mut dirty: HashSet<IVec3> = [b].into();
        dag.rebuild(&mut dirty, &|p| (p == a || p == b).then_some(&c));
        assert_eq!(dag.nodes.len(), after_one, "the second identical chunk adds no nodes");
        for p in (0..32 * 32 * 32).step_by(7).map(mr_core::chunk::cell_of_index) {
            assert_eq!(query(&dag.nodes, dag.roots[&b], p), c.get(p).0, "{p}");
        }
    }
}
