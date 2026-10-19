+++
title = "64-tree: sparse 4×4×4-branching tree"
category = "storage"
status = "measured"
components = ["storage/tree64"]
related = ["svo-dag", "prefix", "bricks"]
origin = "literature"
sources = []
+++

# 64-tree: sparse 4×4×4-branching tree

## What it is

A sparse tree over 64³ regions in which every node has 4 × 4 × 4 = 64 children, described by
a 64-bit child mask. Three levels (64 → 16 → 4 → voxels) cover a region. It stores no dense
voxel pool, so it needs a fraction of the memory.

## How it works

- Every node is four words: the child mask (two u32), the offset of its first child, and a
  block id (non-zero for a uniform node).
- Children are stored contiguously and addressed by popcount: the child for bit `b` is at the
  rank of `b` in the mask.
- Uniform subtrees collapse into one node; leaves store only their solid voxels' ids (two u16
  per word).
- Queries descend from the region's root; levels 0 (voxels), 2 (4³ nodes) and 4 (16³
  nodes) are answered directly, and the region itself (6) through the top table.

## In this project

`components/src/storage/tree64/` (`mod.rs` documents the blob layout, `gpu.wgsl`). GPU only;
a *region* storage with its own top table (`S_TOP` = 6) and no voxel pool.

## Measured

About as fast as `bricks` + `refloor` on terrain, with about 15% of its VRAM (measured with
the standalone 64-tree renderer; the component is within 5% of that renderer's speed).
