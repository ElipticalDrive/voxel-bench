+++
title = "Hardware ray queries"
category = "traversal"
status = "measured"
components = ["traversal/rayquery"]
related = ["greedy", "naive", "boxes", "raycast"]
origin = "literature"
sources = ["Khronos, VK_KHR_ray_query (Vulkan extension), 2020"]
+++

# Hardware ray queries

## What it is

Rays traced by the GPU's ray-tracing hardware against acceleration structures built from
geometry, called inline from a compute shader (ray queries) rather than through a
ray-tracing pipeline.

## How it works

The executor builds one bottom-level acceleration structure (BLAS) per chunk from the
geometry node the kernel reads (`greedy` or `naive` quads as triangles, or `boxes`: one AABB
per occupied 4³ brick, intersected by a small software DDA). A top-level structure (TLAS) of
the chunks is rebuilt every frame, translated relative to the camera's block for precision.
Primary, shadow and bounce rays are all ray queries. Knob `cull`: culls triangle facing for
primary rays.

## In this project

`components/src/traversal/rayquery/gpu.wgsl`; acceleration structures in
`backends/gpu/src/accel.rs`. The kernel of `gpu-rt`, and usable at any GPU ray-casting site
(for example the shadow rays of a rasteriser). Needs a GPU and driver with ray-query support;
wgpu's ray tracing is experimental.

## Measured

The fastest tracing on the RT cores, but BLAS rebuilds make it CPU-bound whenever chunks
stream in or change.

## Open questions

- Ray-tracing pipelines (closest-hit shaders) instead of ray queries.
