+++
title = "Point splatting in compute"
category = "visibility"
status = "measured"
components = ["visibility/splat"]
related = ["points", "deferred-shading"]
origin = "literature"
sources = ["M. Schütz, B. Kerbl, M. Wimmer, Rendering point clouds with compute shaders and vertex order optimization, EGSR 2021"]
+++

# Point splatting in compute

## What it is

Software rasterisation of points: every block with an exposed face is one point, projected
by a compute shader and written with atomics instead of the hardware rasteriser.

## How it works

Passes: clear; a depth splat, in which each point `atomicMin`s, over the screen rectangle of
its block's corners, the view depth where each pixel's ray enters the block (a point under
2 px also covers the pixels it misses, at its centre's depth, so distant points do not fall
between pixel centres); an id splat, in which the point whose depth won writes its id; and
the resolve, the common deferred pass, which ray-boxes the pixel against the winning block
and shades it. One thread splats a point up to `max_splat` px on a side (default 16); larger
points go to a list, and a workgroup per point splats each of them (both splats).

## In this project

`components/src/visibility/splat/` with geometry `points`; the `gpu-splat` back end
(`backends/gpu/src/splat.rs`). Lit-mode shadow rays walk the composed storage.

## Measured

Exact except for points under 2 px: 39–79 dB on every plan but `far` (31–33 dB, distant
foliage), at most 0.2% bad pixels (2026-10-19). `max_splat` 16 is the fastest or close on
most worlds (8 costs up to 40% more on `city`, `gyroid`, `checker`; 64 more on `lattice`).

The first version gave each point a square footprint at its centre's depth, capped at
8 px: near surfaces fell apart into dots with the far side showing through (10–26 dB, 72%
bad pixels on `city`).
