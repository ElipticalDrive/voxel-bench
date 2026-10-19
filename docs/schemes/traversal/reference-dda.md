+++
title = "The reference: an exact two-level DDA in f64"
category = "traversal"
status = "measured"
components = []
related = ["refloor", "direct"]
origin = "literature"
sources = ["J. Amanatides, A. Woo, A fast voxel traversal algorithm for ray tracing, Eurographics 1987"]
+++

# The reference: an exact two-level DDA in f64

## What it is

The renderer that defines ground truth: exactly one ray through each pixel centre, walked in
`f64` with a two-level DDA (chunks, then blocks), and the exact lighting model with a shadow
ray per light. Correctness and clarity come first.

## How it works

The walk steps chunk by chunk, and inside an occupied chunk block by block, with the classic
Amanatides–Woo DDA. Lighting is `mr_core::lighting`: ambient, sun and point lights with
windowed inverse-square falloff and hard shadows. It is multi-threaded so that references do
not take forever.

## In this project

`renderers/cpu-reference/`, the one standalone renderer, on purpose: it does not share code
with the components it judges. Comparisons, probes and the catalogue's PSNR floor are all
against it (`output.compare.reference`).

## Measured

The `cpu` back end is about 1.5× its speed. `cpu-raster`, which keeps the same f64 shading
model, is the closest match to it (64–99 dB).
