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

The CPU executors write the same frames: every CPU lane (ray casting, rasterisation, columns)
and the shared CPU components (direct shading, the deferred pass, the reference model) are
generic over the output's pixel (`Pixel`: `[u8; 4]` or `Half4`, IEEE halves rounded to nearest
even as the GPU stores them), so an RGBA8 pipeline compiles as before. The precompiled CPU build
holds both and picks by the post; a pipeline compiled for its graph names its pixel type.

## In this project

`components/src/post/hdr.wgsl`, `crates/mr-core/src/lighting.rs` (`Tone::Hdr`,
`PixelFormat`, `hdr_to_rgba8`), `crates/mr-gpu/src/lib.rs` (`output_format`, `FrameRing`),
`src/worker.rs` (`take_frame`, `rgba8`); CPU: `mr_core::lighting::{Pixel, Half4,
f32_to_half}`, `backends/cpu/src/lib.rs` (`Frame<P>`, `Lane::Px`, `create_as::<P>`). In plans:
`sites = { post = "hdr" }`, on every executor.

## Measured

2026-10-19, terrain 1080p, three interleaved rounds: pipelines without `hdr` keep their pixels
and GPU times (±0.2 %). With `hdr`, the 8-bit form equals `clamp`'s (unlit frames identical,
lit frames within one quantisation step on a few values, the half's rounding) on every GPU
executor (gpu-compute, gpu-raster raster / visbuffer / gbuffer, gpu-discover, cpu-discover,
gpu-rt, gpu-splat, gpu-voxelspace, two GPUs, mesh mode). GPU time: gpu-compute +0.3 %, gpu-raster
±0, gpu-discover unlit +9 % (lit: noise); host-bound fps −1 to −4 %, more at thousands of fps
(the wider output). Terrain's lit frames reach linear 10 in highlights (0.1 % of the pixels
above 1).

CPU, 2026-10-19 (terrain, 960x540; every lane, both placements, the steps view, generic and
compiled): the 8-bit form equals `clamp`'s, unlit identical, lit within one step on 1–5 % of
the pixels (PSNR 66–74 dB); compiled float frames bit-identical to the generic build's; RGBA8
pipelines keep their pixels (60 of 60 frames identical to the build before).
Cost of `hdr` on the CPU (1080p, three interleaved rounds, CPU ms): ray casting −0.1 to +0.4 %,
cpu-raster +3 to +4 % inline (deferred +1 to +2.5 %), cpu-voxelspace +4 to +9 % but +20 % unlit inline
(2.2 → 2.6 ms: twice the bytes through its column transpose). An 8-bit channel becomes a half
through a 256-entry table (the division per channel cost voxelspace unlit +38 %). The
precompiled CPU build holds every lane twice: backend-cpu builds in 60 s instead of 28 s
(release; dev 80 s instead of 40 s).

## Open questions

- Presenting it: the `view` window and the GUI's ☀ HDR present float frames on an HDR surface
  where the display offers one (not yet seen on an HDR desktop).
- The GPU executors' sky is packed as RGBA8 and clips at 1 (the CPU's goes through the post).
