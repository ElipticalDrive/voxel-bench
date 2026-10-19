+++
title = "Hier: bricks plus 8³ region bits"
category = "storage"
status = "measured"
components = ["storage/hier"]
related = ["bricks", "prefix", "refloor", "incremental"]
origin = "project"
sources = []
+++

# Hier: bricks plus 8³ region bits

## What it is

`bricks` with one more level: a 64-bit summary per chunk, one bit per 8³ region. A walk
can skip an empty 8³ region in one step instead of two 4³ steps.

## How it works

The 64 regions of a 32³ chunk are indexed `x | z << 2 | y << 4` from bits 3–4 of each cell
coordinate, so the region test is a shift and a mask, branch-free. The probe answers levels
1, 4 and 8: region bit, then brick bit, then the cell.

## In this project

`components/src/storage/hier/` (`gpu.wgsl`, `cpu.rs`). On every back end that has `bricks`.

## Measured

- CPU, unlit, CCD-pinned, hardware counters: 20–34% fewer steps than `bricks` and up to
  29% more fps (terrain 19.6 against 17.4 fps; edits 36.4 against 26.6; menger 58.5 against
  48.6). It is the fastest CPU storage per pixel: one cheap extra level beats `prefix`'s many
  levels under `refloor`.
- With `incremental`: terrain 21.5 fps (+10%), edits about even, menger −3%.
- GPU, terrain, `refloor`: 0.76 ms unlit, 1.10 ms lit (bricks 0.86 / 1.20).
