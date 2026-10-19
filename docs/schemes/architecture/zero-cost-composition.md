+++
title = "Composition at no cost, without redundant code"
category = "architecture"
status = "built"
components = []
related = ["pipeline-graph", "compile-on-demand"]
origin = "user"
sources = []
+++

# Composition at no cost, without redundant code

## What it is

Renderers assembled from small modules with no performance penalty, so that implementing one
more part yields many new combinations, and the composition itself becomes a knob. The project
owner's requirements:
1. no cost from composability while a pipeline runs (compiling before it runs is fine);
2. no opinion on hardware configuration;
3. as much code reused as is practical;
4. as many combinations as practical, grown with *adapters* (compile-time code giving a
   method a feature it lacks, even if slower);
5. combinations without a valid path do not exist (greyed with the reason), never silently
   substituted;
6. a feature costs only the pipelines that use it: no runtime flags in hot paths, no wider
   records, no extra passes for everyone.

*Redundant* means the same algorithm written again to accommodate a combination of features;
the same algorithm once per device, API or mechanism is necessary, not redundant.

## How it works

- Every storage answers every query a kernel may ask, through an adapter where it has no fast
  path: good pairings are fast, bad pairings slow, without odd architecture.
- GPU: modules are composed into one shader per pipeline before compilation (naga_oil), so
  the driver optimises across module boundaries. CPU: Rust generics are monomorphised per
  combination, or the pipeline is generated and compiled for its graph.
- After visibility, the common hit record is the boundary, so one shading implementation per
  kind of device serves every strategy, and one face drawer every face source.
- A guard test (`tests/architecture.rs`) fails on WGSL functions defined twice across back
  ends, copied bodies, or lighting outside the shading component.
- "No cost" is testable: a composed pipeline is as fast as a hand-written one of the same
  pieces on the same devices; existing pipelines keep their pixels and speed (A/B scripts).

## In this project

`components/` (registry, capabilities and adapters in `caps.rs`), `crates/mr-gpu/src/program.rs`
(shader modules), `tests/architecture.rs`, `scripts/ab/`.

## Measured

- `bricks` + `refloor` composed ran 7–9% faster than the hand-written renderer it replaced
  (which carried LOD code); `distance` + `sphere` 0.60 against 0.64 ms; `rows` + `rowrun`
  equal.
- The guard found and removed five copies of `occluded` and nine of the primary-ray formula.
- A side finding: WGSL's `select` evaluates both sides, so ten renderers paid for lighting
  in unlit frames; branching instead cut unlit GPU time 25–35%.
- Runtime flags cost: a uniform lit flag 4–6% on NVIDIA, a uniform loop bound up to 45%
  (incremental); hence constants.
