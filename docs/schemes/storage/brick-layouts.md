+++
title = "Brick-major and Morton id layouts"
category = "storage"
status = "measured"
components = ["storage/brickmajor", "storage/morton"]
related = ["bricks", "prefix"]
origin = "project"
sources = []
+++

# Brick-major and Morton id layouts

## What it is

`bricks` with the u16 ids of a chunk in a different memory order, so that the voxels a ray
touches share cache lines: `brickmajor` stores each 4³ brick's 64 ids contiguously (128
bytes); `morton` stores all ids in Z-order over the chunk (bricks contiguous, and neighbouring
bricks close).

## How it works

Same queries and occupancy as `bricks`; only the index of a cell's id changes. One generic
implementation is monomorphised per order.

## In this project

`components/src/storage/layout/` (shared), `brickmajor/` and `morton/`. CPU only (`cpu`,
`cpu-discover`).

## Measured

L1 misses halve, but cycles do not drop, so both run 3–4% behind `bricks`. CPU ray walking
here is not memory bound: hardware counters show ~80 cycles and ~220 instructions per step at
IPC 2.6, with L2 misses at most 3% of the time.
