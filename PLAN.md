# Plan

**THE PLAN (2026-10-12): finish the architecture, steps 9–15 below. It is the only thing
on the plan. Optimisation, bug fixes, new techniques and the backlog wait until it is done;
a bug is fixed only if it blocks this work.** The previous plan document (status,
findings, known issues, backlog, steps 1–8 and their progress log) is archived in
`docs/plan-archive-2026-10-12.md`; ideas stay in `IDEAS.md`.

## The goal

The user's architecture, settled with finality and without limits for later (IDEAS §17,
§18 and the clarification at the end of §17):
- **A pipeline is a graph, not a fixed list of stages.** Any valid path from the input to
  the output is a viable pipeline; pipelines need not share the same kinds of components.
- **Every place rays are cast picks its own ray-casting method and storage** (primary,
  discovery, exposure tests, shadow, bounce, later AO), independently or shared.
- The §17 requirements: no cost from composability while a pipeline runs (compiling
  before it runs is fine); no opinion on hardware configuration; maximum practical reuse;
  maximum practical combinations, grown with compile-time adapters; combinations without a
  valid path do not exist (greyed with the reason); generic precompiled builds plus builds
  compiled on demand for the machine, cached, checked against a trusted build, hot-swapped;
  existing compilers only (`rustc`, the driver via wgpu / naga, an existing shader module
  system).

## What is missing (after steps 1–8, against IDEAS §17–§18 and the graph requirement)
- **The fixed stage list itself.** `Slot` is a fixed enum, `Composition` a struct with one
  field per slot, `BackendInfo` a fixed slot list per back end. A pipeline cannot have
  stages another lacks beyond that list, nor bind a different kernel at each place rays
  are cast. Step 7c's `shadow_storage` / `shadow_traversal` are a hard-coded special case
  of what should be general.
- **Strategies inside back ends:** discovery (quadtree, cluster and their pieces), splat
  and voxelspace columns are back-end code, not components; the ten back ends are
  hand-written assemblers, not executors placing pieces on devices (§18).
- **No common visibility buffer:** strategies do not share one hit record / visibility
  buffer; inline or deferred shading is a choice only where a face drawer exists; meshes
  are a special stage instead of a surface source (§18).
- **No per-stage devices:** cpu-discover (CPU discovery, GPU drawing) is a hard-wired back
  end, not a configuration; two GPUs cannot be combined (requirement 2).
- **No history and no post stages** (§18 pieces 7 and 8).
- **No per-stage measurement** (§18: every piece reports its own time; the planner needs it).
- **Composition not fully data:** knobs not declared as baked-in or live; CPU components
  declare no capabilities; only the `cpu` back end compiles on demand (§17).
- **A known composition cost:** cpu-discover calls its storage through `dyn World` per ray.
- **Shader assembly by text-joining** strains: 7c needed a hand-written renaming pass
  (`traversal::namespaced`); the plan said to adopt an existing module system then.

## The steps (in dependency order; each verified and zero-cost-checked, one at a time)
9. **Pipelines as graphs** (the foundation; replaces `Slot`, `Composition`'s fixed fields
   and per-back-end slot lists). Components declare typed *ports*: inputs they need and
   outputs they provide (e.g. a ray-query service with its capabilities: closest or any
   hit, LOD, step counts; a storage; a face set; a hit record / visibility buffer; an
   image; history), each on a device. A pipeline is a graph of component instances
   (each with its own knobs) and edges; it is valid when every input is connected to a
   compatible output (capabilities met natively or by an adapter, devices joined by a
   communication path), with no fixed list of stages. Every ray-casting site is an input
   port of the component that casts (primary rays of ray casting, discovery's rays and
   exposure tests, shading's shadow and bounce rays, later AO), bound to its own kernel
   and storage nodes or to shared ones. Sub-steps:
   - 9a: the graph model in the registry, validation with reasons, a canonical
     serialisation (hashable: the cache key), today's flat compositions and old records
     loading as graphs; no behaviour change.
   - 9b: GPU shader assembly per graph through an existing module system (`naga_oil` or
     WESL): each node instance its own module, so two instances of one storage or kernel
     need no renaming pass; replaces text-joining and `traversal::namespaced`.
   - 9c: back ends consume graphs: every ray-casting site of every back end binds its own
     kernel and storage (the 7c shadow stages become two such ports).
   - 9d: GUI (the stage grid becomes a tree: each port expands into the node bound to it,
     shared nodes shown as shared), CLI form, labels, sweeps over graphs (bounded
     enumeration), observations and `sweepcmp.py` keyed by the canonical graph.
   On the CPU, arbitrary graphs cannot all be precompiled: generic precompiled builds
   cover a common set, other graphs run compiled on demand (step 8's machinery; the
   user's intent that every pipeline is eventually compiled at run time).
10. **Composition fully data:** knobs declared as baked-in (specialisable) or live; CPU
    components declare capabilities (associated consts, as `backend_cpu::Spec`); the
    graph hash is the compile cache key everywhere.
11. **Per-stage measurement:** every node reports its own GPU time (timestamps per pass)
    and CPU time; observations carry them.
12. **A common visibility buffer:** every strategy outputs one hit record; shading
    placement (inline or deferred) a choice for every strategy; meshes a surface source
    writing into it, no longer a special stage.
13. **Strategies as components, back ends as executors:** discovery's pieces (sampler,
    hole detection, retention, drawer), splat and columns become components; back ends
    become executors placing nodes on devices (a device per node), so cpu-discover is a
    configuration of CPU and GPU nodes, and its `dyn World` call goes.
14. **History and post as nodes:** earlier frames' products as explicit inputs; tone
    mapping and diagnostic views with colour scales written once.
15. **Compiled on demand everywhere:** every CPU graph (generated Rust), and on the GPU
    specialised constants and persistent pipeline caches; then the break-in layers.

## Estimate (2026-10-12)

Working time (implementation plus verification runs; my estimate from steps 6–8, which
took about 1.5–2.5 hours each), with wide uncertainty on the large steps:

| step | estimate |
|---|---|
| 9 graphs (9a model 3–5 h, 9b module system 4–8 h, 9c back ends 4–6 h, 9d GUI/CLI/sweeps 4–6 h) | 15–25 h |
| 10 composition fully data | 3–5 h |
| 11 per-stage measurement | 3–5 h |
| 12 common visibility buffer, meshes as surfaces | 10–20 h |
| 13 strategies as components, back ends as executors | 15–30 h |
| 14 history and post | 4–8 h |
| 15 compiled on demand everywhere | 8–15 h |
| **total** | **about 60–110 h** |

## How we work

- One step (or sub-step) at a time: implement, verify, commit and push, report, wait for
  the go. The GUI may be unusable for a few days within a step.
- Zero cost is checked every step: a composed pipeline is as fast as a hand-written one of
  the same pieces; existing pipelines keep their pixels and speed (A/B tools below).
- Generalise from real users of an interface; but the graph model itself has no fixed
  limits (the user's requirement).
- **Adapters** are compile-time code giving a component a capability it lacks (never
  "fallback"); combinations without a valid path do not exist. **Redundancy** means the
  same algorithm the same way; fundamentally different implementations are not redundant.
- Existing compilers only; no compiler or JIT of our own.
- Repository https://github.com/ElipticalDrive/voxel-bench (public, `main`, repo-local
  noreply identity); commit and push freely and say so. No subagents until re-enabled.
  Ideas the user shares go into `IDEAS.md`; implement only after asking.
- Report honestly, including negative results and mistakes.

## How each step is verified

- Builds, zero warnings: `cargo build` and `cargo build --release`.
  `.cargo/config.toml` forces code alignment (2026-10-12): unaligned builds moved host-bound
  fps 5–8% from layout alone. A/B comparisons need both binaries built this way.
- Architecture guard: `cargo test -q --test architecture` (part of the workspace tests):
  no WGSL function defined twice across back ends or redefining shared code, no copied
  bodies under other names, lighting only through the shading component; exceptions are
  listed in `tests/architecture.rs` with their reasons.
- Unit tests: `cargo test -q --workspace` (49, including: the component registry
  matches its implementations; storages are exact; the CPU walks agree; every shipped
  plan loads).
- GPU end-to-end tests: `cargo build --release && cargo test -q --release --bin multirenderer -- --ignored --skip tuner_demo --skip scale_curve --skip gui_screenshot` (4: the live session, mesh mode, the headless GUI, and a compiled pipeline hot-swapping in, which needs the release binary).
- GUI screenshot (to look at layout changes without a display):
  `cargo test -q --release --bin multirenderer gui_screenshot -- --ignored` writes
  `target/gui-shot.png`; `SHOT_PLAN=plans/x.toml` picks the plan.
- `multirenderer list`: back ends with their slots and defaults, components by slot,
  standalone renderers (cpu-reference).
- One composition against the reference:
  `./target/release/multirenderer run plans/terrain.toml -s 'renderer=[{compose={backend="gpu-compute",storage="prefix",visibility="refloor"}}]' -s output.dir=\"target/check\"`
  - Further knobs go in `options={...}`. The reference: `{name="cpu-reference"}`.
  - CLI: `-r gpu-compute:prefix+incremental`, `-r cpu:hier+refloor`,
    `-r gpu-raster:bricks+cubes+visbuffer` (the back end's required slots in order, then
    optionally shading and prepass), or a bare back end (`-r gpu-discover`).
  - Use `-s run.repeats=3` for interleaved repeats, `-s run.frames=N` (more frames
    means a slower camera on the same path), and `-s run.modes=[\"unlit\"]`.
  - Results are in `<dir>/<stamp>/report.json` (a list of passes with `per_frame`
    counters).
- CPU performance: pin to one CCD (`taskset -c 0-7,16-23`) and read the `perf=true`
  hardware counters of the `cpu` back end (`cycles_per_px`, `instructions_per_px`, `lane_steps_per_pixel`)
  rather than fps alone.
- Whole catalogue: `scripts/suite.sh`, then
  `python3 scripts/summarise.py target/catalogue`.
- Tuner on real terrain: `ALGO=bayes TARGET_FPS=900 SECONDS=60 cargo test --release --bin multirenderer tuner_demo -- --ignored --nocapture`.

## Progress log (steps 9–15)

(Entries are added as steps land; steps 1–8 are in the archive.)
