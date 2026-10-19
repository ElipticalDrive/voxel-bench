+++
title = "Starting rays at last frame's distance"
category = "traversal"
status = "idea"
components = []
related = ["max-height-mipmaps", "beam", "refloor", "early-out"]
origin = "user"
sources = []
+++

# Starting rays at last frame's distance

**Untested idea.**

## What it is

Start each primary ray at, or near, the distance its pixel hit last frame, instead of at the
camera, so the walk skips the steps that lead up to the surface. World knowledge can skip
space as well (see [max-height-mipmaps](../storage/max-height-mipmaps.md)).

## How it works

Last frame proves that the segment from the *old* camera to the old hit was empty. A new ray
is safe up to the first point where it leaves the union of those old segments: a star-shaped
empty region around the old camera, which with motion is not the reprojected depth itself. A
conservative start: the minimum old depth over a neighbourhood (3 × 3, wider for faster
motion), minus the camera's translation. A start that is too late misses a new near occluder
(an edit, or something revealed by motion), so every edit must invalidate the start distances
it affects. In the graph it is a `prev` edge from the visibility node's depth to its own start.

## In this project

Not built. The beam prepass is the nearest relative: it also gives rays a start distance, but
from a proof over empty space rather than from history.

## Measured

Untested. Why it could pay: the beam's empty-space skip saved nothing, but this skips the
near-surface steps, which are most of the ~20 steps per pixel on terrain. To be prototyped
behind a knob, with the audit and the reference measuring its errors.
