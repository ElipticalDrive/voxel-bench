+++
title = "Chebyshev sphere tracing"
category = "traversal"
status = "measured"
components = ["traversal/sphere"]
related = ["distance", "refloor"]
origin = "literature"
sources = ["J. C. Hart, Sphere tracing: a geometric method for the antialiased ray tracing of implicit surfaces, The Visual Computer 1996"]
+++

# Chebyshev sphere tracing

## What it is

Sphere tracing over a voxel distance field, with the L∞ metric: in a cell at distance `k`
from the nearest solid cell, the whole box of cells within `k − 1` is empty, so the ray leaves
it in one step.

## How it works

Each iteration reads `s_dist(cell)`, clips the empty box to the top cell, and moves the ray
to the box's exit. With the `distance` storage, `s_dist` is one load; any other storage
answers it through an adapter built from `s_probe` (correct, slower).

## In this project

`components/src/traversal/sphere/gpu.wgsl`. GPU only. For example as a shadow walk:
`-r gpu-compute:prefix+incremental,shadow=distance/sphere`.

## Measured

With `distance`: the cheapest traversal on terrain (0.9 ms against 1.3 ms for `bricks` +
`refloor`, earlier build), but twice as slow under heavy edits (see
[distance](../storage/distance.md)).
