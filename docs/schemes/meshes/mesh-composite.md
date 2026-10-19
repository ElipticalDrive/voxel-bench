+++
title = "Meshes composited into ray casting"
category = "meshes"
status = "built"
components = ["meshes/composite"]
related = ["mesh-depth", "raycast", "deferred-shading"]
origin = "project"
sources = []
+++

# Meshes composited into ray casting

## What it is

Animated triangle meshes inside a ray-cast voxel world: the meshes are rasterised first, and
each pixel's voxel ray stops at the mesh's depth.

## How it works

Meshes are rasterised into the hit record before the walk (a mesh triangle's hit sets the
record's mesh bit, with the triangle index). The primary ray of each pixel walks only up to
that distance. Meshes and voxels occlude each other by distance along the primary ray; lit
meshes are shadowed by voxels through the meshes' own shadow site. Meshes cast no shadows yet.

## In this project

`meshes = "composite"`, the default on `gpu-compute`; `crates/mr-gpu/src/meshes.rs`
(`MeshRecords`) and `mesh_surface.wgsl`. Scenes with meshes: `plans/meshes.toml`.

## Measured

Not measured on its own yet; scored against the reference in every `+meshes` pass.

## Open questions

- Shadows cast by meshes; path tracing of mesh surfaces.
