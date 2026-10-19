+++
title = "Cube instances"
category = "geometry"
status = "measured"
components = ["geometry/cubes"]
related = ["naive", "visbuffer", "rasterisation"]
origin = "literature"
sources = []
+++

# Cube instances

## What it is

One instance per surface block (a solid block with at least one air neighbour), expanded on
the GPU into a 36-vertex cube; the hardware's back-face culling removes the hidden half.

## How it works

An instance is 8 bytes (position in the chunk, block, slot). The vertex shader derives the
face and corner from the vertex index. Knob `cull = false` instances every solid block.

## In this project

`components/src/geometry/cubes/`. On `gpu-raster` only.

## Measured

Heavy overdraw (faces between adjacent surface blocks are drawn and depth-tested): with
`visbuffer`, lit time halves against forward `raster`.
