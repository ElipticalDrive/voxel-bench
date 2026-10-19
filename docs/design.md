# Design

The requirements behind the bench's architecture, how each is met, and what it measured.
How to use the pieces is in [pipelines.md](pipelines.md) and [architecture.md](architecture.md);
the rendering techniques themselves are in the [encyclopedia](schemes/README.md); ideas for the
bench not built yet are in [ROADMAP.md](../ROADMAP.md).

## Pipelines are graphs

A renderer is a graph of component instances with typed ports, not a fixed list of stages:
"I in no way require that every pipeline have the same list of component types. I treat any
valid path from the input to the output regardless of stages as viable" (the project owner).
Every place rays are cast picks its own ray-casting method and storage. The architecture was
settled before optimisation, so that it sets no limits for later.

- **Pieces** (a first-principles split of the problem): the world and its change log;
  representations derived from it and kept current (storages, meshes, BLAS, face sets);
  traversal kernels answering ray queries against a representation, always fused with it on
  one device; visibility strategies (ray cast, raster, discovery, splat, columns) writing one
  common hit record; surface sources (meshes) writing into the same record; shading reading
  hits and issuing secondary queries, inline or deferred; history (earlier frames' products as
  inputs); post (tone mapping, layers, upscaling, debug views); an executor placing nodes on
  devices and inserting transfers.
- **Two kinds of composition:** fused, at source level (representation + kernel + inline
  shading compiled together on one device, where the per-step cost lives), and at graph level
  (data products: change log, face sets, hit records, history, images), the only things that
  cross devices.
- **State flows both ways:** edges may form cycles; each says when its value arrives (`now`,
  `prev` = the previous frame, `iterate = n` within the frame). Cycles only through `prev` or
  `iterate`.
- Not every combination can be checked, and that is fine: correctness rests on per-port
  contracts and the exact reference; graphs are sampled, not enumerated.

Code: `components/src/graph/` (model, checks with reasons, canonical form and cache key,
schedule); the executors in `backends/`. Measured: every pipeline rebuilt as a graph kept its
images at equal or better speed, except `tree64` (−5%) and the CPU layout storages (−3–4%).

## Composition at no cost, without redundant code

Renderers are assembled from small modules, so that one more part yields many new
combinations and the composition itself is a knob. The project owner's requirements:
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

- Every storage answers every query a kernel may ask, through an adapter where it has no fast
  path: good pairings are fast, bad ones slow.
- GPU: modules are composed into one shader per pipeline before compilation (naga_oil), so the
  driver optimises across module boundaries. CPU: Rust generics are monomorphised per
  combination, or the pipeline is generated and compiled for its graph.
- After visibility, the common hit record is the boundary, so one shading implementation per
  kind of device serves every strategy, and one face drawer every face source.
- A guard test (`tests/architecture.rs`) fails on WGSL functions defined twice across back
  ends, copied bodies, or lighting outside the shading component.
- "No cost" is testable: a composed pipeline is as fast as a hand-written one of the same
  pieces on the same devices; existing pipelines keep their pixels and speed (`scripts/ab/`).

Code: `components/` (registry; capabilities and adapters in `caps.rs`),
`crates/mr-gpu/src/program.rs` (shader modules). Measured:
- `bricks` + `refloor` composed ran 7–9% faster than the hand-written renderer it replaced
  (which carried LOD code); `distance` + `sphere` 0.60 against 0.64 ms; `rows` + `rowrun`
  equal.
- The guard found and removed five copies of `occluded` and nine of the primary-ray formula.
- WGSL's `select` evaluates both sides, so ten renderers paid for lighting in unlit frames;
  branching instead cut unlit GPU time 25–35%.
- Runtime flags cost: a uniform lit flag 4–6% on NVIDIA, a uniform loop bound up to 45%
  (`incremental`); hence constants.

## A device and an API per node

Device and API are knobs of every node, so each component runs where and how it suits its
work. A node runs on one device; cross-device edges have a known, accepted cost.

`device` is a slot of the run's device configuration (`cpu`, `gpu`, `gpu1`, ...; the host maps
slots to adapters, so components hold no hardware opinion) and `api` how it runs there
(compute, raster, ray queries; scalar, AVX2, AVX-512). Components declare implementations per
(device kind, API); a choice without one is greyed with the reason. Fused data (a storage and
its kernel) never crosses devices; any other edge may, through a transfer adapter (readback,
upload). Devices are handed to components; no component opens one.

Code: `components/src/impls.rs`, `backends/gpu/src/remote.rs` (the split over two GPUs);
`plans/two-gpus.toml`. Measured: see
[deferred shading](schemes/shading/deferred-shading.md) (ray casting on an RTX 5060, shading
on a Radeon iGPU a frame behind).

## Pipelines compiled on demand

A generic precompiled build, plus builds compiled on demand for the user's machine and cached;
a user-triggered **break-in mode** spends as long as it needs on the most optimised build.
Eventually every pipeline is compiled at run time (the combinations will outgrow
precompilation), with a common subset shipped precompiled. Only existing compilers: `rustc`
for CPU pipelines, the driver through wgpu / naga for shaders; no compiler or JIT of our own.

- A pipeline is data: its graph and the knob values baked in, canonical and hashable (the
  cache key, with toolchain and CPU). Knobs are declared *baked* (constants in generated code)
  or *live*.
- For one graph, a small crate is generated that instantiates the generic components with the
  graph's values as constants, built with `-C target-cpu=native`.
- Every new build renders a short flight next to the generic build and must match it pixel for
  pixel before it is trusted; it runs as an ordinary worker process, so a bad build only kills
  its worker; the generic build runs while it compiles, then the new one hot-swaps in.
- Break-in: native, fat LTO, profile-guided (trained on a short flight) and both, each timed,
  the fastest kept.

Code: `src/pipelines.rs`; `run.compile` (see [pipelines.md](pipelines.md)). Measured: CPU,
terrain, one CCD: native +2 to +13% over the generic build (`prefix` + `refloor` +11–13%).
