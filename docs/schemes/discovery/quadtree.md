+++
title = "Discovery with a screen-space quadtree"
category = "discovery"
status = "measured"
components = ["visibility/quadtree"]
related = ["cluster", "coverage", "rasterisation", "painter-order", "forward-raster", "visbuffer"]
origin = "user"
sources = []
+++

# Discovery with a screen-space quadtree

## What it is

Find the set of visible block faces with as few rays as possible, keep that set across
frames, and rasterise only it. The project owner's idea: rays are spent on finding out what
is visible, and the rasteriser draws it, so cost follows the visible surface rather than the
pixel count or the whole geometry.

## How it works

- Rays are cast at the corners of screen tiles, on a lattice of pixel centres (shared corners
  are traced once per frame).
- Per tile: corners that disagree, or mix hits and misses, split the tile into up to four
  children; all misses make sky (if the tile is at most `sky_tile`); with the `plane`
  criterion, corners on one plane accept the tile, and every exposed face of that plane in its
  footprint joins the face set (`face`: all corners on one face).
- The face set is a GPU hash set kept for `keep_frames`; the drawer rasterises it.
- `merge = rows` merges runs of faces with the same block and the same coplanar neighbours
  into one quad (about half the quads); seam edges are grown by 1/16 pixel against T-junction
  cracks.
- Jitter (`jitter`) moves the lattice between frames; `audit` counts the pixels the set
  misses.

## In this project

`components/src/visibility/quadtree/` (`gpu.wgsl`, `cpu.rs`, `set.wgsl`) and the shared CPU
code in `components/src/visibility/discover/`. Back ends `gpu-discover` (GPU rays) and
`cpu-discover` (CPU rays, one frame ahead, the GPU drawing). Views: `tiles`, `rays`,
`churn`, `overdraw`, `quads`.

## Measured

- 0.05–0.34 rays per pixel; audit misses about 0–1 pixel per frame.
- Not yet faster than rasterising everything: the levels run one after another, and late
  rays are incoherent.
- `visbuffer` / `gbuffer` drawing is 2–4× faster than `raster` lit on terrain, a little
  slower unlit.
- `merge = rows` on terrain: 272k faces → 140k quads; lit 10% faster (tiny quads waste
  shading lanes), unlit slower (the merge pass costs ~0.35 ms), −2 dB from the remaining
  cracks at convex corners.
- `cpu-discover`: 0.25 rays per pixel finds all but ~15 faces on terrain (PSNR 49.8);
  pipelined, the frame of latency costs 0.1–0.3% of the pixels while moving; CPU discovery
  (25–60 ms) is the bottleneck.

## Open questions

- Seed discovery from the previous frame's set; one persistent kernel for all levels.
- 2D (rectangle) merging, and closing convex-corner cracks.
