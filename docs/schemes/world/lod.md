+++
title = "Level of detail by chunk rings"
category = "world"
status = "measured"
components = []
related = ["client", "refloor", "prefix", "clipmap"]
origin = "literature"
sources = []
+++

# Level of detail by chunk rings

## What it is

Coarser chunks farther away: level `L` has cells 2^L blocks wide, and each level covers the
view radius at twice the previous level's cell size, so a far view distance costs a fraction
of the memory.

## How it works

The server streams LOD chunks for `run.max_lod` levels (the client picks or derives them).
Textures stay tiled per block. The `refloor` walk over a pool storage reads the finest level
whose region contains the cell: cells, top cells and probe results grow by 2^lod with
distance.

## In this project

`run.max_lod`, or a client's `max_lod`. Walked by `refloor` over the pool storages on
`gpu-compute`; other storages, kernels and the beam prepass run without LOD (with a warning).

## Measured

- At the same far view distance (768 blocks): 136 MB of VRAM against 1037 MB at full
  resolution, and faster. The quality loss is concentrated in pixel-scale foliage in the
  distance.
- Against the full-resolution reference at the same reach (2026-10-19): 13–25 dB, 7–25% bad
  pixels, all of it beyond the full-resolution radius (the coarse cells differ pixel by pixel;
  `lattice`'s beams alias or vanish). A pick client's coarse rings are aligned to their own
  grid, so the reach overshoots by up to 2^max_lod − 1 chunks in some directions. Per-pixel
  PSNR is not the right measure of a lossy LOD.
- `prefix` with LOD is 22% faster than `bricks` with LOD.

## Open questions

- LOD for the rasterisers and the trees, and LOD that reflects edits (`derive` does).
