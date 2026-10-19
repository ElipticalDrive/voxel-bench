+++
title = "CPU ray packets and SIMD hot loops"
category = "traversal"
status = "measured"
components = []
related = ["refloor", "incremental", "brick-layouts"]
origin = "literature"
sources = ["I. Wald, P. Slusallek, C. Benthin, M. Wagner, Interactive rendering with coherent ray tracing, Eurographics 2001"]
+++

# CPU ray packets and SIMD hot loops

## What it is

Tracing several neighbouring rays in lockstep, so the CPU's vector units work on all of them
at once, and splitting the image over threads in cache-friendly pieces.

## How it works

The `cpu` back end walks `refloor` in packets of N rays (knob `packet`), with hot loops
written for scalar, AVX2 and AVX-512 (knob `isa`). Work is split over threads by image rows
(packets along the row) or, with `schedule = tiles`, by screen tiles handed out along a
space-filling curve (`tile_order` = morton, hilbert or linear), with 2D packets.

## In this project

`backends/cpu/src/raycast.rs` (the lane), `components/src/traversal/refloor/cpu.rs`. Knob
`perf = true` (Linux) adds hardware counters per frame: cycles, instructions and cache and TLB
misses per pixel.

## Measured

- Packets bring no gain on Zen 5, and **not** because of memory: ~80 cycles and ~220
  instructions per step at IPC 2.6, L2 misses at most 3% of the time. What helps is fewer
  steps (see [hier](../storage/hier.md)) and fewer instructions per step.
- Avoiding libm `floor` calls was the largest single win (the `cpu` back end is about 1.5×
  the reference renderer).
- `schedule = tiles`: +4% over rows.
- Compiled for the machine (`compile = "native"`): +2 to +13% (`prefix` + `refloor` +11–13%),
  because the generic x86-64 target lacks `popcnt` and wide vectors outside the hand-written
  AVX paths.
- Tried and dropped: software prefetch (it only added instructions).

## Open questions

- Fewer instructions per step: register spills, saturating float-to-int casts, bounds checks
  in the hot loop.
