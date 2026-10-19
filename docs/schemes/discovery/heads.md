+++
title = "Discovery with heads that walk the faces"
category = "discovery"
status = "idea"
components = []
related = ["cluster", "quadtree", "clip-space-discovery"]
origin = "user"
sources = []
+++

# Discovery with heads that walk the faces

**Untested idea.**

## What it is

N "heads" grow across the frame. Each needs only its previous and current step to decide
where to go next, so its state stays hot in cache. Together they discover the whole frame
with no gaps, and no head's trail crosses another's. When a ray hits a block, the area of that
block's face is removed from the frame.

## How it works

- **The lemma (true):** no nearer block can project *entirely* inside the projection of a
  farther block's face. The occluder's face parallel to the face's plane P lies on a plane
  P1 nearer the camera; projected onto P it is an axis-aligned square of side d(P)/d(P1) > 1,
  which cannot fit in a unit face.
- **But single containment is not enough:** the union of several removed faces can hide a
  block (a pillar in front of three removed wall faces). Fix for the coplanar case: the
  pillar's projection onto the wall is a square of side > 1, so it contains a lattice point, a
  corner of a wall face; casting a ray at every corner of every removed face finds it. The
  mixed-plane case (a block covered by a wall face and a floor face) is not proven.
- **Heads without communication** are possible if their movement rules make contact
  impossible by construction (monotone or interleaved fronts); the open task is such a rule
  that also guarantees full coverage. The alternative: each head owns a disjoint region
  (a tile, a Hilbert segment), walks the face adjacency inside it (from a hit face, one ray
  just beyond each projected edge: the coplanar neighbour, or a silhouette), and faces on
  region borders are found twice.
- **Bounded by a distance or work limit,** with degradation in the distance (the LOD
  question), not by a pixel resolution.

## In this project

Not built. Face-cluster discovery grew out of it (corner rays, flood fill over coplanar
faces). It belongs on the CPU, where a head's state stays in cache; GPU threads would
serialise on the walks.

## Measured

Untested.

## Open questions

- A proof (or a test) for the mixed-plane case.
- A movement rule that keeps heads disjoint and covers everything.
