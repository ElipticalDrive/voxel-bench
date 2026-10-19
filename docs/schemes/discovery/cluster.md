+++
title = "Face-cluster discovery"
category = "discovery"
status = "measured"
components = ["visibility/cluster"]
related = ["quadtree", "greedy"]
origin = "user"
sources = []
+++

# Face-cluster discovery

## What it is

Discovery that grows over the geometry instead of over screen tiles: from a ray's hit, a
flood fill takes the neighbouring coplanar faces, and rays to the faces' corners confirm them.
The project owner's design.

## How it works

1. **Seeds.** A pixel lattice is walked coarse to fine (`seed_step`, halving down to 1
   pixel); every lattice pixel not yet covered casts a ray.
2. **Clusters.** From the hit face, a flood fill crosses shared edges to exposed faces on the
   same plane.
3. **Corner rays.** Each face is confirmed by rays to its four corners. Visibility belongs to
   the point, so each lattice point is cast once per frame, whatever faces share it.
4. **Cover.** A face with all corners visible covers the pixel centres of its projection, and
   the fill continues through it. A face with an occluded corner joins the set but stops the
   fill.
5. **Sky** (`sky`): `corners` (default): a lattice cell at spacing up to `sky_tile` is sky
   when all four corner seeds miss; `sparse` (the owner's rule): a single miss covers its
   cell, and the lattice shifts each frame so small things appear within a few frames.

*Why corners suffice, nearly:* an axis-aligned square of side > 1 that overlaps a unit face
contains one of that face's corners. An occluder's near face projects onto the plane as such
a square, so a pillar in front of a wall is caught. Its full shadow (the hull of its near and
far faces) can overlap a face by a sliver without covering a corner, so completeness is not
proven; in tests (floor, wall, pillar, floating block, three viewpoints) and on terrain with
`sky_tile = 1`, no pixel is missed.

## In this project

`components/src/visibility/cluster/cpu.rs`, CPU only, on `cpu-discover`. Views `clusters`
(one colour per cluster) and `kinds` (confirmed, occluded corner, carried over).

## Measured

Against the quadtree, synchronous, 1080p unlit:

| scene | rays/px quadtree → cluster | missed pixels | PSNR |
|---|---|---|---|
| terrain | 0.254 → 0.242 | 0.0062% → 0.0038% | 43.8 → 51.0 dB |
| menger | 0.114 → 0.150 | 0.68% → 0 | 26.4 → 51.5 dB |
| room | 0.027 → 0.025 | 0 → 0 | 60.5 → 60.5 dB |

- More complete everywhere at about the same rays: about one corner ray per confirmed face.
- 2.5× the quadtree's CPU time (100 against 40 ms on terrain): the fill runs in ~460 waves
  per frame and about a million hash operations.
- Sky decides the cost: `sparse` at 8 pixels: 0.26 rays per pixel but 0.15% missed; every
  sky pixel sampled: 0.89 rays per pixel, none missed; `corners` gets both.

## Open questions

- Far-field consolidation (super-faces where faces are smaller than a pixel: a form of LOD).
- A scanline fill (row runs per plane) to remove the hash traffic; a GPU version.
