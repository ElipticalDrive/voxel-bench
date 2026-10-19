+++
title = "Incremental discovery: coverage-based hole detection"
category = "discovery"
status = "measured"
components = ["holes/coverage"]
related = ["quadtree", "painter-order"]
origin = "user"
sources = []
+++

# Incremental discovery: coverage-based hole detection

## What it is

Discovery that only looks where something may have changed. Last frame's face set is drawn at
this frame's camera first; rays are then cast only where that image can be wrong. A still
camera then costs almost nothing, and motion costs rays in proportion to the newly revealed
area. The project owner's idea (a temporally refined visible-face buffer).

## How it works

New faces can only appear next to an occlusion edge (within the parallax the camera's motion
causes), at the screen border, in edited or newly loaded regions, or in uncovered pixels. Per
frame:
1. Last frame's faces are rasterised at the current camera into face ids and depth; every
   face covering a pixel is marked seen. An unseen face in view is kept for `keep_unseen`
   frames (sub-pixel faces must not flicker); one out of view expires sooner.
2. A level-0 tile needs discovery if it has an uncovered pixel that was not sky last frame
   (the sky mask is kept by direction), if an occlusion edge in it can reveal more than
   `parallax_px` (accumulated since the tile was last discovered, so slow motion adds up), if
   an edited block or changed chunk projects onto it, or on its round-robin `refresh` turn.
3. Discovery runs on the flagged tiles; the final draw updates the sky mask.

With `events` (default), frames are classified: a static camera skips discovery and redraws
the set; a camera that only rotated skips seen-marking and the parallax test (`still_holes`:
`now` keeps the face-id raster, `late` drops it); a moved camera runs everything.

In the graph this is a loop: the drawer's faces iterate twice through hole detection, and the
sky is read from the previous frame (`prev`).

## In this project

`components/src/holes/coverage/gpu.wgsl`, `holes = "coverage"` on `gpu-discover`. View
`tiles` shows why each tile was discovered.

## Measured

Against full re-discovery (unlit, 1080p, audit on):

| scene | full ms | incremental ms | rays/px | PSNR |
|---|---|---|---|---|
| terrain, walking | 2.03 | 1.35 | 0.235 → 0.040 | 53.8 → 54.1 |
| terrain, flight | 2.03 | 2.30 | 0.235 → 0.176 | 53.6 → 53.6 |
| room, static camera (events) | 0.60 | 0.035 | | identical |

- Walking speed or slower: up to 83% fewer rays and 33% less GPU time, equal or better
  quality.
- Fast flight and orbits: 13–19% slower. The fixed passes (~0.4 ms) are not recouped, and the
  tiles that must be redone are the depth edges, where refinement is deepest.
- A static camera is 17× cheaper.
- Pure rotation still changes what a pixel grid samples: skipping hole detection then
  (`late`) leaves 1-frame holes (−8 dB).

## Open questions

- New geometry appearing in front of faces already in the set is caught for edits and
  streaming (their screen areas), otherwise only by the refresh.
- Fuse the marking and tile passes, read depth at quarter resolution, a one-sided parallax
  test (reveals lie on the background side).
