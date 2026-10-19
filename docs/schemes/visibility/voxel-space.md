+++
title = "Voxel Space: column marching over a heightmap"
category = "visibility"
status = "measured"
components = ["visibility/columns"]
related = ["heightmap"]
origin = "literature"
sources = ["NovaLogic, Comanche: Maximum Overkill (the Voxel Space engine), 1992", "S. Macke, VoxelSpace (an explanation and reimplementation), github.com/s-macke/VoxelSpace"]
+++

# Voxel Space: column marching over a heightmap

## What it is

The 2.5D technique of the 1992 game Comanche: for each screen column, march front to back
over a heightmap and draw vertical spans, keeping a y-buffer (the highest row drawn so far) so
that nearer terrain hides farther terrain.

## How it works

One thread (GPU) or task (CPU) per screen column walks the heightmap along the column's
direction. Each column top it passes projects to a screen row; if that row is above the
y-buffer, the span between them is drawn and the y-buffer raised. Shading is inline in the
march or deferred (`placement`: the march writes hit records, the common pass shades them).
Shadow and bounce rays walk the same heightmap.

## In this project

`components/src/visibility/columns/` (`gpu.wgsl`, `cpu.rs`) over storage `heightmap`; back
ends `gpu-voxelspace` and `cpu-voxelspace`.

## Measured

- No overhangs, caves or trunks (PSNR ~15–36 dB), 27 MB of GPU memory.
- The CPU version gives the identical image at about half the GPU version's speed.
- A per-pixel hook function in the march cost 2.5% of unlit GPU time on NVIDIA, so the
  march's output is chosen by a shader define instead.
