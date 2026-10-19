+++
title = "Rasterising every face"
category = "visibility"
status = "measured"
components = ["visibility/faces"]
related = ["greedy", "naive", "cubes", "forward-raster", "visbuffer", "gbuffer", "quadtree"]
origin = "literature"
sources = []
+++

# Rasterising every face

## What it is

The classic approach: turn the chunks into faces (the geometry node), send every face to
the GPU's rasteriser, and let the depth buffer decide what is visible. The cost scales with
the number of faces drawn, including hidden ones.

## How it works

Faces are meshed on the CPU, in parallel and incrementally per changed chunk, into 8-byte
instances. They are drawn with vertex pulling and one `multi_draw_indirect`, after per-chunk
frustum culling and per-face-direction culling (a chunk's faces pointing away from the camera
are skipped as a group). UVs are computed per pixel. The draw node (`raster`, `visbuffer` or
`gbuffer`) decides how fragments become the image.

## In this project

`visibility = "faces"`, the strategy of `gpu-raster` and `cpu-raster`;
`backends/gpu/src/faces/mesh.rs` (the GPU drawer lane) and `backends/cpu/src/raster.rs` (the
CPU software rasteriser: homogeneous clipping, tiled multi-threaded edge functions into a
visibility buffer, then deferred exact shading in f64).

## Measured

- GPU: on menger, `greedy` quads with `merge = exposure` drew 16% fewer quads with 13% less
  GPU time. Overdraw makes lit forward shading expensive; `visbuffer` is 1.5–2.5× faster lit.
- CPU (`cpu-raster`): the closest match to the reference (64–99 dB), but its cost scales with
  faces (menger: 8 fps).
