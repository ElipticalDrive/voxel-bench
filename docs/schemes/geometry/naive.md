+++
title = "Naive faces: one quad per exposed face"
category = "geometry"
status = "measured"
components = ["geometry/naive"]
related = ["greedy", "cubes", "rasterisation"]
origin = "literature"
sources = []
+++

# Naive faces: one quad per exposed face

## What it is

One 1 × 1 quad per block face whose neighbour is air, with no merging ("Minecraft-style").

## How it works

The same packed quad encoding as `greedy`, with width and height 1, sorted by face direction.
Knob `cull = false` emits all six faces of every solid block (for measuring culling).

## In this project

`components/src/geometry/naive/`. On `gpu-raster`, `gpu-rt` and `cpu-raster`.

## Measured

2.6–18× more faces than `greedy`, depending on the scene.
