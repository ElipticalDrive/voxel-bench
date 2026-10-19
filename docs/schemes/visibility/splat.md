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

Four passes: clear; a depth splat, in which each point `atomicMin`s its view depth over a
square footprint sized from its projected size (capped at `max_splat` pixels); an id splat,
in which the point whose depth won writes its id; and the resolve, the common deferred pass,
which ray-boxes the pixel against the winning block and shades it.

## In this project

`components/src/visibility/splat/` with geometry `points`; the `gpu-splat` back end
(`backends/gpu/src/splat.rs`). Lit-mode shadow rays walk the composed storage.

## Measured

Approximate: edges fatten, because a square footprint covers more than the block's
silhouette (PSNR ~20–27 dB).
