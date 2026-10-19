+++
title = "Beam prepass"
category = "prepass"
status = "measured"
components = ["prepass/beam"]
related = ["raycast", "refloor"]
origin = "literature"
sources = ["S. Laine, T. Karras, Efficient sparse voxel octrees (the beam optimization), I3D 2010"]
+++

# Beam prepass

## What it is

A pass before ray casting that marches one conservative cone per screen tile, so that every
ray of the tile can start where its cone is provably empty.

## How it works

One thread per `tile` × `tile` pixel tile marches the tile's centre ray in steps of one top
cell. At each step, every top cell overlapping a box that covers the whole cone's slab is
asked whether it is empty. The distance up to the first non-empty step is the tile's start;
the rays of the tile skip it (`start_t`). Exact: no ray skips anything it could hit.

## In this project

`components/src/prepass/beam/gpu.wgsl`, `prepass = "beam"` on `gpu-compute` (knob `tile`; 1 is
off). LOD walks run without it.

## Measured

Exact, but still a net loss: chunk-level skipping in the walk already makes empty space
cheap.

## Open questions

- A brick-level beam (finer than the chunk-level one).
