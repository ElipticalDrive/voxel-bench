+++
title = "Distance field: Chebyshev distance per voxel"
category = "storage"
status = "measured"
components = ["storage/distance"]
related = ["sphere", "bricks", "refloor"]
origin = "literature"
sources = ["J. C. Hart, Sphere tracing: a geometric method for the antialiased ray tracing of implicit surfaces, The Visual Computer 1996"]
+++

# Distance field: Chebyshev distance per voxel

## What it is

Each cell stores its Chebyshev (L∞) distance to the nearest solid cell of its chunk. A walk
in a cell at distance `k` knows that the whole cube of radius `k − 1` around it is empty, and
can leave it in one step.

## How it works

Distances are u8, packed four per 32-bit word in dense order (`x + 32 z + 1024 y`); 0 means
solid. They are computed per chunk on the CPU when a chunk loads or changes, capped at 32,
with a separable pass (rows, then max-min passes along z and y). Block ids come
from the shared pool. The storage answers the optional `s_dist` query directly; its cube
queries are conservative (a cube is empty only when the distance at its first cell proves it,
exact at level 0, as the interface requires).

## In this project

`components/src/storage/distance/` (`mod.rs`, `gpu.wgsl`). GPU only. Fast with the `sphere`
kernel; any other kernel works through the interface's core queries.

## Measured

- With `sphere`: the cheapest traversal on terrain (0.9 ms against 1.3 ms for `bricks` +
  `refloor`, earlier build).
- Twice as slow under heavy edits: an edited chunk is walked cell by cell until its field has
  been recomputed.
- Slow to stream (2026-10-19, `city` and `islands` plans): the field build costs 23–30 ms of
  world update per frame on average (p95 80 ms) against about 1 ms for `bricks`, and the
  initial load takes 1–2.5 s against 0.1 s; frame rate drops to 32–41 fps although the walk
  itself takes about 1 ms. The z and y passes of `chebyshev_df` are O(32⁴) per chunk.

## Open questions

- Distances that cross chunk borders (today they stop at the chunk's edge).
- A linear-time field build (the z and y passes are quadratic per line), off the frame thread.
