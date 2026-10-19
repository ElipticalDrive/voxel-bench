+++
title = "Path tracing on incremental discovery"
category = "shading"
status = "idea"
components = []
related = ["pathtrace", "coverage", "quadtree"]
origin = "user"
sources = []
+++

# Path tracing on incremental discovery

**Untested idea** (observed, not measured).

## What it is

Path tracing (`pathtrace`) on top of incremental discovery (`gpu-discover`, `quadtree` with
`holes = coverage`). The project owner, trying it in the GUI: "I've never seen real time path
tracing as fast as that". The owner has further tricks to make it faster, not yet recorded.

## How it works

A likely reading: discovery rasterises a face set kept across frames, so primary visibility
costs little and most of the frame goes to the shadow and bounce rays, which walk the
composed storage; progressive accumulation while the view is still adds samples over frames.

## In this project

Runs today, in a plan:
`compose = { backend = "gpu-discover", visibility = "quadtree", holes = "coverage", shading = "pathtrace" }`.

## Measured

Untested. To measure: primary visibility and shading times (stage times) against
`gpu-compute` ray casting with `pathtrace` on the same scene.
