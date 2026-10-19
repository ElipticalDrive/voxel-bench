+++
title = "Heightmap: top block per column"
category = "storage"
status = "measured"
components = ["storage/heightmap"]
related = ["voxel-space"]
origin = "literature"
sources = []
+++

# Heightmap: top block per column

## What it is

For each (x, z) column, the height and id of its top solid block, in a ring around the
camera. Everything below the top counts as solid: a 2.5D world with no overhangs, caves or
tree trunks.

## How it works

The ring is built on the CPU from the loaded chunks. Each entry packs the top `y` (offset by
32768; 0 = empty column) and the block id; per 32 × 32 tile it also keeps the largest top, so
walks can skip low tiles. GPU back ends upload the dirty rows. A ray walk over it
(`hm_trace`) serves shadow and bounce rays, so the storage still means "what the shading's
rays walk", as on every other back end.

## In this project

`components/src/storage/heightmap/` (`mod.rs`, `gpu.wgsl`). The storage of `gpu-voxelspace`
and `cpu-voxelspace`.

## Measured

27 MB of GPU memory with `gpu-voxelspace`. The image misses everything that is not a
column top (PSNR ~15–36 dB against the reference).
