+++
title = "Bricks: dense ids with 4³ occupancy bits"
category = "storage"
status = "measured"
components = ["storage/bricks"]
related = ["hier", "prefix", "brick-layouts", "u8", "bits", "refloor"]
origin = "literature"
sources = []
+++

# Bricks: dense ids with 4³ occupancy bits

## What it is

The plainest useful voxel storage: every cell of a 32³ chunk stores its block id (16 bits),
and every 4³ brick has one occupancy bit. An empty chunk is not stored at all. It is the
baseline every other storage is compared with.

## How it works

A walk asks the storage for the largest empty aligned cube around a cell (`s_probe`). The
answer comes from three levels: the chunk table (an absent or empty chunk is one empty 32³
cube), the brick bit (an empty 4³ brick), and finally the cell's id. Levels answered
directly: 1 and 4 (`S_LEVELS`). On the GPU, chunks live in slots of one shared pool, found
through a hash of the chunk position; the pool is also the source of block ids for most other
GPU storages, which add side data next to it. On the CPU a chunk is a 512-bit brick mask
plus 32³ u16 ids (64 KiB).

## In this project

`components/src/storage/bricks/` (`gpu.wgsl`, `cpu.rs`). The default storage of
`gpu-compute`, `cpu`, `gpu-raster`, `gpu-discover`, `cpu-discover` and `gpu-splat`, and the
default storage for the shadow rays of `cpu-discover`. Works with every traversal kernel.

## Measured

- GPU, terrain, with `refloor`: 0.86 ms unlit, 1.20 ms lit. `prefix` is 13% faster on terrain
  and 23% faster under edits; `hier` 11–19% faster.
- CPU, terrain, unlit: 17.4 fps, 1986 cycles and 19.4 steps per pixel (`hier`: 19.6 fps,
  15.6 steps).
