+++
title = "Palette compression"
category = "storage"
status = "measured"
components = ["storage/palette"]
related = ["bricks", "u8", "bits"]
origin = "literature"
sources = []
+++

# Palette compression

## What it is

Each chunk keeps a palette of the block ids it contains, and stores per voxel an index into
it, packed at 1, 2, 4, 8 or 16 bits, the smallest width that fits the palette. Index 0 is
always air.

## How it works

The occupancy test is an index compare against 0; the id is a palette lookup at the hit. A
chunk with two block types needs one bit per voxel.

## In this project

`components/src/storage/palette/cpu.rs`. CPU only.

## Measured

7× smaller than `bricks` on terrain, with unchanged cycles per pixel.
