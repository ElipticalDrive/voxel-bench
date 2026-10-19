+++
title = "Deferred shading over a common hit record"
category = "shading"
status = "measured"
components = []
related = ["visbuffer", "gbuffer", "raycast", "direct", "pathtrace", "splat"]
origin = "literature"
sources = ["M. Deering et al., The triangle processor and normal vector shader: a VLSI system for high performance graphics, SIGGRAPH 1988"]
+++

# Deferred shading over a common hit record

## What it is

Separating visibility from shading: the visibility stage writes one record per pixel, and a
single pass shades every pixel once. Here the record and the pass are shared by every
visibility strategy.

## How it works

The GPU hit record is 8 bytes: the hit distance and a packed id (bit 0 hit, bits 1–3 face,
4–19 block, 20–22 cell parity, 23–31 optional capabilities, such as a mesh triangle). The
common pass rebuilds the surface from the record and the view ray, and calls the shading
component. Strategies that write something else supply its decoding: `visbuffer` face ids,
`splat` point ids, `cpu-raster` quad ids. Where a strategy can do both, the shading node's
knob `placement` chooses: `inline` (shade where the hit is found) or `deferred`.

## In this project

`components/src/shading/deferred.wgsl` (GPU) and `deferred.rs` (CPU), the record in
`crates/mr-gpu/src/voxels.wgsl` (`pack_hit`) and `components/src/visibility/record.rs`.
`placement` exists on `gpu-compute`, `gpu-rt`, `cpu`, the voxelspace back ends and
`cpu-raster`. The record is also where a pipeline can be split over two GPUs: ray casting on
one, deferred shading on the other (`plans/two-gpus.toml`).

## Measured

- `cpu-raster`: inline (forward per fragment) is slower than deferred when lit (80 against
  61 ms: overdraw shaded in f64), with identical pixels.
- Two GPUs (RTX 5060 casting, Radeon iGPU shading, terrain): 16.6 MB of records per frame
  cross between them (copy 0.83 ms, upload 1.35 ms of CPU); shading a frame behind (`prev`)
  overlaps the GPUs: unlit 191 → 216 fps.
