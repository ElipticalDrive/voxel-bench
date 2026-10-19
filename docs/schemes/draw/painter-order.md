+++
title = "Painter's order by Manhattan distance"
category = "draw"
status = "measured"
components = []
related = ["forward-raster", "quadtree", "coverage"]
origin = "user"
sources = ["M. E. Newell, R. G. Newell, T. L. Sancha, A solution to the hidden surface problem, ACM Annual Conference 1972"]
+++

# Painter's order by Manhattan distance

## What it is

Drawing the face set back to front so that depth testing can be switched off. The project
owner's idea: discovery already produces roughly ordered faces, and only a local order is
needed.

## How it works

Along any ray, a voxel walk only moves away from the camera's cell on each axis, so a cell's
Manhattan distance from the camera cell strictly increases along the ray, and an occluder
always has the smaller distance. Drawing by decreasing distance is therefore an exact
painter's order for grid-aligned faces. The key is a small integer, so a GPU counting sort
does it: a histogram, a one-workgroup scan, a scatter. (Ordering octree children by the
camera's octant, the Morton index XOR the octant bits, is not exact in general.)

## In this project

Knob `order = painter` of discovery's drawers (`gpu-discover`); view `overdraw` shows
fragments per pixel.

## Measured

- Images identical to depth-tested drawing.
- Full discovery on terrain: overdraw 1.03 fragments per covered pixel. Unlit, the sort
  (~0.12 ms) saves nothing; lit, painter is ~3% faster, from skipping the depth prepass.
- With incremental discovery it backfires: kept unseen faces raise overdraw to ~5 on edits,
  and lit painter is 56% slower, because hidden faces are shaded with shadow rays. Depth order
  stays the default.
