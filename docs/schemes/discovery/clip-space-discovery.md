+++
title = "Discovery in signed fixed-point clip space"
category = "discovery"
status = "idea"
components = []
related = ["heads", "cluster", "quadtree"]
origin = "user"
sources = []
+++

# Discovery in signed fixed-point clip space

**Untested idea.**

## What it is

Running discovery in a `[-1, 1]²` coordinate space, independent of resolution and aspect
ratio, which maps directly onto signed fixed-point coordinates. Rendering is unchanged: it
receives a list of faces, as from any other method.

## How it works

Refinement follows the geometry, not the pixel grid. With integer edge functions and a
top-left rule (as GPU rasterisers do), adjacent face projections tile the plane exactly: no
gaps, no double coverage. Pixels stay the yardstick for checking the result, because the
image is point-sampled there: "complete" means nothing that covers a sample point is missing,
within the distance or work limit.

## In this project

Not built. Face-cluster discovery did not need it: there the pixel lattice is the yardstick,
and coverage is an exact pixel-centre test against projected faces.

## Measured

Untested.
