+++
title = "G-buffer of hit records"
category = "draw"
status = "measured"
components = ["draw/gbuffer"]
related = ["visbuffer", "deferred-shading"]
origin = "literature"
sources = ["T. Saito, T. Takahashi, Comprehensible rendering of 3-D shapes, SIGGRAPH 1990"]
+++

# G-buffer of hit records

## What it is

The raster pass writes the common hit record (distance, block, face, cell parity: 8 bytes per
pixel) instead of an id; the common deferred pass rebuilds the surface from it and shades
once per pixel.

## How it works

The record is the same one ray casting writes (`pack_hit`), so the deferred pass is shared
with every other strategy. A fatter buffer than `visbuffer` (8 bytes against 4), but no
refetch of the face.

## In this project

`components/src/draw/gbuffer/gpu.wgsl`. On `gpu-raster`, `gpu-discover` and `cpu-discover`.

## Measured

A little slower than `visbuffer`.
