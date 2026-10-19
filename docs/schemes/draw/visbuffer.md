+++
title = "Visibility buffer"
category = "draw"
status = "measured"
components = ["draw/visbuffer"]
related = ["gbuffer", "forward-raster", "deferred-shading", "cubes"]
origin = "literature"
sources = ["C. A. Burns, W. A. Hunt, The visibility buffer: a cache-friendly approach to deferred shading, JCGT 2013"]
+++

# Visibility buffer

## What it is

The raster pass writes only a face id and depth per pixel; a compute pass then reconstructs
each pixel's hit from the id and shades it once, whatever the overdraw.

## How it works

The id is `instance << 3 | face`, plus one (0 = sky), 4 bytes per pixel. The face source
decodes it (`face_hit`) in the common deferred pass. Meshes write `VIS_MESH | triangle` into
the same target. The source's diagnostic views still apply, because the source decodes its
own ids.

## In this project

`components/src/draw/visbuffer/gpu.wgsl`. On `gpu-raster`, `gpu-discover` and `cpu-discover`.

## Measured

- 1.5–2.5× faster than forward `raster` when lit with lots of overdraw; halves lit time for
  `cubes`.
- On discovery (which draws almost only visible faces): 2–4× faster lit on terrain, a little
  slower unlit.
