+++
title = "Discovery and meshing as face sources"
category = "discovery"
status = "built"
components = []
related = ["quadtree", "cluster", "greedy", "rasterisation", "forward-raster"]
origin = "user"
sources = []
+++

# Discovery and meshing as face sources

## What it is

Discovery is a precursor to rasterisation, like meshing: both turn the voxel world into faces
a rasteriser can draw. Meshing is view-independent (it runs on chunk changes and yields every
exposed face); discovery is view-dependent (it runs every frame and yields only the faces it
sees: meshing and occlusion culling in one, by ray sampling). They differ only in what feeds
the face drawer's input. The project owner's observation.

## How it works

In the graph, both are nodes whose output is `faces`, read by one shared face drawer. So
"gpu-raster" and "gpu-discover" are names of preset graphs of one GPU executor, not separate
renderers.

## In this project

Built: the face drawer (`components/src/draw/`) is shared, and one GPU executor runs both.

## Measured

Not a technique of its own; see the entries of its parts.

## Open questions

Combinations it opens, untested: greedy-merging discovered faces beyond `merge = rows`;
meshing culled by discovery (mesh chunks, draw only what discovery found visible); discovery
seeded from meshed faces.
