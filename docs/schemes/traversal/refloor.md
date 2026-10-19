+++
title = "Refloor: probe, step out, re-floor"
category = "traversal"
status = "measured"
components = ["traversal/refloor"]
related = ["incremental", "sphere", "rowrun", "bricks", "prefix", "lod", "reference-dda", "ray-packets"]
origin = "literature"
sources = ["J. Amanatides, A. Woo, A fast voxel traversal algorithm for ray tracing, Eurographics 1987"]
+++

# Refloor: probe, step out, re-floor

## What it is

A voxel ray walk over any storage: at each cell, ask the storage for the largest empty
aligned cube containing it, jump to where the ray leaves that cube, and recompute ("re-floor")
the cell from the new position. With unit cubes only, this is the classic grid DDA; larger
cubes skip empty space.

## How it works

Each iteration calls `s_probe(cell)`, which returns either a block id (a hit) or the size
of the empty cube; a top cell absent from the storage is one empty cube. The ray moves to its exit
from the cube, and the position is floored again to find the next cell. The cell containing the ray's origin is skipped, as in the
reference. With LOD streamed (a compile-time constant), cells, top cells and probe results grow
by 2^lod with distance from the camera. On the CPU the walk runs in packets of N rays in
lockstep.

## In this project

`components/src/traversal/refloor/` (`gpu.wgsl`, `cpu.rs`). The default kernel of every back
end that walks voxels, and the only one that walks LOD chunks. Works over every storage. Knob
on gpu-compute: `dispatch = "fragment"` runs the pixels in a fragment shader over a
full-screen triangle instead of compute.

## Measured

- GPU terrain, unlit / lit: 0.86 / 1.20 ms over `bricks`, 0.75 / 1.07 over `prefix`.
- `dispatch = "fragment"` runs at the same speed as compute.
- CPU: ~80 cycles and ~220 instructions per step at IPC 2.6; a non-unit step costs more than a
  unit step (floors on all axes), which is why `prefix`'s large steps do not pay on the CPU.
