+++
title = "Targeted loading priority"
category = "world"
status = "idea"
components = []
related = ["client", "lod", "link", "served-split", "coverage"]
origin = "user"
sources = []
+++

# Targeted loading priority

**Untested idea.**

## What it is

Choosing *which* chunks to load first when not everything in range can arrive at once (a
per-frame budget, a slow link): the ones the viewer will notice missing first.

## How it works

The client node orders its requests by a score instead of by distance alone. Candidate terms:
- inside the view frustum (or about to be: the camera's motion predicted a few frames ahead);
- the screen area the chunk would cover, at its LOD level (a coarse level first, refined later);
- holes the renderer saw this frame (discovery's coverage, sky pixels where the coarse world
  says ground);
- occlusion: chunks behind loaded solid ground later;
- the chunk's cost on the link (its size compressed) against its value.

The score is a knob set of the client (or a node of its own between the renderer's feedback
and the link), so pipelines compare loading orders by the measured holes and the time to the
first complete frame.

## In this project

Not built. The client picks chunks by Chebyshev rings per LOD level; the server loads within a
per-frame budget (`streaming = { budget = N }`); passes report holes per frame and the first
complete frame.

## Measured

Untested.
