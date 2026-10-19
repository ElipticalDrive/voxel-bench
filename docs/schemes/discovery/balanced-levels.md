+++
title = "Self-balancing breadth-first discovery levels"
category = "discovery"
status = "measured"
components = []
related = ["quadtree"]
origin = "user"
sources = []
+++

# Self-balancing breadth-first discovery levels

## What it is

With N execution units, split the screen into N rectangles; each unit subdivides its cells
and decides which need descent. When all have finished a level, their buffers of cells
needing descent are joined and redistributed evenly for the next level, so a detailed region
cannot leave other units idle. Optionally in Morton order, to keep neighbouring rays together.

## How it works

The GPU quadtree is level-synchronous already: a global queue of tiles to split, rebalanced
across all threads at each level. Knob `morton` enumerates level 0 along a Z-curve, and
appends preserve order (a workgroup prefix sum instead of a local atomic), so deep levels
inherit the locality. Knob `inline_fp`: footprints of up to 8 cells are expanded by the
classifying thread instead of a 64-thread workgroup each.

## In this project

`gpu-discover` knobs `morton` (off by default) and `inline_fp` (8). On the CPU it would map to
rayon with a Morton-sorted work vector per level.

## Measured

- Morton order: no gain (GPU time unchanged within noise). The order-preserving appends were
  kept because they make the face list's order deterministic.
- Why: the passes, not the order. Rays cost 1.2–2.9 ns each per level, against 0.45 ns per
  pixel for plain ray casting with shading; each level pass lasts as long as its slowest ray
  plus the gap between dependent passes, roughly 0.6 ms of a 1.9 ms frame.
- What paid: inline small footprints (level-2 jobs 38k → 4.6k, classify −31%, edits
  1.72 → 1.35 ms, identical images). At 32 cells single threads diverge, so 8 is the default.

## Open questions

- A persistent kernel with work stealing, which removes the barriers between levels.
