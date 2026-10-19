+++
title = "Mesh mode: animated meshes with voxels"
category = "meshes"
status = "built"
components = []
related = ["mesh-composite", "mesh-depth"]
origin = "user"
sources = ["T. Möller, B. Trumbore, Fast, minimum storage ray-triangle intersection, Journal of Graphics Tools 1997"]
+++

# Mesh mode: animated meshes with voxels

## What it is

Arbitrary animated triangle meshes rendered together with the voxel scene, as a mode
independent of lighting: `unlit`, `lit`, `unlit+meshes`, `lit+meshes`. Pipelines support it
optionally; the bench and the tuner use only pipelines that match the requested mode. Scope:
only meshes are animated; the aim is a fair number of meshes with very large amounts of
voxels, not huge amounts of both.

## How it works

- Scenes (`scene.meshes`): box, sphere and torus with per-vertex sRGB colours, scattered
  copies, spin / orbit / bob animation, deterministic in the frame index. Geometry is sent
  once; instances when they change.
- Semantics: interpolated vertex colours with perspective-correct barycentrics; flat normals
  on the side facing the viewer; meshes and voxels occlude each other by distance; lit meshes
  use the voxel lighting model and are shadowed by voxels; meshes cast no shadows yet.
- The reference intersects triangles in f64 (two-sided Möller–Trumbore, bounding-sphere
  culling).
- How pipelines draw them: see [mesh-composite](mesh-composite.md) and
  [mesh-depth](mesh-depth.md). In the GUI, **Lit** and **Meshes** are two latching toggles.

## In this project

`crates/mr-core/src/mesh.rs` (semantics), `crates/mr-gpu/src/meshes.rs`; `plans/meshes.toml`
(a voxel Cornell box with a tumbling torus, orbiting spheres and bobbing boxes, 5,520
triangles).

## Measured

On `plans/meshes.toml`: meshes cost about 1 dB of edge pixels and almost no time for a
rasteriser (0.21 → 0.23 ms lit); ray casters composite them for +0.01–0.05 ms.

## Open questions

- Mesh shadows (on voxels and on each other), skinning, path tracing of mesh surfaces.
