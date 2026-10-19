+++
title = "Antialiasing"
category = "post"
status = "idea"
components = []
related = ["tone-mapping", "hdr-output", "rasterisation", "raycast", "temporal-ray-start"]
origin = "user"
sources = []
+++

# Antialiasing

**Untested idea.**

## What it is

Smoothing the stair steps along voxel edges and the shimmer of distant, sub-pixel detail.
Every pipeline today takes one sample per pixel.

## How it works

The families differ by where they act, so each fits a different node:
- **More samples per pixel**: supersampling (render larger, filter down; works for every
  strategy), multisampling for the rasterisers (coverage at several points, shading once),
  several jittered rays per pixel for the ray casters (cheap where packets share a walk).
- **Post filters on the image**: edge detection and blending on the final colour (FXAA-,
  SMAA-like); a post node, so every pipeline gets them.
- **Temporal**: a jittered sample per frame accumulated with reprojection (TAA); reuses the
  previous frame's depth or hit record, which some pipelines already keep (temporal ray start,
  discovery's previous coverage).

Under the zero-cost rule each is a component compiled in only where a graph chooses it; a
pipeline without antialiasing keeps one sample and no extra pass. The comparison against the
reference has to account for it (an antialiased image differs from the one-sample reference
along every edge).

## In this project

Not built.

## Measured

Untested.
