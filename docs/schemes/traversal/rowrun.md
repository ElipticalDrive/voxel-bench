+++
title = "Row runs: bit-scan jumps along x"
category = "traversal"
status = "measured"
components = ["traversal/rowrun"]
related = ["rows", "refloor"]
origin = "literature"
sources = []
+++

# Row runs: bit-scan jumps along x

## What it is

The `refloor` walk plus a jump: in an empty cell of an occupied region, when the ray runs
mostly along x, it crosses the empty run of its (y, z) row up to the next solid cell (or the
row's exit) in one step.

## How it works

The next solid cell in the row is found with `firstTrailingBit` / `firstLeadingBit` on the
row's occupancy mask (`s_row`). The jump visits the same cells, t and axis as the plain walk
would. With the `rows` storage `s_row` is one load; other storages answer it through the
generic adapter (32 voxel queries).

## In this project

`components/src/traversal/rowrun/gpu.wgsl`. GPU only.

## Measured

With `rows`: 5–15% slower than `bricks` + `refloor`.
