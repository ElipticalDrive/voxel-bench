# Plan

**THE PLAN (2026-10-12; step 14 added 2026-10-14, step 17 2026-10-16): finish the architecture, steps 9–17 below. It is the only thing
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
- **A feature costs only the pipelines that use it** (IDEAS §23): supporting more features
  (transparency, layers, effects) must add no run-time cost to pipelines without them.
  Features are capabilities compiled in where chosen and absent elsewhere (no runtime
  flags in hot paths, no wider records or extra passes for everyone).
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
  of what should be general. (Step 9 done: graphs exist, back ends bind every ray-casting
  site from them, and the GUI, CLI, plans, labels, sweeps and observations speak graphs
  through the sites form. `Slot` / `Composition` remain as the skeleton a back end
  accepts until step 13 makes back ends executors.)
- **Strategies inside back ends:** discovery (quadtree, cluster and their pieces), splat
  and voxelspace columns are back-end code, not components; the ten back ends are
  hand-written assemblers, not executors placing pieces on devices (§18).
- **No common visibility buffer:** strategies do not share one hit record / visibility
  buffer; inline or deferred shading is a choice only where a face drawer exists; meshes
  are a special stage instead of a surface source (§18). (Step 12 done: one hit record with
  optional capabilities, placement a choice for every strategy, meshes write records.)
- **No per-stage devices:** cpu-discover (CPU discovery, GPU drawing) is a hard-wired back
  end, not a configuration; two GPUs cannot be combined (requirement 2).
- **No history and no post stages** (§18 pieces 7 and 8).
- **No per-stage measurement** (§18: every piece reports its own time; the planner needs it).
  (Step 11 done: every frame reports its stages, their nodes and CPU / GPU times.)
- **Only the `cpu` back end compiles on demand** (§17; step 16). (Step 10 done: knobs are
  declared specialisable or not, CPU components declare capabilities, components declare
  implementations per device kind and API, nodes carry `api`, the graph key is the compile
  cache key.)
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
    several hits per pixel or continued rays, an opaque-only query for shadow rays, all
    as optional capabilities: opaque-only pipelines compile without them and cost nothing
    more): every strategy outputs one hit record; shading
    placement (inline or deferred) a choice for every strategy; meshes a surface source
    writing into it, no longer a special stage. Sub-steps (2026-10-16):
    - 12a: the stored hit record defined once (`pack_hit` / `record_surface` in
      `mr::voxels`: distance + block + face + cell parity, 8 bytes, high bits reserved for
      optional capabilities) and one deferred shading pass for every strategy
      (`shading/deferred.wgsl`); the face drawer's `gbuffer` rebuilt on them. (Done 2026-10-16.)
    - 12b: placement a choice for ray casting (gpu-compute, gpu-rt, `cpu`): the walk writes
      records, the common pass shades; the CPU record and shade loop. (Done 2026-10-17.)
    - 12c: the other strategies on the record (splat's resolve, columns on GPU and CPU,
      cpu-raster's buffer, `visbuffer` as the face-id encoding decoded by its face source),
      with placement wherever a strategy lacks it. (Done 2026-10-17.)
    - 12d: meshes as a surface source writing records (a mesh capability in the reserved
      bits, compiled in only with meshes), replacing `MeshComposite` and the drawer's
      after-shading mesh pass; transparency's capabilities (layers, continued rays, the
      opaque-only query) named in the registry, built after the architecture. (Done
      2026-10-17.)
13. **Strategies as components, back ends as executors** (first: design stateful nodes and
    two-way edges, and the executor's fusion and pipelining, IDEAS §22; needs step 11's
    timing to show executors cost nothing): discovery's pieces (sampler,
    hole detection, retention, drawer), splat and columns become components; back ends
    become executors placing nodes on devices (a device and API per node, IDEAS §21), with
    transfer adapters on edges that cross devices, so cpu-discover is a configuration of
    CPU and GPU nodes, two GPUs combine, and its `dyn World` call goes.
    **The design (2026-10-17):**
    - *Edges have a timing* (a node's `timing`, per input; default `now`): `now` = this
      frame's value (the producer runs first, or is compiled in); `prev` = the previous
      frame's value, kept by the executor (closes cycles over frames; between devices it
      lets them overlap a frame: pipelining); `iterate = n` = within the frame, closing a
      cycle: the consumer runs n times, first on the producer's previous-frame value, the
      nodes between n - 1 times. Fused data (storages, ray queries) is always `now`. A cycle
      of `now` edges, or an `iterate` edge closing none, is invalid with the reason. Today's
      knobs are timings: cpu-discover's `pipelined` = the drawer's faces `prev`;
      gpu-discover's `incremental` = discovery's `covered` input from the drawer with the
      drawer's faces iterating twice (draw last frame's faces, discover the holes, draw);
      its `still_holes = late` = `covered` read `prev` instead.
    - *Stateful nodes* declare what they keep across frames (`graph::state`: storages,
      geometry, discovered face sets, path-tracing accumulation, acceleration structures).
      The executor owns it per instance on the node's device and resets it on resize or a
      structural change. State only the node reads is not an edge; another node reads it
      through an output edge `prev`.
    - *The schedule* (`Checked::schedule`, computed once at creation: no per-frame cost):
      passes in frame order, each a node doing work of its own with the fused nodes it
      reads compiled in (kernels; a storage is both an update pass and compiled-in data;
      `rayquery` builds its structures as a pass); loops for iterations; a transfer
      adapter per non-fused edge between devices (upload, readback, peer copy), with its
      timing; kept values for `prev` / `iterate` edges. `multirenderer graph` prints it.
    - *Fusion*: beyond fused data, adjacent passes on one device and API joined by a `now`
      edge run as one where the consumer can run inline (shading `placement = inline` in
      ray casting or raster), else as separate passes in one submission with buffers kept
      across frames. Fusion is decided when the pipeline is compiled (naga_oil modules and
      shader defs, Rust monomorphisation), never a run-time branch, and only where it is
      no slower than the hand-written equivalent (checked with step 11's stage times).
    - *Executors*: one per device kind (GPU: wgpu; CPU), each a library of pass
      implementations per (component, API) driven by the schedule; lanes per device; a
      pipeline over two devices is the schedule's transfers, so cpu-discover and two GPUs
      are configurations, not back ends.
    Sub-steps:
    - 13a: the model: timings on edges, cycles with reasons, stateful nodes, the
      schedule (passes, loops, transfers, kept values), printed by `multirenderer graph`;
      back ends reject timings with the reason until they run them; no behaviour change,
      every existing graph key unchanged. (Done 2026-10-17.)
    - 13b: discovery's pieces as components (sampler `quadtree` / `cluster`, hole
      detection, retention, the drawer), CPU and GPU; `incremental`, `still_holes` and
      `pipelined` become the timings above (old records load as the graphs they mean).
    - 13c: one discovery executor: gpu-discover and cpu-discover become one, discovery on
      `cpu` or `gpu` by its node's device, transfers from the schedule, the CPU storage
      monomorphised (no `dyn World`).
    - 13d: splat and columns as components; the voxelspace back ends as configurations.
    - 13e: GPU and CPU executors run every graph from the schedule (ray casting, raster,
      ray queries, splat, columns, discovery); hand-written back ends removed; a pipeline
      split over `gpu` and `gpu1` through a transfer adapter; pixel and timing A/B of every
      old composition.
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
17. **Windows and Linux both build and run** (user, 2026-10-16; after step 16, before
    optimisation and bug fixes): today only Linux compiles. Known Unix-only code: the
    worker IPC (`std::os::unix::net` Unix sockets in `mr-ipc`, `mr-serve`, `src/worker.rs`),
    shared-memory frame slots under `/dev/shm` (`mr-ipc`), the `cpu` back end's hardware
    counters (`perf_event_open` through `libc`, `backends/cpu/src/perf.rs`), and the
    compiled-on-demand pipelines (dynamic libraries, toolchain paths, `.so` names;
    `src/pipelines.rs`) plus shell scripts used by tools. Each gets a portable form or a
    per-OS implementation behind one interface (e.g. local sockets / named pipes, a named
    file mapping, Windows performance counters or the knob greyed with the reason), with
    no cost to the Linux build. Verified by cross-compiling for `x86_64-pc-windows-msvc`
    or `-gnu` (zero warnings) and, where a Windows machine is available, running the tests.

## Exit criteria (when the architecture is done; then optimisation starts)

- No hand-written back ends remain: executors run graphs, a device and API per node. (Every
  renderer then is a graph, cpu-discover included, and a pipeline split over two GPUs
  renders one frame: outcomes of the architecture, checked as such.)
- Stateful nodes and two-way edges work (incremental discovery, accumulation).
- The served split exists as a separation with a bottleneck between server and client.
- Every old composition renders identically and is no slower (pixel and timing A/B).
- No implementation exists to accommodate a combination of features: combinations are
  compiled in (one implementation per algorithm and device / API or mechanism).
- Windows and Linux both build (step 17).

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
| 17 Windows and Linux builds (added 2026-10-16) | 4–8 h |
| **total** | **about 75–135 h** |

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

**2026-10-16, step 9d done (step 9 complete): the GUI, CLI, plans, sweeps and observations
speak graphs.** `components/src/graph.rs` (sites form), `src/plan.rs`, `src/gui/stages.rs`:
- **Sites form**: a graph written as its flat composition plus the sites it binds otherwise
  (`Graph::sites_form`, `with_sites`, `site_values`, `site_choices`). Sites are named by port
  (`primary`, `rays`, `exposure`, `shadow`, `bounce`, `meshes-shadow`); a value is another
  site (shared), `<kernel>` (its own kernel over the lead's storage), `<storage>/<kernel>`
  or `<site>/<kernel>`, and for exposure tests `<storage>` or a site. Unlisted sites follow
  the rays before them, as unbound ports do. Graphs whose own kernels carry knobs have no
  sites form and stay graphs.
- **CLI**: `-r 'gpu-compute:bricks+refloor+pathtrace,bounce=tree64/sphere'`; `multirenderer
  graph <renderer>` also prints the sites form; `graph --keys` gives old observation
  records their graph key. **Plans**: `sites = { bounce = "tree64/sphere" }`; a
  `[renderer.graph]` is normalised to the sites form when it has one. Bug fixed on the
  way: saving a plan wrote a graph renderer as its nearest composition (the graph was lost);
  it is now saved as the graph.
- **Labels**: `gpu-compute: bricks + refloor + pathtrace, bounce = tree64/sphere`; only graphs
  without a sites form keep `[graph <key>]`.
- **Sweeps over graphs**: `sites = { bounce = "*" }` or a list, crossed with `compose`
  sweeps, bounded (at most 100,000 per composition), each distinct graph once (by key);
  `"*"` = the other sites, each kernel over the lead's storage, each storage × kernel. Unlit
  passes drop shadow / bounce / mesh-shadow sites as moot (`RendererSpec::effective`).
- **Observations** (version 3) carry `sites`, `graph` (when there is no sites form) and
  `graph_key`; `scripts/ab/sweepcmp.py` keys pipelines by graph key (older records keyed
  through `graph --keys`).
- **GUI**: the stage grid is a tree from the output: each port expands into its node
  (component picker and its knobs beside it), nodes reached twice show as shared, off-lead
  sites pick "shared: <site>" or "own kernel" with kernel and storage pickers below; every
  choice is listed and greyed with the reason it cannot run. Stages the tree does not reach
  (no prepass, no meshes) follow as rows. Live: site edits hot-swap the renderer.
- Verified: builds without warnings, 67 unit tests (new: every site binding one or two away
  from each back end's compositions round-trips through the sites form, 930 bindings;
  sites in plans, CLI and sweeps; graph plans save and reload), architecture guard, 4 GPU
  end-to-end tests; GUI screenshot with a sites renderer running live; a terrain run of
  gpu-compute bounces over tree64/sphere, gpu-raster bounces over prefix/incremental and
  `cpu` with its own shadow kernel: each matches its nearest composition's psnr and bad
  pixels; the 2026-10-11 baseline sweep's 3047 records all get graph keys (self-comparison
  100%). Back-end and shader code unchanged, so no pixel / timing A/B was needed.
- Not yet: knobs of a site's own kernel are edited in the plan's graph form only (the GUI
  shows them as such); the catalogue still groups by composition (the nearest).

**2026-10-16, step 10 done: composition fully data.** `components/src/impls.rs`,
`components/src/lib.rs` (registry), `components/src/graph.rs`, `crates/mr-core/src/params.rs`,
`src/pipelines.rs`:
- **Implementations per device kind and API** (10a): every component lists its
  implementations (`Impl`: `gpu`/`cpu`, the APIs it runs under, the capabilities it provides
  there) instead of the back ends it is on. APIs: `compute`, `raster`, `rayquery` (GPU);
  `scalar`, `avx2`, `avx512` (CPU). WGSL modules (storages, kernels, shading) run under any
  GPU API; generic Rust under every ISA; geometry builders are CPU code (scalar). Back ends
  declare where they run each slot (`runs` / `at`: a `Place`, a kind and its APIs), what
  their own hand-written code hosts (`hosts`: their strategy, the geometry their drawer
  reads, their mesh drawing; executors lift these in step 13), the storage interface they
  read (`reads`), and their API choice (`api_choice`: `cpu`'s `isa`, and gpu-compute's
  `dispatch`, found to be one: its walk in a compute or fragment shader). A component fills
  a slot if it has an implementation of the place's kind covering its APIs. Availability is
  exactly as before (compared against the old lists), except: cpu-discover's GPU shadow
  stages are now its own places (before: borrowed from gpu-discover through `rays`, same
  result).
- **Capabilities per implementation**: `bricks` has `lod` on the GPU, not the CPU. Two new
  ones name the storage interfaces, `voxels` (`s_probe`: walks, exposure tests, prepasses)
  and `heights` (the column march); kernels, discovery and beam need `voxels`, columns
  `heights` (no adapter between them yet), so a graph putting a heightmap under a walk is
  rejected with that reason.
- **CPU capabilities as associated consts** (10b): `Repr::CAPS` (every CPU storage: `voxels`)
  and a `Kernel` trait with unit types `refloor::cpu::Refloor`, `incremental::cpu::Incremental`
  (`steps`); the registry reads them (`<Prefix as Repr>::CAPS`), so they are declared once.
- **Nodes carry `api`** beside `device`: checked against the device's kind, the component's
  implementations and the executor (an API a back end does not run that stage with is
  rejected with the reason; a face drawer's shading runs `raster` with the `raster` drawer,
  else `compute`); fused nodes inherit their callers' API when all agree (a kernel shared by
  gpu-compute's compute walk and its raster mesh shadows is compiled into both). Where the
  back end offers a choice, the nodes' API is its knob (`isa = "avx2"`, `dispatch =
  "fragment"`) and enters the canonical form unless it is the choice's default; elsewhere it
  says nothing new and stays out, so keys are unchanged. Devices default from the places:
  **geometry now runs on the CPU** in the graphs of gpu-raster, gpu-rt and gpu-splat (true:
  faces are built on the CPU), which changes those graphs' keys; `sweepcmp.py` now
  recomputes every record's key. The GUI tree's hover says where each node runs; `list`
  shows implementations and places.
- **Knobs declared specialisable** (10c): `ParamSpec::baked`: switches and choices select code
  and are specialisable by default, numbers are quantities read at run time; overridden where
  that is wrong (`bounces`, a loop count: baked; gpu-rt's `fast_build`, a build flag: not).
  The GUI's knob hover says so. Independent of `live` (the generic builds still change a
  baked knob live; a compiled pipeline refuses, as before).
- **The graph key is the compile cache key** (10d): a compiled `cpu` pipeline is the renderer's
  graph with the values of the knobs its generated code bakes (`BAKES`: `exact`, `packet`,
  each checked to be declared specialisable) placed on the nodes they belong to
  (`Graph::with_knobs`; a knob whose node is absent is moot and left out, so `packet` no
  longer splits incremental pipelines); key = `cpu-<graph key>-<source/toolchain/CPU hash>`.
  The trust check renders that graph twice (generic and compiled).
- Verified: builds without warnings, 69 unit tests (new: `apis`, `the_graph_is_the_key`;
  registry consistency now checks the CPU capabilities against their consts; every
  composition still round-trips), architecture guard, 4 GPU end-to-end tests (a pipeline
  compiled and trusted under the new key); a terrain run with APIs set in graphs (`cpu`
  nodes `avx512`, gpu-compute's walk `raster`) matches the flat-knob twins (the fragment
  graph has the fragment twin's psnr, 58.9, not compute's 58.6). Render code unchanged
  (except a compiled incremental pipeline's packet width, now unset where unused), so no
  pixel / timing A/B was needed.
- Not yet: executor-level knobs without a node (e.g. `cpu`'s `view`, `schedule`) cannot be
  baked into a graph (none is baked today; step 16 decides where they live); GPU builds
  have no compile cache yet (step 16); `hosts` lists remain until step 13.

**2026-10-16, step 11 done: per-stage measurement.** `crates/mr-core/src/renderer.rs`
(`StageTime`, `FrameStats::stages`), `crates/mr-gpu/src/lib.rs` (`FrameRing::stage`),
`components/src/draw/drawer.rs` (`DrawNodes`), `src/report.rs` (`StageSummary`, `stage_table`):
- **A stage** is a timed part of a frame and the graph nodes it runs: callers only, since
  storages and kernels are fused into their callers and timed with them (`Checked::ids`).
  Inline-fused work is one stage (`cast+shade` = visibility + shading); nodes in separate
  passes are separate stages. Work no node owns has no nodes (gpu-discover's `view`
  overlay and `audit`, cpu-voxelspace's `transpose`).
- **GPU stages**: `FrameRing::stage(enc, ticket, name, nodes)` writes a timestamp between
  the commands before and after (an empty compute pass; up to 15 stages a frame); the last
  stage ends at submit; times are read back with the frame. **CPU stages**: timed by the
  back end (`StageTime::cpu`). A stage may have both (gpu-rt's `geometry`: BLAS inputs on
  the CPU, builds on the GPU).
- Every back end: gpu-compute `meshes`, `prepass`, `cast+shade`; gpu-raster `geometry` and
  `cull` (CPU), then the drawer's `draw+shade` (forward, meshes included) or `draw`,
  `shade`, `meshes` (deferred); gpu-discover `discover`, `merge`, `sort`, the drawer's,
  `sky mask`, `view`, `audit`; cpu-discover `discover` (CPU) and the drawer's; gpu-rt
  `geometry`, `cast+shade`; gpu-splat `splat+shade`; gpu-voxelspace `columns+shade`; `cpu`
  `storage` (grid rebuilds, some frames) and `cast+shade`; cpu-raster `geometry`,
  `transform`, `raster`, `shade`; cpu-voxelspace `columns+shade`, `transpose`. Stage times
  add up to the frame's GPU time (e.g. gpu-compute + beam: 0.388 + 0.696 vs 1.08 ms).
- Reports: `per_frame[].stages`, pass `stages` (summaries over the frames that ran each,
  CPU and GPU parts and repeats summed per frame), a stage table after the run summary,
  `.agent/bin/report` prints them. Observations version 4 carry `stages` (mean ms, nodes
  and what each is, `role.component`). The live GUI shows smoothed stage rows in the stats
  (hover: the nodes).
- `run.stage_times` (default true): GPU stages cost a timestamp each, measured +0.1–0.6% GPU
  time; `false` writes no marks (GPU time then identical to the build before, ±0.1%). CPU
  stages are always reported.
- Verified: builds without warnings, 71 unit tests (new: stage summaries; a postcard round
  trip of frame stats with stages: a `skip_serializing_if` there broke every worker, caught
  on the first run), 4 GPU end-to-end tests (now asserting stages on gpu-compute,
  gpu-raster, and meshes in the forward drawer's stage); runs exercising every stage path
  (meshes, deferred meshes, incremental, merge, painter, views, audit); pixel A/B 72/72
  frames identical (all 10 back ends, lit + unlit), 18/18 in mesh mode; GUI screenshot.
- **Timing A/B, an unresolved loss**: GPU time within −0.4…+0.7% (all GPU back ends);
  fps of CPU back ends within ±0.8%, of GPU back ends within noise except the two whose
  frame is bound by CPU geometry building: gpu-raster −1.3…−4.6% and gpu-rt −1.6…−2.3% fps
  over seven A/Bs (A/A noise ±1.5%). The extra time is in their remesh / BLAS-input code
  (same chunks per frame, +2–4% per chunk), whose hot function (`mr_mesh::mesh_chunk_merge`)
  is instruction-identical in both builds. Not the marks (stages on vs off: no difference),
  not the shared infrastructure (a build with it and the old back ends: no loss), not a
  relink (the old build with a meaningless change: no loss; the new code with it: loss).
  Removing the per-frame CPU stage records from those two back ends recovered about half,
  within noise; their execution costs well under a microsecond against a ~100 µs loss, so
  it is presumably how the surrounding frame code compiles. Without a profiler on this
  machine the bisect could not go further. `scripts/ab/perf.sh` gained `AFTER_BIN` for
  such probes.
- Not yet: per-node update cost (world deltas into storages and geometry) is measured per
  frame as a whole (`update_ms`), not per node; stages inside one fused pass cannot be
  split (they are one node set by construction).

**2026-10-16, step 12a: the common hit record and the deferred shading pass.**
`crates/mr-gpu/src/voxels.wgsl` (`pack_hit`, `record_surface`), `components/src/shading/deferred.wgsl`,
`components/src/draw/{gbuffer/gpu.wgsl,faces.wgsl,drawer.rs}`:
- **The record** (GPU): one rg32uint texel per pixel: `t` along the pixel's unit primary ray
  (f32 bits) and an id word: 0 = sky, else bit 0, face (bits 1-3), block (bits 4-19); bits
  20-31 are for optional capabilities (mesh surfaces, transparency's layers) and stay 0 in
  pipelines without them. `record_surface(r, o, d)` rebuilds the `Surface` as
  `face_plane_hit` does (cell's normal axis from the face plane, the others from the point;
  one block per cell whatever the LOD, as `surface_of` tiles textures).
- **The deferred pass**, written once: a compute thread per pixel reads the record, rebuilds
  the surface and calls the shading component. A back end provides `HITS_GROUP` and its view
  (`view_ray`, `view_origin`, `view_sky`, `view_lit`); the face drawer's are in `faces.wgsl`.
- **`gbuffer` rebuilt on it**: the raster pass writes `pack_hit` (8 bytes per pixel, one
  target) instead of the hit point, face and block (20 bytes, two targets); its own shade
  entry is gone. `visbuffer` keeps its face-id encoding for now (12c).
- Verified: builds without warnings, 71 unit tests, 4 GPU end-to-end tests. Pixel A/B: raster
  and visbuffer identical (gpu-raster, gpu-discover; lit + unlit); gbuffer differs at 80-220
  grazing pixels per frame on all three drawer back ends, because `t` is now stored rather
  than re-derived from the stored point: psnr against the reference equal or up 0.1 dB
  (gpu-raster unlit 52.8 -> 52.9, now equal to forward `raster`; gpu-discover lit 49.3 ->
  49.4), and the gbuffer image is closer to forward `raster`'s (125-169 differing pixels,
  down from 148-224; the three drawers inherently differ by 150-250 such pixels). Mesh mode
  (gpu-raster gbuffer): psnr unchanged (63.5 / 71.2). Timing A/B (3 rounds, 300 frames):
  gbuffer GPU time -5.2% lit / -10.8% unlit on gpu-raster, -3.6% / -4.2% on gpu-discover,
  -48% / -68% on cpu-discover, whose face set has much overdraw (its draw stage 0.73 -> 0.19 ms:
  8 bytes written per fragment instead of 20); raster and visbuffer within ±0.2%.

**2026-10-17, step 12b: shading placement for ray casting.** `components/src/lib.rs`
(`placement_param`, `parse_placement`), `components/src/visibility/record.rs`,
`components/src/shading/deferred.wgsl`, `backends/{gpu-compute,gpu-rt}/src/deferred.wgsl`, `backends/cpu/src/lib.rs`:
- **`placement = inline | deferred`**, a knob of the shading node on gpu-compute, gpu-rt and
  `cpu` (baked; fixed at creation, so inline pipelines carry no record buffer, binding or
  pass). Deferred: the walk writes one record per pixel (a storage buffer on the GPU, so
  gpu-compute's `dispatch = "fragment"` writes them from fragments too), then the common
  pass shades. Stages `cast` and `shade` instead of `cast+shade`. `view = "steps"` shows
  the walk, so it runs inline whatever the placement. Like other knobs (`bounces`,
  `exact`), placement is in the options of reports and observations, not in labels or the
  structural graph key.
- **The record carries the cell's parity** (bits 20-22, one per axis; capabilities keep
  23-31): the point alone cannot tell a cell from its neighbour on block edges (3-88
  pixels per frame differed); with the parity the surface is rebuilt exactly. `pack_hit`
  now takes the `Surface` (the record is a packed surface). The deferred pass reads
  records through `record_at(px)` and shows `view_miss(px)` where nothing was hit
  (gpu-compute: the composited mesh, as inline).
- **The CPU record** (`Record`, same layout as the GPU's, so a record could cross devices)
  and the CPU deferred pass (`shade_records`). The `cpu` back end's rectangle renderer is
  monomorphised over placement (`const D`), chosen once per frame: no flag in the hot loop.
- Fixed on the way: `FrameRing::{compute,render}_timestamps` returned a timestamp write
  with neither index when asked for neither (wgpu rejects it); now `None`.
- Verified: builds without warnings, 72 unit tests (new: the CPU record round trip, edges
  included; every gpu-compute and gpu-rt composition's deferred shader composes and
  validates), architecture guard (the deferred pass's hooks registered as an interface), 4
  GPU end-to-end tests. Pixel A/B against the build before 12a: every inline pipeline
  identical (gpu-compute plain, pathtrace, beam, fragment; gpu-rt; `cpu` refloor and
  incremental; mesh mode); gbuffer as in 12a, now closer still to forward `raster` (74-151
  differing pixels per frame, from 80-169). Deferred against inline: `cpu` identical (12/12
  frames); gpu-compute and gpu-rt differ at 39-77 pixels per frame, the same when the
  inline path also packs and unpacks a record, so not the record but the walk compiled in
  two shaders (as `dispatch = fragment` against `compute`); psnr equal or +0.1 dB. Timing
  A/B, inline: GPU time within ±0.2% (gpu-compute plain, pathtrace, fragment; gpu-rt), `cpu`
  fps within ±0.7% (pinned; refloor, incremental, packets of 8 in tiles). Deferred against
  inline (interleaved, 3 repeats): gpu-compute +7-8% GPU time with `direct` (+3-5% with
  fragment dispatch), -2..-3% with lit `pathtrace` (its shading is 3/4 of the frame and
  runs more coherently on its own); gpu-rt +7-9%; `cpu` +2-4% frame time.

**2026-10-17, step 12c: every strategy on the record.** `components/src/shading/{deferred.wgsl,packed.wgsl,deferred.rs}`,
`components/src/draw/visbuffer/gpu.wgsl`, `backends/gpu-splat/src/splat.wgsl`,
`backends/{gpu-voxelspace,cpu-voxelspace,cpu-raster}/src/`, `crates/mr-gpu/src/program.rs` (`Program::define`):
- **Record encodings.** The deferred pass reads a record through its encoding: `record_at`,
  `record_hit`, `record_decode(r, d)` (the surface on the pixel's ray), `record_view(r, color)`
  (the source's diagnostic views), plus the back end's view (`view_ray`, `view_miss`, ...).
  The packed record (`pack_hit`) is one encoding (`shading/packed.wgsl`; the back end may
  refine the decoded surface in `view_surface`). `visbuffer`'s face ids are another, decoded by
  the face source (`face_hit`, `src_view`): its own shade entry is gone. gpu-splat's point ids
  likewise: its resolve *is* the common pass (stages `splat` and `shade` instead of
  `splat+shade`).
- **The CPU pass**, written once: `shading::deferred::{Records, shade}` (a trait monomorphised
  per strategy: record type, decode and shading call). Used by `cpu` (replacing
  `shade_records`), cpu-voxelspace (records) and cpu-raster (quad ids, its f64 reference shading).
- **Placement where it was missing**: gpu-voxelspace and cpu-voxelspace (`placement`, default
  inline: the march writes records, the common pass shades; the CPU records are
  column-major and the pass writes rows, so the transpose goes); cpu-raster (`placement`,
  default deferred as before; inline = forward shading of every fragment that passes the depth
  test). Splatting settles visibility only once every point is in, so it is deferred by
  construction (no knob). The face drawers already had it (`raster` or `visbuffer` / `gbuffer`).
- **Columns' surface is not exactly a record.** The march draws rows whose ray meets the top
  plane outside the marched column and clamps their texture coordinates there; the record's
  cell parity names only the point's cell or a neighbour. Deferred columns rebuild
  column-relative texture coordinates and the slope normal from the record's cell
  (`view_surface`), and differ from inline in texels at 8-19% of pixels (equally on GPU and
  CPU): PSNR against the reference 20.8 vs 20.9 dB unlit, 18.2 vs 18.2 lit (plain record
  decoding: 20.7).
- **A hook function cost 2.5% (log).** gpu-voxelspace's march first called a per-pixel hook
  (`column_hit`: shade inline, or write a record): unlit GPU time +1.0% against the build
  before (three A/Bs), and the old shader in the new build was equal, so it was the shader.
  Hand-inlining the same code: -1.4%. Building the surface inside each branch, or returning
  the colour from the hook, kept the +1.0%: on NVIDIA the call boundary in front of
  `shade_surface` costs. Fix: naga_oil shader defs (`Program::define`, `#ifdef DEFERRED`), one
  march body; then -0.4% unlit, +0.2% lit (noise).
- **Deferred columns on the GPU are 8.5x faster lit** (0.89 vs 7.6 ms; unlit 0.72 vs 1.14): the
  march runs one thread per screen column (1920 threads), so inline shading (shadow rays) runs
  serially down each column on a mostly idle GPU; the common pass spreads it over every pixel
  (stages: `columns` 0.67 ms, `shade` 0.08 / 0.24 ms). On the CPU deferred columns are slower
  (3.6 -> 4.7 ms unlit, 8.5 -> 9.6 lit: decode, heightmap re-reads). cpu-raster inline is
  slower than deferred (lit 80 vs 61 ms: overdraw shaded in f64) and pixel-identical to it.
- Verified: builds without warnings, 73 unit tests (new: gpu-splat's shader composes for every
  composition; gpu-voxelspace's for both placements), architecture guard (the record hooks
  registered as interface), 4 GPU end-to-end tests, GUI screenshot. Pixel A/B against the
  build before: 66/66 frames identical (all ten back ends, lit + unlit), deferred variants
  (gpu-compute, cpu, gpu-rt, gbuffer on gpu-raster and cpu-discover, visbuffer on gpu-discover)
  42/42, visbuffer diagnostic views (`quads`, `churn`) 18/18, mesh mode 24/24, room 30/30.
  Timing A/B (interleaved, 3-5 rounds): GPU time within ±0.5% (visbuffer and gbuffer drawers,
  gpu-compute deferred, gpu-voxelspace), gpu-splat -0.1% / +0.8% (the extra pass boundary);
  CPU pinned: `cpu` deferred ±0.2%, cpu-raster fps +0.7..+2.2% with its shade stage +1.2%
  unlit / equal lit (after `#[inline]` on the quad decode; +2% without). cpu-voxelspace got
  faster: march -15% unlit (4.46 -> 3.78 ms), -2% lit, fps +14.7%: the march is now a method
  monomorphised per output instead of one closure; the exact cause was not isolated (no
  profiler).
- Not yet: records carry no capability bits yet (12d: meshes); the deferred pass's lit flag is
  a runtime value (as in 12a/b).

**2026-10-17, step 12d: meshes as a surface source.** `crates/mr-gpu/src/{meshes.rs,mesh.wgsl,mesh_surface.wgsl,voxels.wgsl}`,
`components/src/{shaders.rs,caps.rs,shading/{deferred,packed}.wgsl,draw/{drawer.rs,visbuffer/gpu.wgsl}}`,
`backends/{gpu-compute,gpu-raster}/src/`:
- **The record's mesh capability**: bit 23 flags a mesh triangle's hit, its index in bits 1-22
  (`pack_mesh_hit`; 4M triangles, the mesh pass truncates beyond with a warning); `visbuffer`
  encodes one as `VIS_MESH | tri` (bit 31). Decoding (`mr::mesh_surface`, `mesh_surface.wgsl`)
  reads the triangle from the mesh pass's vertex buffer (now also a storage buffer): the ray's
  barycentrics give the colour (the rasteriser's perspective-correct ones at the pixel centre),
  the ray's point on the triangle the position, the flat normal turned to the camera.
  `shade_mesh` lights it as the forward mesh shader did (`shade_lit`), through the mesh-shadow
  site: `shaders::meshes` adds a lighting instance of its own (`lighting::mesh` over
  `rays::mesh`) when that site's kernel is not the shadow rays'. Shader def `MESHES` compiles
  the capability in (deferred pass: `record_is_mesh` / `record_mesh` per encoding).
- **Ray casters** (gpu-compute `composite`): `MeshComposite` became `MeshRecords`: the mesh pass
  writes records and depth only (no shading, no shadow-ray program of its own); rays stop at the
  mesh depth as before; where no voxel is nearer, the walk shades the mesh record inline or
  writes it for the deferred pass.
- **Face drawers** (gpu-raster `depth`): forward `raster` shades mesh fragments as before;
  `visbuffer` / `gbuffer` draw meshes into the same id / record target in the faces' pass (the
  after-shading mesh pass and its stage are gone; meshes join the `draw` stage), and the
  common pass decodes them from the triangles in the images group (binding 2,
  `images_layout(dev, meshes)`).
- **Compiled in only for frames with meshes.** With the mesh code in the walk, gpu-compute's
  `dispatch = fragment` cost +2.0% / +2.5% GPU time (unlit / lit, 5 rounds) on frames without
  meshes (presumably the second inlined copy of the lighting and shadow walk in the pixel shader;
  compute dispatch showed none). Both back ends now build the mesh variant beside the plain one
  and use it only on frames that draw mesh records: gpu-compute's walk and shade passes
  (`Passes`, `plain` / `meshed`), the drawer's shade pass (`DrawerSpec::mesh_module`; one id
  target). Frames without meshes run exactly the shaders of before.
- **Transparency named in the registry**: `caps::RECORD_CAPS` (meshes, built; `layers` bits
  24-26, `continued` bit 27, `opaque-only` a ray service, not built), tested disjoint and
  matching the WGSL constants.
- Verified: builds without warnings, 75 unit tests (new: the mesh record program composes,
  the record capabilities; gpu-compute's and gpu-raster's compose tests now cover programs with
  and without meshes and mesh shadows through their own kernel), architecture guard (mesh
  surfaces may call `shade_lit`, listed with the reason; `record_is_mesh` / `record_mesh` as
  interface), 4 GPU end-to-end tests. Pixel A/B without meshes: every frame identical (ten back
  ends, the drawers' deferred variants, gpu-compute deferred / fragment, lit + unlit). Mesh
  mode: forward `raster` identical; the record paths differ by 1 at 36-165 mesh pixels per
  frame (ray barycentrics against the rasteriser's interpolation); packed records (gbuffer,
  gpu-compute deferred) also at 6-7 voxel pixels per frame on texel boundaries (up to 12; the
  shade pass compiled with the mesh branch; most now match forward `raster` and the reference).
  PSNR against the reference equal or +0.1 dB in all six mesh configurations. Mesh shadows
  through their own kernel run (inline and deferred; PSNR as the build before).
- Timing A/B, mesh mode (`meshes` plan, GPU time, 3-5 rounds): gbuffer -16.1% lit / -6.1%
  unlit, visbuffer -15.9% / -12.4% (meshes no longer shaded in a pass of their own), forward
  `raster` -0.5% / -3.5%, gpu-compute inline -2.0% / -0.4%, deferred -2.6% / +0.3%, fragment
  +0.3% / **+1.6% unlit (log)**: its mesh pixels now decode their triangle in the walk's fragment
  shader instead of reading a colour the mesh pass shaded. Without meshes: GPU time within ±0.9%
  everywhere. **CPU-bound fps, an unresolved loss (log)**: gpu-raster -1.4..-2.5% and gpu-rt
  -1.7% fps (5 / 3 rounds), all of it in the CPU geometry stage (+2-3%: chunk meshing, code this
  step did not touch; gpu-rt's back end is untouched and does not use the mesh code), the
  signature of step 11's loss: how the surrounding code is laid out, not what runs.
- Not yet: meshes are shaded by the lighting model (`shade_mesh`), not the shading component
  (no path tracing of mesh surfaces; a `Surface` would need an albedo instead of a block texel);
  gpu-discover and cpu-discover have no meshes stage (their drawer now supports one); CPU back
  ends have no meshes.


**2026-10-17, step 13a: the model for stateful nodes, two-way edges and the schedule.**
`components/src/graph.rs` (`When`, `Node::timing`, `state`, `Schedule`, `Checked::schedule`),
`src/main.rs` (`graph` prints the schedule), `src/gui/stages.rs`:
- **Edge timings** per input (`timing = { faces = "prev" }` or `{ faces = { iterate = 2 } }` in a
  graph; JSON alike for the worker): `now` (default), `prev`, `iterate = n`, as designed above.
  Validation with reasons: a cycle of `now` edges (says to mark an edge `prev` or `iterate`), an
  `iterate` edge closing no cycle, fewer than 2 passes, a timing on fused data or on an unbound
  port. The frame order follows `now` edges only; reachability (nodes leading to no image)
  follows every edge. The canonical form writes a timing after its input (`faces=n2~iterate2`),
  so every existing graph's form and key is unchanged: all 16,975 compositions compared before
  and after, byte-identical.
- **A feedback port**: `quadtree`'s optional `covered` (hits from the drawer), so incremental
  discovery is expressible as a graph (`covered` = draw, the drawer's faces iterating twice;
  `still_holes = late` as `covered` read `prev`).
- **Stateful nodes** (`graph::state`) and **the schedule** (`Checked::schedule`): passes in frame
  order with their compiled-in fused nodes and their state, loops contracted to one step in the
  order (the consumer first; nodes reading the loop after it), transfers for non-fused edges
  between devices, kept values; `pipelined()` when a `prev` edge crosses devices. The schedule
  is a pure function of the checked graph. `multirenderer graph <renderer>` prints it, e.g.
  cpu-discover: world, storage @cpu, visibility @cpu [kernel, storage], draw @gpu,
  shadow.storage @gpu, shading @gpu [shadow.kernel, shadow.storage]; transfers faces cpu -> gpu
  and the change log cpu -> gpu.
- **Back ends reject timings** with the reason (`accept`: they run every edge within the frame;
  their knobs set what timings would). The GUI's pipeline tree shows a timed edge as
  `← node (timing)` instead of descending (it cannot loop: shared nodes were already cut).
- Verified: builds without warnings (dev, release), 78 unit tests (new: timings and their
  reasons, TOML / JSON round trips, an explicit `now` keyed as none; the iteration's schedule
  with the shading after the loop; cpu-discover's transfers, pipelined with faces `prev`; every
  composition schedules, with walking kernels only compiled in, `rayquery` a pass too, every
  fused node compiled into a pass on its device), 4 GPU end-to-end tests. No renderer code
  changed, so no pixel or timing A/B was needed.
- Not yet: nothing runs from the schedule; cpu-discover's graph says its faces arrive `now`
  while its `pipelined` knob (default on) makes them `prev` (13b turns such knobs into timings);
  the GUI cannot set timings (shows them); nested or overlapping iterations are rejected with
  the reason.
