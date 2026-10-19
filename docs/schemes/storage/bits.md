+++
title = "Bits: one occupancy bit per voxel"
category = "storage"
status = "measured"
components = ["storage/bits"]
related = ["bricks", "u8", "palette", "hier"]
origin = "project"
sources = []
+++

# Bits: one occupancy bit per voxel

## What it is

Occupancy only, one bit per voxel, brick-major (one u64 per 4³ brick, 4 KiB per chunk). Block
ids are read only at the hit, from the world.

## How it works

The brick test and the voxel test are the same word: a brick is empty when its u64 is zero,
a voxel is solid when its bit is set. `hier` is this plus 8³ region bits.

## In this project

`components/src/storage/bits/cpu.rs`. CPU only.

## Measured

16× smaller than `bricks`, with unchanged cycles per pixel.
