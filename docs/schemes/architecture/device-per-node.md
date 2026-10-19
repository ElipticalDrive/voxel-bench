+++
title = "A device and an API per node"
category = "architecture"
status = "built"
components = []
related = ["pipeline-graph", "device-inventory", "deferred-shading"]
origin = "user"
sources = []
+++

# A device and an API per node

## What it is

Device and API are a knob of every node, so each component runs where and how it suits its
work and the distribution of work. Cross-device edges have a known, accepted cost. A node runs
on one device (indivisible); nodes split over devices would be a later extension.

## How it works

`device` is a slot of the run's device configuration (`cpu`, `gpu`, `gpu1`, ...; the host
maps slots to adapters, so components hold no hardware opinion) and `api` how it runs there
(compute, raster, ray queries; scalar, AVX2, AVX-512). Components declare implementations per
(device kind, API); a choice without one is greyed with the reason. Fused data (a storage and
its kernel) never crosses devices; any other edge may, through a transfer adapter (readback,
upload). Devices are handed to components; no component opens one.

## In this project

`components/src/impls.rs`, `backends/gpu/src/remote.rs` (the split over two GPUs);
`plans/two-gpus.toml`.

## Measured

See [deferred-shading](../shading/deferred-shading.md): ray casting on an RTX 5060 and
shading on a Radeon iGPU, a frame behind, unlit 191 → 216 fps against the same split shading
the current frame.

## Open questions

- Other splits: other record writers, peer copies, splitting a node by screen tiles or ray
  batches.
