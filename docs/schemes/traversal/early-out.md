+++
title = "Early-out ray casting"
category = "traversal"
status = "built"
components = []
related = ["refloor", "max-height-mipmaps", "direct"]
origin = "user"
sources = []
+++

# Early-out ray casting

## What it is

Stopping each ray, or each piece of per-pixel work, as soon as its answer is known.

## How it works

Built: primary rays stop at the first hit; shadow rays stop at the first occluder; rays stop
when they leave the loaded region or reach `max_steps`; chunk, brick and region skips. Not
built: the max-height early-out for sky and sun rays; shadow rays toward a point light ending
at the light's distance rather than the region's edge (to be checked in every walk); skipping
shading work for pixels whose ray missed.

## In this project

In every traversal kernel and shading component.

## Measured

The built parts are part of every measurement; the open ones are untested.

## Open questions

- The three open items above.
