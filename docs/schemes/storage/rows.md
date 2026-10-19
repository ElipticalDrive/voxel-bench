+++
title = "Rows: occupancy bitmasks along x"
category = "storage"
status = "measured"
components = ["storage/rows"]
related = ["rowrun", "bricks"]
origin = "literature"
sources = []
+++

# Rows: occupancy bitmasks along x

## What it is

`bricks` plus one 32-bit occupancy mask per row of a chunk: bit `x` of row `(y, z)` says
whether that cell is solid. A ray running along x can find the next solid cell in its row with
one bit-scan.

## How it works

32 × 32 u32 rows per chunk (row index `y * 32 + z`), stored next to the pool slot. The storage
answers the optional `s_row` query directly; other storages answer it through a generic adapter
(32 voxel queries).

## In this project

`components/src/storage/rows/` (`mod.rs`, `gpu.wgsl`). GPU only. Made for the `rowrun`
kernel.

## Measured

With `rowrun`: 5–15% slower than `bricks` + `refloor`.
