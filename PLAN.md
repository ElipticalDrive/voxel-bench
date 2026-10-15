# Plan

**THE PLAN (2026-10-12; step 14 added 2026-10-14): finish the architecture, steps 9–16 below. It is the only thing
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
  discovery, shadow, bounce, mesh shadows, later AO; exposure tests pick their storage),
  independently or shared.
- **Every node picks its device and API** (IDEAS §21): a knob per node, `device` (a slot
  of the run's device configuration: `cpu`, `gpu`, `gpu1`, ...) and `api` (how it runs
  there: e.g. wgpu/Vulkan compute, wgpu raster, ray queries, scalar, AVX2, AVX-512), so
  each component runs where and how it suits its work and the distribution of work.
  Components declare their implementations per (device kind, API); a choice without one
  is greyed with the reason. Fused data (storages, kernels) runs with its caller; any
  other edge may cross devices, through a compile-time transfer adapter with its known,
  accepted cost. A node runs on one device (indivisible); splittable nodes are a possible
  later extension, not part of this plan.
- **Nodes exchange state freely, both ways** (IDEAS §22): edges may run in both directions
  and form cycles; each says when its value arrives (this frame, through iteration or a
  fused pair; or the previous frame). Fused nodes cost nothing, above all within one device
  and API, and the executor fuses whatever it can.
- **No redundant implementations** (IDEAS §16, §22): the same algorithm implemented once per
  device / API or mechanism is necessary, not redundant; redundant is an algorithm written
  again to accommodate some combination of features. Combinations are chosen and compiled
  in, never hand-written per combination.
- **Every renderer fits the graph** (else the space cannot be explored); a pipeline may
  span devices, so two GPUs render one frame by splitting the pipeline between them
  (required; splitting a node over devices can wait).
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
  of what should be general. (9a–9c: graphs exist, and back ends bind every ray-casting
  site from them; `Slot` / `Composition` remain for the GUI, CLI, labels and sweeps until 9d.)
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
- **No realistic server / client line** (IDEAS §19): renderers already see only change
  messages over IPC, but the host streams per renderer, builds its LOD chunks, and the link
  is free (no latency, bandwidth, reordering; everything resident before the frame).
- **Shader assembly by text-joining** strained (7c's renaming pass); 9b replaced it with
  naga_oil modules for storages, kernels, ray services, lighting (back-end roots remain).

## The steps (in dependency order; each verified and zero-cost-checked, one at a time)
9. **Pipelines as graphs** (the foundation; replaces `Slot`, `Composition`'s fixed fields
   and per-back-end slot lists). Components declare typed *ports*: inputs they need and
   outputs they provide (e.g. a ray-query service with its capabilities: closest or any
   hit, LOD, step counts; a storage; a face set; a hit record / visibility buffer; an
   image; history), each on a device. A pipeline is a graph of component instances
   (each with its own knobs) and edges (the world and its change log is a node too, so
   step 14's link is just an edge); it is valid when every input is connected to a
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
    components declare capabilities (associated consts, as `backend_cpu::Spec`);
    components declare their implementations per (device kind, API) instead of per back
    end, and nodes carry `api` beside `device` (IDEAS §21); the graph hash is the compile
    cache key everywhere.
11. **Per-stage measurement:** every node reports its own GPU time (timestamps per pass)
    and CPU time; observations carry them.
12. **A common visibility buffer** (designed so transparency fits later, IDEAS §23:
    several hits per pixel or continued rays, an opaque-only query for shadow rays): every strategy outputs one hit record; shading
    placement (inline or deferred) a choice for every strategy; meshes a surface source
    writing into it, no longer a special stage.
13. **Strategies as components, back ends as executors** (first: design stateful nodes and
    two-way edges, and the executor's fusion and pipelining, IDEAS §22; needs step 11's
    timing to show executors cost nothing): discovery's pieces (sampler,
    hole detection, retention, drawer), splat and columns become components; back ends
    become executors placing nodes on devices (a device and API per node, IDEAS §21), with
    transfer adapters on edges that cross devices, so cpu-discover is a configuration of
    CPU and GPU nodes, two GPUs combine, and its `dyn World` call goes.
14. **The served split (IDEAS §19):** the world (generator, authoritative store, change
    log) becomes a node on a *server* device; a *link* is a graph edge, the bottleneck
    between them (the point is the separation, not realistic networking; latency,
    bandwidth, loss and reordering are knobs that stress an algorithm's resilience, zero
    cost as one setting); the
    *client* derives everything renderer-specific (storages, LOD levels) from one stream
    shared by all clients. Generator placement is a choice: streamed from the server, or
    on the client from a shared seed with the server sending only edits. The renderer
    contract becomes strict: change messages only, nothing guaranteed to have arrived.
    Measured: bytes on the wire, update cost per byte, time to the first complete frame,
    holes while streaming.
15. **History and post as nodes:** earlier frames' products as explicit inputs; tone
    mapping and diagnostic views with colour scales written once.
16. **Compiled on demand everywhere:** every CPU graph (generated Rust), and on the GPU
    specialised constants and persistent pipeline caches; then the break-in layers.

## Exit criteria (when the architecture is done; then optimisation starts)

- No hand-written back ends remain: executors run graphs, a device and API per node. (Every
  renderer then is a graph, cpu-discover included, and a pipeline split over two GPUs
  renders one frame: outcomes of the architecture, checked as such.)
- Stateful nodes and two-way edges work (incremental discovery, accumulation).
- The served split exists as a separation with a bottleneck between server and client.
- Every old composition renders identically and is no slower (pixel and timing A/B).
- No implementation exists to accommodate a combination of features: combinations are
  compiled in (one implementation per algorithm and device / API or mechanism).

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
| 14 served split: server, link, client (added 2026-10-14) | 8–15 h |
| 15 history and post | 4–8 h |
| 16 compiled on demand everywhere | 8–15 h |
| **total** | **about 70–125 h** |

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
- `multirenderer graph gpu-compute:prefix+incremental`: a composition as a graph (TOML for a
  plan's `[renderer.graph]`), its canonical form and cache key.
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

## Progress log (steps 9–16)

(Entries are added as steps land; steps 1–8 are in the archive.)

**2026-10-14, step 9a done: the graph model.** `components/src/graph.rs`:
- Nodes are component instances (`role`, `component`, optional `device`, `inputs` port →
  node, own `knobs`); a `Graph` is an executor (the back end, until step 13) plus nodes.
  Typed ports (`Data`: changes, storage, faces, ray starts, ray queries, hits, mesh
  surfaces, image); every ray-casting site is a `Rays` port of the caster (`primary`;
  discovery `rays` and `exposure`; shading `shadow` and `bounce`). The world is a node
  (implicit when left out; unbound `changes` inputs read it), so step 14's link is an edge.
- Validation with reasons: unknown components and ports, type mismatches, unbound
  required inputs, capability needs (native or adapter), one image output, cycles,
  nodes leading to no image, devices (storages and kernels are fused into their caller
  and run on its device; a fused node shared by callers on two devices is rejected).
- Canonical form (nodes renamed in visiting order from the output, devices resolved,
  knobs sorted) and a stable 64-bit key (FNV-1a): the cache key for 9d / step 10.
- `Graph::from_composition`: every flat composition (so every stored record, old forms
  included) loads as a graph; `same` secondary stages share nodes; cpu-discover's GPU
  shadow rays get their own nodes. `to_composition` is the executor's acceptance: a graph
  runs if its back end has a composition with exactly that graph (else the reason and
  the nearest composition), until 9c makes back ends consume graphs.
- Plans: `[renderer.graph]` instead of `compose` (node knobs become the back end's flat
  options; conflicting values of one knob rejected until 9c). CLI: `multirenderer graph
  <backend:a+b+...>` prints the graph (TOML), canonical form and key.
- Verified: all 16,975 valid compositions of all back ends round-trip (12,699 distinct
  graphs; compositions share a key only when they differ in spelling `same`); tests for
  renamed ids, a hand-written graph, every rejection reason, plan loading; a graph renderer
  and its `compose` twin give the same psnr and fps in a real run; builds without warnings,
  56 unit tests, architecture guard, 4 GPU end-to-end tests. No back-end or runtime code
  changed (load time only), so no pixel / timing A/B was needed.
- Bug found on the way (fixed): the step-8 trust check re-ran `current_exe()`, the test
  harness under `cargo test`, which saved no frames, so a fresh compiled pipeline was
  falsely rejected and the `.rejected` file stuck. It now uses `MR_WORKER_EXE` like the
  workers. It only showed because 9a changed the source hash (new cache key).
- Not yet: devices are inferred, not checked against where components have
  implementations (the executor check covers it today); knobs per node are flattened;
  the GUI, sweeps and observations still use compositions (9d).

**2026-10-14, step 9b done: shaders as modules (naga_oil).** `crates/mr-gpu/src/program.rs`,
`components/src/shaders.rs`:
- Module system chosen: **naga_oil 0.23** (Bevy's composer; built on naga 30.0.1, the same as
  our wgpu). WESL was ruled out: 0.4.3+ needs rustc 1.97.1 (this machine: 1.96 nightly; not
  changed), and 0.4.2 cannot resolve the ray-query built-ins gpu-rt uses.
- A `Program` is a set of modules (path, plain WGSL, dependencies); each module's
  `#import`s are generated from the names it uses, taken from the nearest dependency that
  declares them (its own first, breadth first), so a caller reaches only the kernel it is
  bound to. Composed into a naga module and handed to wgpu as IR (`ShaderSource::Naga`;
  wgpu feature `naga-ir`). `MR_DUMP_WGSL` now dumps the program text (modules + root).
- Every storage instance, kernel instance (its own `LOD`, `EXACT`, `LEVEL_MASK`), ray
  service (`occluded`, `strace` through one kernel) is its own module; storages bind at
  `@group(S_GROUP)`, a constant of the instance. Shared modules `mr::common`, `mr::voxels`,
  `mr::lighting` (its shadow rays through the bound ray service). gpu-compute's second walk
  is `shadow::storage`/`shadow::kernel` beside `primary::`: no renaming.
  `traversal::namespaced`, `secondary_wgsl`, `occlusion_wgsl`, `OCCLUDED_WGSL` and
  `voxels::shader_source` are gone; mesh passes take a program (`Occlusion { program, rays }`).
- naga_oil's limits, met: module-level names and struct members must not end in a digit
  (renamed in shared WGSL only: `hash3`→`hash_cell`, `aux4`→`aux_v4`, prefix's
  `morton3`/`spread3`, padding fields); ray queries cannot be in a module (naga cannot
  write them back), so gpu-rt's ray-query kernel stays in its root and overrides the
  program's virtual ray services `rt::rays` (naga_oil `virtual` / `override fn`); `enable`
  directives are hoisted above the imports.
- Still in back-end roots (not yet modules): each back end's own code and, except on
  gpu-compute and gpu-rt, the shading component and shading state (gpu-raster and the
  discovery drawers wrap shading per draw mode with `fragment_wgsl`); the prepass (reads
  gpu-compute's own `beam` bindings); draw and geometry WGSL. Converting them is
  mechanical (give each its bindings) and fits 9c / step 12.
- Verified: a headless `programs_compose` test per GPU back end composes and validates
  every combination (gpu-compute 512, gpu-raster, gpu-discover incl. every drawer, gpu-rt,
  gpu-voxelspace) plus every primary × secondary storage × kernel pairing; pixel A/B
  against the build before: 90/90 frames identical (14 compositions over all GPU back
  ends, lit + unlit, incl. separate shadow walks, pathtrace, beam, gbuffer, boxes), 30/30
  in mesh mode, 24/24 with LOD chunks; GPU-time A/B (3 rounds interleaved, 7 back ends, lit
  + unlit) within ±1.2% (one +2.6% on gpu-splat lit re-measured over 6 rounds: −1.3%,
  noise); builds without warnings, 63 unit tests, architecture guard (gpu-compute and
  gpu-rt shade through `shaders::shading`), 4 GPU end-to-end tests. Composing costs a few
  ms to ~25 ms per shader at creation (validation included), not per frame.
- `scripts/ab/wgsl.sh` (byte-identical dumps) cannot compare across 9b (the text changed
  by construction); it works again between later builds.

**2026-10-15, step 9c done: back ends consume graphs.** `components/src/graph.rs`,
`components/src/shaders.rs` (`Walks`), `components/src/storage/gpu_data.rs` (`StorageSet`):
- The host hands every back end its graph (`InitParams::graph_json`; `RendererSpec::pipeline`:
  the graph given, else the composition's); `graph::of_init` reads it (or builds it from the
  flat options), and back ends take each site's kernel and storage from `Checked::site`.
- Model fixes: discovery's exposure tests are a storage port (they look blocks up, they cast
  no rays; 9a had them as rays); meshes have a `shadow` ray port; unbound secondary rays
  follow the rays before them (shadow rays the strategy's own on the same device, bounces
  the shadow rays, mesh shadows the shading's), so implicit and explicit edges have one
  canonical form.
- Acceptance (`Graph::accept`, replacing the 9a round trip): each back end has a lead site
  (its strategy's rays, or a rasteriser's shadow rays: its storage and traversal stages) and
  binds the others freely (gpu-compute: shadow, bounce, mesh shadows; gpu-raster: bounce,
  mesh shadows; gpu-splat: bounce; gpu-discover: exposure storage, shadow, bounce;
  cpu-discover: GPU shadow and bounce), over the lead's storage only (`cpu`: its shadow
  kernel; one storage type per precompiled instance), or only to the lead (gpu-rt: one
  acceleration structure; cpu-discover's exposure: the discovery storage, its `dyn World`
  until step 13). Free sites' components must be implemented where their rays run; the rest
  (the skeleton) must be one of the back end's compositions. It returns the nearest
  composition (labels, flat options) and whether it is exact; inexact graphs are labelled
  `<composition> [graph <key>]`. Knobs of free-site nodes stay per node (`exact`, `skip*`
  per kernel instance), the skeleton's flatten as before.
- GPU: `Walks` gives one module per storage node and per kernel node (shared nodes shared),
  each storage at its own bind group (gpu-compute 2, 4, 5, 6; drawer back ends 2, then after
  the deferred images at 3), and per-site ray services `rays::shadow` (`occluded`) and
  `rays::bounce` (`strace`; without a bounce site, through the shadow kernel, so a live
  switch to pathtrace works). `StorageSet` keeps one `StorageData` per storage instance;
  face drawers bind the extra groups (`DrawFrame::more`). Mesh shadows compile their own
  site's walk (`Walks::shadow_program`). CPU: the `cpu` back end's shadow rays may use a
  kernel node of their own (`Scene::shadow`). gpu-rt, the voxelspace back ends and
  cpu-raster read no sites (their only rays are their own).
- Verified: builds without warnings, 64 unit tests (new: `free_sites`; every composition
  still round-trips exactly), architecture guard, 4 GPU end-to-end tests; headless
  `programs_compose` per GPU back end now also composes graphs with bounces, mesh shadows,
  exposure and shadow walks of their own. Pixel A/B against the build before: 102/102
  frames identical (16 compositions over all 10 back ends, lit + unlit), 36/36 in mesh
  mode, 24/24 with LOD chunks. GPU-time A/B (3 rounds interleaved, 10 compositions over 7
  GPU back ends, lit + unlit) within ±0.4%; `cpu` fps (pinned) within −0.4…+1.6%. Six
  graphs no composition expresses run on terrain against the reference (gpu-compute with
  three walks, and two instances of one kernel with different knobs; gpu-raster bounces;
  gpu-discover exposure, discovery and shadow rays on three storages; cpu-discover GPU
  bounces; `cpu` shadow kernel): each matches its nearest composition's psnr and bad
  pixels (the kernels are exact, so the hits agree).
- Small changes in what runs: a separate shadow kernel over the primary storage no longer
  binds that storage twice (one storage module, group 2 only); mesh shadows sharing the
  primary kernel use its constants (before: the kernel's defaults; differs only in mesh
  mode with non-default `incremental` knobs).
- Seen on the way (pre-existing, same on the build before, not fixed: not blocking): a
  `distance` storage caps the terrain flight at ~40 fps while its CPU and GPU frame times
  are as low as `bricks` (host-side side-data build per streamed chunk, presumably).
- Not yet: the GUI, CLI form, sweeps and observations still use compositions (9d); the
  `cpu` back end's shadow rays over another storage need a compiled pipeline (step 16).

