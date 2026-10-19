+++
title = "Meshes in the rasteriser's depth buffer"
category = "meshes"
status = "built"
components = ["meshes/depth"]
related = ["mesh-composite", "rasterisation", "visbuffer"]
origin = "literature"
sources = []
+++

# Meshes in the rasteriser's depth buffer

## What it is

Triangle meshes drawn by rasterising pipelines into the same depth buffer as the voxel faces,
so the two occlude each other naturally.

## How it works

Forward drawers shade mesh fragments as they draw them; `visbuffer` and `gbuffer` write the
mesh triangle into the same id or record target, and the common deferred pass decodes and
shades it. Lit meshes are shadowed by voxels.

## In this project

`meshes = "depth"`, the default on `gpu-raster`; `crates/mr-gpu/src/meshes.rs` (`GpuMeshes`).

## Measured

Not measured on its own yet; scored against the reference in every `+meshes` pass
(`plans/meshes.toml`).
