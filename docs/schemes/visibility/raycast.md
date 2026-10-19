+++
title = "Ray casting: one primary ray per pixel"
category = "visibility"
status = "measured"
components = ["visibility/raycast"]
related = ["refloor", "rayquery", "beam", "deferred-shading", "mesh-composite"]
origin = "literature"
sources = []
+++

# Ray casting: one primary ray per pixel

## What it is

Primary visibility by casting one ray through each pixel centre and taking the first block
it hits. The cost scales with pixels and with the steps each ray needs, not with the amount
of geometry.

## How it works

One GPU thread (or one packet lane on the CPU) per pixel calls the traversal kernel's
`ctrace` from the camera, optionally from a start distance given by a prepass. The hit is
shaded in place (`placement = inline`) or written as a hit record for the common deferred
pass (`placement = deferred`). Meshes join through the `composite` meshes node.

## In this project

`components/src/visibility/mod.rs` (the strategy), executed by the ray-casting lanes of
`backends/gpu/src/raycast/` (`gpu-compute`, `gpu-rt`) and `backends/cpu/src/raycast.rs`
(`cpu`). Any storage and kernel can be combined with it. Diagnostic views (`gpu-compute`,
`cpu`): `steps` shows the walk's iterations per pixel, `distance` the primary hit's distance.

## Measured

See the kernels and storages: the fastest combination on open scenes is `prefix` + `refloor`
(0.75 ms unlit on terrain).
