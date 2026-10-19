+++
title = "Points: one per exposed block"
category = "geometry"
status = "built"
components = ["geometry/points"]
related = ["splat"]
origin = "literature"
sources = []
+++

# Points: one per exposed block

## What it is

The geometry for point splatting: one point per block that has an exposed face.

## How it works

A point is the block's world position, its id and a six-bit mask of which neighbours are
air. The node keeps every chunk's points and rebuilds those of changed chunks and their
neighbours.

## In this project

`components/src/geometry/points/`. For visibility `splat` on `gpu-splat`.

## Measured

See [splat](../visibility/splat.md).
