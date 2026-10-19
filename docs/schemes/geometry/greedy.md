+++
title = "Greedy meshing"
category = "geometry"
status = "measured"
components = ["geometry/greedy"]
related = ["naive", "rasterisation", "rayquery", "cluster"]
origin = "literature"
sources = ["M. Lysenko, Meshing in a Minecraft game, 0fps.net, 2012"]
+++

# Greedy meshing

## What it is

Merging coplanar exposed faces into rectangles, so that a flat wall is a few quads instead of
one quad per block face.

## How it works

Per chunk and face direction, the exposed faces of each plane are merged greedily into
rectangles. Each quad is packed into 8 bytes: the block owning its minimum corner, its extent
along the face's u and v axes, the face, the block id and the chunk's slot. Knob `merge`:
- `block` (default): only faces of the same block type merge;
- `exposure`: faces merge on exposure alone, whatever their block, and the fragment shader
  looks up the block of the cell under it in the voxel world (clamped to the quad, so
  silhouette fragments never read a neighbour). Raster only.

On `gpu-rt` each quad becomes two triangles of a chunk's BLAS.

## In this project

`components/src/geometry/greedy/` (the mesher is `crates/mr-mesh`), CPU-side, parallel and
incremental per changed chunk. The default geometry of `gpu-raster`, `gpu-rt` and
`cpu-raster`.

## Measured

`merge = exposure` against `block`:

| scene | quads | unlit GPU ms | lit GPU ms |
|---|---|---|---|
| terrain | 527k → 523k | 0.269 → 0.297 | 1.340 → 1.356 |
| menger | 1.93M → 1.62M (−16%) | 0.930 → 0.812 | 2.273 → 2.008 |
| random | 2.11M → 1.73M (−18%) | 1.239 → 1.188 | |

Image quality is identical, or up to +0.4 dB (fewer T-junction cracks). Terrain is mostly one
block type per region, so there is little to merge and the per-fragment lookup (a hash probe
and a pool read) costs ~10% unlit; `block` stays the default.

## Open questions

- A cheaper lookup: pass the voxel slot per chunk instead of hashing per fragment.
- Binary (bitwise) greedy meshing, which exposure merging makes possible, to cut CPU meshing
  time.
