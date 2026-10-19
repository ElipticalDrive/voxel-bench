+++
title = "Forward rasterisation with per-fragment ray–plane hits"
category = "draw"
status = "measured"
components = ["draw/raster"]
related = ["visbuffer", "gbuffer", "rasterisation", "quadtree", "painter-order"]
origin = "literature"
sources = []
+++

# Forward rasterisation with per-fragment ray–plane hits

## What it is

The face drawer's forward mode: faces are rasterised with depth testing, and every fragment
that passes is shaded at once.

## How it works

Each fragment intersects its pixel's ray with the face's plane (`fragment_hit`), which gives
an exact hit point and UV at any distance, rather than interpolating vertex attributes. With
`depth_prepass` (lit mode), depth is laid down first, so only the front face shades.
Discovery's drawers have knob `order` (`depth`, or `painter`: see
[painter-order](painter-order.md)). On `cpu-raster` the same draw node is an edge-function
rasteriser into a visibility buffer (knobs `tile_width`, `band_height`).

## In this project

`components/src/draw/raster/gpu.wgsl`, the shared face drawer `components/src/draw/`
(`faces.wgsl`, `drawer.rs`). The default draw of `gpu-raster`, `gpu-discover`,
`cpu-discover` and `cpu-raster`.

## Measured

Lit with lots of overdraw, `visbuffer` is 1.5–2.5× faster; fragment-shaded `pathtrace` needs
the depth prepass so that one fragment per pixel updates the accumulation.
