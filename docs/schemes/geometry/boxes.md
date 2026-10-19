+++
title = "Brick boxes for hardware ray tracing"
category = "geometry"
status = "measured"
components = ["geometry/boxes"]
related = ["rayquery", "bricks"]
origin = "project"
sources = []
+++

# Brick boxes for hardware ray tracing

## What it is

Instead of triangles, one axis-aligned bounding box per occupied 4³ brick in a chunk's
acceleration structure. The RT cores find the candidate boxes; a small software DDA through
the voxel pool finds the voxel inside.

## How it works

The BLAS holds procedural AABB geometry: one box per occupied brick, or one per uniformly
solid chunk. For every candidate box the query reports, the shader runs a DDA through the
box's cells in the voxel pool and reports the nearest solid cell
(`rayQueryGenerateIntersection`); shadow rays stop at the first one.

## In this project

`components/src/geometry/boxes/`; on `gpu-rt` only.

## Measured

The same images and speed as the standalone renderer it was made from; not yet compared with
triangle geometry in one run.
