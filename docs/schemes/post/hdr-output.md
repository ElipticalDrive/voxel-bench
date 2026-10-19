+++
title = "HDR output"
category = "post"
status = "built"
components = ["post/hdr"]
related = ["tone-mapping", "pathtrace"]
origin = "literature"
sources = ["IEC 61966-2-2 (scRGB), extended sRGB as in VK_EXT_swapchain_colorspace", "SMPTE ST 2084 (PQ), ITU-R BT.2100"]
+++

# HDR output

## What it is

A post component that maps nothing and limits nothing: the pipeline's output becomes a float
image (RGBA16F) holding the lit colours beyond 1, for an HDR display to show. Its 8-bit form is
`clamp`'s image.

## How it works

The output holds extended sRGB: the sRGB transfer curve continued past 1, unquantised. Every
other writer of the output (the sky, unlit texels, diagnostic views, overlays) writes the same
display colours it writes into RGBA8, and extended sRGB holds them unchanged, so only the post
(`display`) differs and nothing else in a pipeline knows about the format. The output texture,
its bindings and render targets, and the read-back take the format from the post; the WGSL
declarations of the output switch on the shader def `HDR`. A pipeline without `hdr` compiles
exactly as before.

The host converts a float frame to RGBA8 (clamp and quantise, a 64 K table) only where an 8-bit
frame is needed: the window, comparisons against the reference, PNGs. Saved frames also go to a
Radiance `.hdr` file in linear light.

## In this project

`components/src/post/hdr.wgsl`, `crates/mr-core/src/lighting.rs` (`Tone::Hdr`,
`PixelFormat`, `hdr_to_rgba8`), `crates/mr-gpu/src/lib.rs` (`output_format`, `FrameRing`),
`src/worker.rs` (`take_frame`, `rgba8`). In plans: `sites = { post = "hdr" }`. GPU executors
only: the CPU executors write RGBA8, so `hdr` on a CPU device is refused with that reason.

## Measured

2026-10-19, terrain 1080p, three interleaved rounds: pipelines without `hdr` keep their pixels
and GPU times (±0.2 %). With `hdr`, the 8-bit form equals `clamp`'s (unlit frames identical,
lit frames within one quantisation step on a few values, the half's rounding) on every GPU
executor (gpu-compute, gpu-raster raster / visbuffer / gbuffer, gpu-discover, cpu-discover,
gpu-rt, gpu-splat, gpu-voxelspace, two GPUs, mesh mode). GPU time: gpu-compute +0.3 %, gpu-raster
±0, gpu-discover unlit +9 % (lit: noise); host-bound fps −1 to −4 %, more at thousands of fps
(the wider output). Terrain's lit frames reach linear 10 in highlights (0.1 % of the pixels
above 1).

## Open questions

- Presenting it: eframe's surface is SDR; an HDR window needs its own wgpu surface
  (wgpu 30 has `SurfaceColorSpace`: extended sRGB linear, BT.2100 PQ).
- The CPU executors (a float frame of their own).
- The sky is packed as RGBA8 and clips at 1.
