+++
title = "Max-height mipmaps"
category = "storage"
status = "idea"
components = []
related = ["temporal-ray-start", "heightmap", "early-out"]
origin = "literature"
sources = ["A. Tevs, I. Ihrke, H.-P. Seidel, Maximum mipmaps for fast, accurate, and scalable dynamic height field rendering, I3D 2008"]
+++

# Max-height mipmaps

**Untested idea.**

## What it is

A 2D pyramid of the maximum solid height per column region. A ray above a region's maximum
height skips the whole region; sun shadow rays get an immediate early-out above the terrain.

## How it works

Level 0 holds each (x, z) column's top solid height; each coarser level holds the maximum of
its 2 × 2 children. A walk tests the ray's height range across a region against the region's
maximum and skips it when the ray stays above. Exact for any world, most effective on
terrain-like ones.

## In this project

Not built. It would be side data of a storage, used by any kernel as an optional capability.
The `heightmap` storage keeps a per-tile maximum already (one level).

## Measured

Untested.
