//! Storage `svo_dag` (from gpu-svo-dag; Kampe et al. 2013): each 32^3 chunk is a binary
//! octree whose leaves are block ids, and interior nodes (8 child references) are
//! hash-consed in one global table shared by all chunks, so identical subtrees (stone
//! interiors, repeated structures) are stored once. A child reference with the high bit set
//! is an inline value (block id, 0 = air): uniform subtrees collapse to it. A *shared*
//! storage: the DAG lives in `aux` (8 words per node), chunk roots in the `regions` table.
//! Nodes are appended; the garbage edits and unloaded chunks leave is reclaimed by
//! [`Dag::collect`] (mark from the roots, compact in place, no chunk rebuilt) once the trees
//! replaced or dropped since the last mark could be half the DAG.

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

/// The global DAG on the host. Every child reference points to a lower index than its
/// parent's (children are merged first), which [`Dag::collect`] relies on.
#[derive(Default)]
pub struct Dag {
    pub nodes: Vec<[u32; 8]>,
    map: HashMap<[u32; 8], u32>,
    /// Nodes already on the GPU.
    pub uploaded: usize,
    /// Nodes of the chunk trees replaced or dropped since the last mark (their local sizes:
    /// an upper bound of the garbage, shared nodes may still be live).
    stale: usize,
    /// Compactions so far (a counter).
    pub collections: usize,
    /// Per chunk: its tree size with only chunk-local sharing (for the dedup ratio).
    local_nodes: HashMap<IVec3, u32>,
    /// Chunk roots (non-air chunks only).
    pub roots: HashMap<IVec3, u32>,
}

impl Dag {
    /// Rebuilds the `dirty` chunks, then collects garbage if due. Returns the chunks whose
    /// root changed or vanished (every chunk after a compaction, which also resets
    /// `uploaded`: every node is uploaded again).
    pub fn rebuild<'a>(&mut self, dirty: &mut HashSet<IVec3>, chunk: &(dyn Fn(IVec3) -> Option<&'a ChunkData> + Sync)) -> Vec<IVec3> {
        if dirty.is_empty() {
            return vec![];
        }
        let work: Vec<IVec3> = dirty.drain().collect();
        let built: Vec<(IVec3, Option<(Vec<[u32; 8]>, u32)>)> = work.par_iter().map(|&c| (c, chunk(c).map(build_chunk))).collect();
        for (c, tree) in built {
            self.roots.remove(&c);
            self.stale += self.local_nodes.remove(&c).unwrap_or(0) as usize;
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
        if self.nodes.len() > 1 << 16 && 2 * self.stale >= self.nodes.len() && self.collect() {
            let mut all: HashSet<IVec3> = work.into_iter().collect();
            all.extend(self.roots.keys().copied());
            return all.into_iter().collect();
        }
        work
    }

    /// Marks the nodes reachable from the roots; when at least half are garbage, compacts
    /// them in order (renumbering children and roots) and returns true. Below that the DAG
    /// is kept as it is: uploading every node again would cost more than the garbage.
    pub fn collect(&mut self) -> bool {
        let n = self.nodes.len();
        let mut live = vec![false; n];
        for &r in self.roots.values() {
            if r & LEAF == 0 {
                live[r as usize] = true;
            }
        }
        // Children have lower indexes, so one descending sweep marks every reachable node.
        for i in (0..n).rev() {
            if live[i] {
                for &c in &self.nodes[i] {
                    if c & LEAF == 0 {
                        debug_assert!((c as usize) < i, "a child after its parent");
                        live[c as usize] = true;
                    }
                }
            }
        }
        self.stale = 0;
        let count = live.iter().filter(|&&l| l).count();
        if 2 * count > n {
            return false;
        }
        let mut renumber = vec![u32::MAX; n];
        let mut kept = Vec::with_capacity(count);
        for (i, node) in self.nodes.iter().enumerate() {
            if live[i] {
                renumber[i] = kept.len() as u32;
                kept.push(node.map(|c| if c & LEAF != 0 { c } else { renumber[c as usize] }));
            }
        }
        self.map = kept.iter().enumerate().map(|(i, &k)| (k, i as u32)).collect();
        self.nodes = kept;
        for r in self.roots.values_mut() {
            if *r & LEAF == 0 {
                *r = renumber[*r as usize];
            }
        }
        self.uploaded = 0;
        self.collections += 1;
        true
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

    /// A chunk of noise (few shared subtrees: ~4.7k nodes), different per `seed`.
    fn noise(seed: u32) -> ChunkData {
        let mut c = ChunkData::Uniform(BlockId::AIR);
        for i in 0..32 * 32 * 32u32 {
            let h = (i ^ seed.wrapping_mul(0x9e37_79b9)).wrapping_mul(0x85eb_ca6b).rotate_left(13).wrapping_mul(0xc2b2_ae35);
            c.set(mr_core::chunk::cell_of_index(i as usize), BlockId(1 + (h >> 22) as u16));
        }
        c
    }

    /// Every block of every loaded chunk reads back from the DAG, and no root of an unloaded one stays.
    fn reads_back(dag: &Dag, world: &HashMap<IVec3, ChunkData>) {
        assert_eq!(dag.roots.len(), world.len());
        for (p, c) in world {
            for l in (0..32 * 32 * 32).step_by(5).map(mr_core::chunk::cell_of_index) {
                assert_eq!(query(&dag.nodes, dag.roots[p], l), c.get(l).0, "{p} {l}");
            }
        }
    }

    fn step(dag: &mut Dag, world: &HashMap<IVec3, ChunkData>, dirty: impl IntoIterator<Item = IVec3>) -> Vec<IVec3> {
        let mut dirty: HashSet<IVec3> = dirty.into_iter().collect();
        dag.rebuild(&mut dirty, &|p| world.get(&p))
    }

    /// Edits leave garbage; it is compacted away (the DAG stays near the margin), every chunk reads back after each compaction, and a compaction
    /// reports every chunk changed and uploads everything again.
    #[test]
    fn edits_are_collected() {
        let (a, b) = (IVec3::ZERO, IVec3::X);
        let mut world: HashMap<IVec3, ChunkData> = [(a, noise(0)), (b, noise(1))].into();
        let mut dag = Dag::default();
        step(&mut dag, &world, [a, b]);
        for i in 2..60 {
            world.insert(a, noise(i));
            dag.uploaded = dag.nodes.len();
            let changed = step(&mut dag, &world, [a]);
            if dag.uploaded == 0 {
                let mut changed = changed;
                changed.sort_by_key(|p| p.x);
                assert_eq!(changed, [a, b], "a compaction renumbers every root");
            }
            assert!(dag.nodes.len() < 1 << 17, "{} nodes", dag.nodes.len());
            reads_back(&dag, &world);
        }
        assert!(dag.collections >= 2, "{} collections", dag.collections);
    }

    /// A world that only grows has no garbage: never collected, past the margin.
    #[test]
    fn streaming_in_is_not_collected() {
        let mut world = HashMap::new();
        let mut dag = Dag::default();
        for i in 0..40 {
            let p = IVec3::new(i, 0, 0);
            world.insert(p, noise(100 + i as u32));
            step(&mut dag, &world, [p]);
        }
        assert!(dag.nodes.len() > 1 << 17, "past the margin: {}", dag.nodes.len());
        assert_eq!(dag.collections, 0);
        reads_back(&dag, &world);
    }

    /// A chunk that unloads loses its root, and its nodes are collected.
    #[test]
    fn unloaded_chunks_are_collected() {
        let mut world: HashMap<IVec3, ChunkData> = (0..30).map(|i| (IVec3::new(i, 0, 0), noise(200 + i as u32))).collect();
        let mut dag = Dag::default();
        step(&mut dag, &world, world.keys().copied().collect::<Vec<_>>());
        let full = dag.nodes.len();
        let gone: Vec<IVec3> = (1..30).map(|i| IVec3::new(i, 0, 0)).collect();
        for p in &gone {
            world.remove(p);
        }
        world.insert(IVec3::ZERO, noise(999));
        let changed = step(&mut dag, &world, gone.iter().copied().chain([IVec3::ZERO]));
        assert_eq!(changed.len(), 30);
        assert_eq!(dag.collections, 1);
        assert!(dag.nodes.len() * 10 < full, "{} of {full} nodes kept", dag.nodes.len());
        reads_back(&dag, &world);
    }
}
