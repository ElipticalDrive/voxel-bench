+++
title = "Tone mapping"
category = "post"
status = "built"
components = ["post/clamp", "post/reinhard", "post/aces"]
related = ["direct", "pathtrace"]
origin = "literature"
sources = ["E. Reinhard, M. Stark, P. Shirley, J. Ferwerda, Photographic tone reproduction for digital images, SIGGRAPH 2002", "K. Narkowicz, ACES filmic tone mapping curve, knarkowicz.wordpress.com, 2016"]
+++

# Tone mapping

## What it is

The map from the shading's linear radiance to a display colour: `clamp` (to 0–1, the
reference's), `reinhard` (on luminance) or `aces` (Narkowicz's fit of the ACES filmic curve).

## How it works

The post node is compiled into the shading's final encode on the same device: no pass and no
image of its own. On the GPU it is a module (`display(c)`), on the CPU `Tone::encode`. It maps
lit radiance (after the environment's exposure) and the sky; unlit texels and diagnostic
views pass through unmapped.

## In this project

`components/src/post/` (`reinhard.wgsl`, `aces.wgsl`), `crates/mr-core/src/lighting.rs`
(`Tone`). In plans: `sites = { post = "aces" }`. The reference renderer stays `clamp`.

## Measured

No run-time cost: the default pipelines' pixels and GPU times were unchanged when the node
was added. GPU ray casting, CPU ray casting and the face drawer give the same image for each
tone.
