+++
title = "Sparse voxel DAG"
category = "storage"
status = "measured"
components = ["storage/svo_dag"]
related = ["tree64", "prefix"]
origin = "literature"
sources = ["V. Kämpe, E. Sintorn, U. Assarsson, High resolution sparse voxel DAGs, ACM TOG (SIGGRAPH) 2013", "S. Laine, T. Karras, Efficient sparse voxel octrees, I3D 2010"]
+++

# Sparse voxel DAG

## What it is

A sparse voxel octree in which identical subtrees are stored once, which turns the tree into
a directed acyclic graph. Stone interiors and repeated structures collapse into shared nodes.

## How it works

- Each 32³ chunk is a binary octree whose leaves are block ids.
- Interior nodes (eight child references) are hash-consed in one global table shared by all
  chunks, so an identical subtree anywhere in the world is stored once.
- A child reference with the high bit set is an inline value (a block id, 0 = air), so
  uniform subtrees collapse into it.
- Every octree level exists, so occupancy queries answer levels 0–4.
- Nodes are append-only; garbage from edits is reclaimed by a full rebuild once the DAG has
  doubled in size.

## In this project

`components/src/storage/svo_dag/` (`mod.rs`, `gpu.wgsl`). GPU only; a *shared* storage: the
DAG in one buffer, the chunk roots in the region table.

## Measured

About 3× deduplication on terrain, and the smallest GPU footprint of all storages. Its
speed matched the standalone DAG renderer it was made from; it has not yet been compared
with the other storages in one run.

## Open questions

- Incremental garbage collection instead of rebuilding.
