+++
title = "The client: picked or derived LOD levels"
category = "world"
status = "measured"
components = ["client/pick", "client/derive"]
related = ["link", "generator", "lod"]
origin = "project"
sources = []
+++

# The client: picked or derived LOD levels

## What it is

The renderer's side of the world stream: it keeps what has arrived and decides what its
renderer holds, by radius and LOD levels. `pick` takes the coarse levels the server sends;
`derive` builds them itself from the full-resolution chunks.

## How it works

The server sends every client one stream, wide enough for the largest region any client
needs at each level. `pick` passes on the chunks within its `radius` at each of its `max_lod`
levels. `derive` ignores the server's coarse levels: each coarse cell is the topmost non-air
cell of its eight finer cells, built level by level from the client's LOD 0, and rebuilt in
dirty boxes when chunks arrive or blocks are edited, so edits reach every level (with `pick`
they do not yet).

## In this project

`crates/mr-core/src/client.rs`; in plans `sites = { client = "derive(radius=4, max_lod=2)" }`.
Runs in the worker, before the renderer.

## Measured

For the same radius-3, two-level region, `derive` needs 13× `pick`'s bytes (LOD 0 out to
radius 11), so behind a slow link bandwidth, not latency, is what fails it.
