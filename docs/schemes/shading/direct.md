+++
title = "Direct lighting: the reference model"
category = "shading"
status = "measured"
components = ["shading/direct"]
related = ["pathtrace", "deferred-shading", "reference-dda", "tone-mapping"]
origin = "literature"
sources = []
+++

# Direct lighting: the reference model

## What it is

The lighting model every renderer is judged by. Unlit: the face's texel. Lit: albedo × (ambient
+ sun + the point lights), each light with a hard shadow ray.

## How it works

At a surface point (block, face, block-local position), the texel is sampled (nearest). In
lit mode, the sun and every point light near the point add their contribution with windowed
inverse-square falloff (`lighting::attenuation`), each tested by a shadow ray through the
pipeline's shadow walk (`occluded`), which may be another storage and kernel than the primary
rays'. The result passes through the post node (tone mapping) and is encoded as sRGB. Knob
`shadows`: `all` (default), `sun` or `none`.

## In this project

`components/src/shading/direct/` (`gpu.wgsl`, `cpu.rs`): on every back end, written once per
device kind. The architecture test checks that no back end lights a surface any other way.
`cpu-raster` keeps the reference renderer's f64 model on purpose (higher PSNR).

## Measured

The lit pass costs about 40% on top of unlit for ray casting on terrain (`prefix` + `refloor`:
0.75 ms unlit, 1.07 lit), more for rasterisers with overdraw.
