+++
title = "Pipelines compiled on demand, and break-in mode"
category = "architecture"
status = "built"
components = []
related = ["zero-cost-composition", "learning-planner"]
origin = "user"
sources = []
+++

# Pipelines compiled on demand, and break-in mode

## What it is

A generic precompiled build of some or all variants, plus variants compiled on demand for the
user's machine and cached. A user-triggered **break-in mode** spends as long as it needs to
build the most optimised version for the machine. Eventually every pipeline is compiled at
run time (the combinations will outgrow precompilation), with a common subset shipped
precompiled. Only existing, well-supported compilers: `rustc` for CPU pipelines (shipped with
the product), the driver through wgpu / naga for shaders, an existing shader module system; no
compiler or JIT of our own.

## How it works

- A pipeline is data: its graph and the knob values baked in, canonical and hashable (the
  cache key, with toolchain and CPU). Knobs are declared *baked* (constants in generated code)
  or *live*.
- For one graph, a small crate is generated that instantiates the generic components with the
  graph's values as constants, and `cargo` builds it with `-C target-cpu=native`.
- Every new build renders a short flight next to the generic build and must match it pixel for
  pixel before it is trusted; it runs as an ordinary worker process, so a bad build only kills
  its worker; the generic build runs while it compiles, then the new one hot-swaps in.
- Break-in: native, fat LTO, profile-guided (trained on a short flight) and both, each timed,
  the fastest kept. On the GPU: specialised constants and the driver's pipeline cache.

## In this project

`src/pipelines.rs`; `run.compile = "native" | "break-in"`. See
[docs/pipelines.md](../../pipelines.md).

## Measured

CPU, terrain, one CCD: native +2 to +13% over the generic build (`prefix` + `refloor`
+11–13%).

## Open questions

- Which further pipelines to precompile, chosen by the planner's run-time feedback.
- A possible extra back end: the WGSL components on the CPU through a software Vulkan driver
  (lavapipe).
