+++
title = "Clipmap: a camera-centred toroidal volume"
category = "storage"
status = "measured"
components = ["storage/clipmap"]
related = ["bricks", "lod"]
origin = "literature"
sources = ["C. C. Tanner, C. J. Migdal, M. T. Jones, The clipmap: a virtual mipmap, SIGGRAPH 1998"]
+++

# Clipmap: a camera-centred toroidal volume

## What it is

A fixed-size volume of 3D textures around the camera, addressed by absolute position modulo
its size. As the camera moves, chunks entering the volume overwrite the slots of chunks
leaving it, so nothing is ever moved and no hash table is needed.

## How it works

Chunk `c` lives in slot `c mod slots`. Three textures: block ids (R8 or R16), brick occupancy
(R8), and slot tags (R32) saying which chunk owns each slot, so stale or foreign data reads
as empty. Every access is a `textureLoad`. Vertical slots are capped, because worlds are much
wider than tall.

## In this project

`components/src/storage/clipmap/` (`mod.rs`, `gpu.wgsl`). GPU only; a *texture* storage with
its own bindings and no voxel pool. Not yet multi-level: one resolution only.

## Measured

36–39% faster than the standalone clipmap renderer it was made from, because the slot wrap
now happens once per chunk instead of on every access.

## Open questions

- Power-of-two slot counts (the wrap as a mask) and a uniform-chunk flag.
- Coarser rings for distance (a real clipmap's levels).
