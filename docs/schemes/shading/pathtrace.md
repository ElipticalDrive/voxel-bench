+++
title = "Diffuse path tracing"
category = "shading"
status = "measured"
components = ["shading/pathtrace"]
related = ["direct", "deferred-shading", "quadtree"]
origin = "literature"
sources = ["J. T. Kajiya, The rendering equation, SIGGRAPH 1986"]
+++

# Diffuse path tracing

## What it is

Global illumination by Monte Carlo: from each visible point, random diffuse bounces gather
indirect light, with the lights sampled directly at every vertex of the path.

## How it works

Next-event estimation for the sun and the point lights (shadow rays through `occluded`), a
constant-ambient sky, `bounces` cosine-weighted bounces traced through the pipeline's bounce
walk (`strace`), `spp` paths per pixel per frame, and progressive accumulation in a linear
buffer while the view is still (`accumulate`). `bounces = 0` reproduces `direct`'s lit image.
Unlit, it shows the texel like every shading.

## In this project

`components/src/shading/pathtrace/gpu.wgsl`. GPU only, on `gpu-compute`, `gpu-raster`,
`gpu-rt`, `gpu-discover`, `cpu-discover`, `gpu-splat` and `gpu-voxelspace`. The bounce walk is
a site of its own: `-r gpu-compute:bricks+refloor+pathtrace,bounce=tree64/sphere`.
Fragment-shaded path tracing (forward `raster` drawers) needs the lit depth prepass, so that
one fragment per pixel updates the accumulation.

## Measured

6–7% faster than the standalone path tracer it was made from.

## Open questions

- A denoiser, jittered primary rays, emissive blocks.
