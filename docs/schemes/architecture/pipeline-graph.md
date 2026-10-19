+++
title = "Pipelines as graphs"
category = "architecture"
status = "built"
components = []
related = ["zero-cost-composition", "device-per-node", "compile-on-demand", "served-split"]
origin = "user"
sources = []
+++

# Pipelines as graphs

## What it is

A renderer is a graph of component instances with typed ports, not a fixed list of stages:
"I in no way require that every pipeline have the same list of component types. I treat any
valid path from the input to the output regardless of stages as viable" (the project owner).
Every place rays are cast picks its own ray-casting method and storage. The architecture was
to be settled with finality, setting up no limits for later, before optimisation.

## How it works

- **Pieces** (a first-principles split of the problem): the world and its change log;
  representations derived from it and kept current (storages, meshes, BLAS, face sets);
  traversal kernels answering ray queries against a representation, always fused with it on
  one device; visibility strategies (ray cast, raster, discovery, splat, columns) writing one
  common hit record; surface sources (meshes) writing into the same record; shading reading
  hits and issuing secondary queries, inline or deferred; history (earlier frames' products as
  inputs); post (tone mapping, layers, upscaling, debug views); an executor placing nodes on
  devices and inserting transfers.
- **Two kinds of composition:** fused, at source level (representation + kernel + inline
  shading compiled together, same device, where per-step cost lives), and at graph level (data
  products: change log, face sets, hit records, history, images), the only things that cross
  devices.
- **State flows both ways:** edges may form cycles; each says when its value arrives (`now`,
  `prev` = the previous frame, `iterate = n` within the frame). Cycles only through `prev` or
  `iterate`.
- Not every combination can be checked, and that is fine: correctness rests on per-port
  contracts and the exact reference; graphs are sampled, not enumerated. Every renderer must
  fit the graph, or the space cannot be explored.

## In this project

`components/src/graph.rs` (model, checks with reasons, canonical form and cache key,
schedule); the executors in `backends/`. See [docs/pipelines.md](../../pipelines.md).

## Measured

Every former standalone renderer was rebuilt as a graph with the same images and equal or
better speed, except `tree64` (−5%) and the CPU layout storages (−3–4%).
