# Plan and progress

The working state of the project for future sessions: what exists, what was learned, what
is next. Ideas and their analysis live in `IDEAS.md`; usage and architecture live in
`README.md`.

## Working agreements

- Repository: https://github.com/ElipticalDrive/voxel-bench (public, branch `main`),
  created 2026-10-09. Commits use the repo-local identity (GitHub noreply address).
  Commit and push freely whenever a chunk of work is done (nobody depends on this repo),
  and mention it to the user.
- The project is about exploring the problem space, not chasing speed. Slower techniques
  and negative results are findings too.
- **New rendering techniques are built as components** (storage, geometry, prepass,
  visibility, shading, back end; IDEAS §12), never as monolithic renderers. A new knob
  declares the component it belongs to (`ParamSpec::of(slot, &[components])`) so the GUI
  shows it next to that component.
- No subagents until the user re-enables them. Usage is self-metered.
- Ideas the user shares go into `IDEAS.md` with analysis. Implement only after asking.
- Verify everything: zero warnings, unit tests, GPU end-to-end tests, a comparison
  against the reference renderer, and interleaved repeats for performance claims.
  Report negative results honestly.
- **One step at a time** (the user's request after hitting a rate limit, 2026-10-10):
  finish a step, verify, commit and push, report, and wait for the go before the next.
- **Priority (2026-10-11): the architecture refactor** (Plan, "Next" below; IDEAS §16–§18).
  The user agreed the direction, the requirements (IDEAS §17) and the condition: grow it
  in steps, each useful on its own and checked against the zero-cost definition, never as
  a rewrite; generalise an interface once two or three real techniques need it. New
  techniques may land between steps. The user prefers new techniques over completing
  coverage of a feature across every renderer (7b is shelved for that reason).
- **Existing, well-supported compilers everywhere** (the user, 2026-10-11): never write our
  own compiler or JIT (no Cranelift back end, no custom IR). CPU pipelines compiled on
  demand are generated Rust compiled by `rustc` (shipped with the final product); GPU
  shaders go through the driver (via wgpu / naga). Shader composition may use an existing
  module system (`naga_oil`, WESL) when text-joining strains.
- **The GUI may be unusable for a few days during a refactor step** (the user,
  2026-10-11); steps need not end in something to play with, as long as the project gets
  back to it.
- Terminology: **adapters** are compile-time code that gives a component a feature it lacks
  natively (e.g. `storage/fallbacks.wgsl`); combinations with no valid path do not exist
  (greyed with the reason), nothing substitutes silently. Avoid the word "fallback".
- **Redundancy means the same algorithm the same way** (the user, 2026-10-11): one
  algorithm implemented several ways is fine when the ways differ fundamentally (back end,
  precision, data layout, a different trade-off); only same-purpose copies that differ in
  nothing essential should be merged.
- The user values diagnostic `view` modes highly; add them where a component has
  something to show (with a numeric colour scale once that exists).

## How to verify and measure

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
- GPU end-to-end tests: `cargo test -q --release --bin multirenderer -- --ignored --skip tuner_demo --skip scale_curve --skip gui_screenshot` (3: the live session, mesh mode, and the headless GUI).
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

## Known issues (not yet fixed)

- Some renderers produce incorrect frames. The user knows; fix later.
- `GrowableBuffer` (`crates/mr-gpu`) rounds sizes up to powers of two, so a 2 GiB
  request exceeds the storage-binding limit (2^31 − 4). The `gpu-raster` back end caps
  `quad_buffer_mb` at 2040 MiB to avoid it.
- `gpu-raster: … + naive` (and `cubes`) with `cull = false` overflows the instance buffer
  on terrain and edits and renders only part of the scene.
- The `svo_dag` storage never frees DAG nodes after edits. A full rebuild is meant to kick in at
  2× growth plus 65,536 nodes, but that path is untested.
- `gpu-splat` re-uploads every point on any change (slow loads and frames); it has no
  culling.
- `cpu-discover` uploads the whole face list every frame, not incrementally.
- `gpu-voxelspace` lit mode is slower than `gpu-compute: bricks + refloor`; its shadow
  rays still walk the voxel pool, not a composed storage.
- The `tree64` storage is ~5% slower than the former gpu-tree64 (cause open); the CPU
  `brickmajor` / `morton` storages 3–4% behind their former renderers (codegen, see
  phase 4j).
- `cpu: prefix` has higher peak RSS than `cpu: hier` despite less world data, probably
  transient build allocations; not investigated.
- The ~0.1 dB image difference of `cpu: prefix + refloor` comes from 2-cell steps
  rounding differently at grazing edges. It goes both ways against the reference.
- The incremental walk on the GPU is 3.5–8× slower than refloor on NVIDIA only (AMD
  1.27×), for an unknown compiler reason (IDEAS §1 "Result 3").
- `gpu-rt: naive + rayquery` runs out of GPU memory on `far` (it already uses 911 MB on
  terrain); found by the first overnight sweep.
- `cpu-discover` (quadtree and cluster) scores 25–34 dB on menger, far, terrain and edits
  lit, where `gpu-discover` with the same quadtree stays above 35 dB everywhere. Cause open.
- The catalogue judges path tracing against the direct-lit reference with the 35 dB floor,
  so noisy path-traced frames count as incorrect (384 of 512 in the first sweep).
- A compared frame still costs ~2.7 ms of readback (GPU→host copy of the image), so
  `compare.every = 50` lowers fps by ~15% at 2,800 fps and ~3% at 500 fps.

## Status (2026-10-10, after the component refactor)

- **Architecture:** every renderer except the reference is a composition (IDEAS §12):
  **back end × storage × geometry × prepass × visibility × shading**. Each back end
  declares which slots it has and their defaults.
  - `components/` (crate `mr-components`): one directory per component with its GPU
    (`gpu.wgsl` / `rt.wgsl`) and/or CPU implementation; the registry (`BACKENDS`,
    `COMPONENTS`), compatibility (`unavailable`), `Composition`, and the shared host-side
    storage manager (`storage::gpu_data::StorageData`: pool, region, DAG and texture
    storages).
  - `backends/`: gpu-compute, cpu, gpu-raster, gpu-rt, cpu-raster, gpu-discover,
    cpu-discover, gpu-splat, gpu-voxelspace, cpu-voxelspace.
  - `renderers/`: only `cpu-reference` (standalone on purpose: it defines the reference).
  - Plans: `compose = { backend, storage, geometry, prepass, visibility, shading }`
    (missing slots take defaults; `name = "<back end>"` loads as its composition).
  - GUI: one compact **stage grid** per renderer (`src/gui/stages.rs`), in the plan
    editor and the live Metrics tab: the back end with its own knobs, then per stage the
    component and that component's knobs. Unavailable components are greyed, with the
    conflict on hover; live switches apply in place or hot-swap.
- **Components by back end:**
  - gpu-compute: storage bricks, hier, prefix, distance, rows, tree64, svo_dag, clipmap;
    prepass none, beam; visibility refloor, incremental, sphere, rowrun; shading direct,
    pathtrace; knobs `dispatch` (compute / fragment), `view` (steps), LOD via `max_lod`
    (pool storages + refloor).
  - cpu: storage bricks, hier, prefix, u8, bits, palette, brickmajor, morton; visibility
    refloor, incremental; shading direct; knob `schedule` (rows / tiles).
  - gpu-raster: storage (shadow rays) the eight GPU ones; geometry greedy, naive, cubes;
    visibility raster, visbuffer, gbuffer; shading direct, pathtrace.
  - gpu-rt: geometry greedy, naive, boxes; visibility rayquery; shading direct,
    pathtrace.
  - cpu-raster: geometry greedy, naive; visibility raster; shading direct.
  - gpu-discover: storage (GPU); visibility quadtree (knob `draw`: raster, visbuffer,
    gbuffer); shading direct, pathtrace.
  - cpu-discover: storage (CPU); visibility quadtree, cluster (knob `draw`: raster,
    visbuffer, gbuffer); shading direct, pathtrace (knob `shadow_storage`: the GPU storage
    shadow and bounce rays walk).
  - gpu-splat: storage (shadow rays); geometry points; visibility splat; shading direct,
    pathtrace.
  - gpu-voxelspace, cpu-voxelspace: storage heightmap (a component: ring + walks);
    visibility columns; shading direct (gpu also pathtrace); knob `shadows`.
- **Verification:** zero warnings in both builds; 44 unit tests; 3 GPU end-to-end tests
  (live session, mesh mode, headless GUI).
- **Tools:** the GUI (stage grids, live knobs, hot swaps, quality probes, auto-tuner:
  local search, resolution controller, Bayesian; tooltips everywhere); diagnostic views
  (gpu-compute `steps`; gpu-discover `tiles` / `rays` / `churn` / `overdraw` / `quads`;
  cpu-discover cluster `clusters` / `kinds`); benchmark plans in `plans/`; catalogue
  scripts in `scripts/`; hardware counters on the `cpu` back end (`perf`).

### Key findings so far

- **CPU traversal is instruction-bound, not memory-bound.** About 80 cycles and 220
  instructions per step at IPC 2.6; cache misses are ≤3% of the time.
- **Steps per pixel are the lever.**
  - `cpu: hier`, with one 8³ level, is the fastest CPU storage under refloor (+29% on
    edits vs bricks).
  - `cpu: prefix` (the user's structure) takes the fewest steps, 31–51% fewer than
    bricks, but loses 3–9% per pixel to hier under refloor.
  - Under the incremental-level traversal (step 1, IDEAS §1 "Result 2") each move costs
    ~30% fewer instructions, but failed ascent tests add work. It wins on terrain (prefix
    +13–17%, hier +5–10%) and loses on short rays (menger −3–12%). Prefix comes within 2%
    of hier but does not pass it.
- **Composition costs nothing** at run time: `gpu-compute: bricks + refloor` is 7–9%
  faster than gpu-dda (no LOD code); `distance + sphere` and `rows + rowrun` match
  gpu-sdf and gpu-bitmask; the CPU compositions match the former cpu-simd-* speeds; the
  raster compositions match their monoliths unlit and are 7–18% faster lit (shadow rays
  take the composed refloor walk). The new pairing `cubes + visbuffer` halves cubes' lit
  time (IDEAS §12 "Result (phase 3)").
- **Face-cluster discovery (`cpu-cluster`, the user's §10 design)** is more complete
  than the quadtree on every scene at about the same rays (terrain 51.0 vs 43.8 dB;
  menger 51.5 vs 26.4 dB), at 2.5× the CPU time.
- **GPU storage × visibility (gpu-compute, formerly gpu-compose):**
  - prefix + refloor is the fastest GPU ray walk on open scenes (−13% terrain, −23%
    edits vs bricks);
  - the split costs nothing (bricks + refloor is 7–9% faster than gpu-dda);
  - the incremental traversal hits an unexplained 3.5–8× slowdown on NVIDIA only (AMD
    1.27×). See IDEAS §1 "Result 3".
- **Event-driven discovery:** a still camera costs 0.035 ms instead of 0.6 (17×). Pure
  rotation still needs hole detection, because pixel centres move. On moving scenes the
  time is in fixed passes, not rays, so full re-discovery stays 10–24% faster there.
- **Unlit GPU timings before 2026-10-09 were inflated.** Ten GPU shaders computed lighting
  and shadow rays in unlit mode (`select` evaluates both sides); fixed, −25–35% unlit.
- **Greedy merge across block types** (`gpu-raster merge = "exposure"`) gives −16–18%
  quads on mixed geometry, but nothing on terrain.
- **Discovery** (`gpu-discover`, `cpu-discover`) finds nearly every visible face at
  0.04–0.25 rays per pixel. The user reports incremental mode at roughly double the speed
  of full re-discovery in their use and is happy with the measurements.
  - Incremental mode wins at walking speed: −33% GPU time and −83% rays, at better
    quality.
  - It loses 13–19% in fast flight.
  - It is still about 5× slower than rasterising everything (0.27 ms terrain). The
    multi-pass GPU pipeline and incoherent deep-level rays are the cost.

## Plan

### Done: the component refactor (IDEAS §12)

The user's goal, reached 2026-10-10: no chooser of monolithic renderers; a renderer is a
back end plus one component per stage; the UI forbids unsupported combinations and says
why. Phases 1–5 (components crate and gpu-compute; CPU back end; GPU-raster back end;
porting every remaining renderer, 4a–4l; removing the legacy handling and adding the
stage grid) are in the progress log below, with their measurements.

### Next: the architecture refactor (IDEAS §16–§18)

**Where we are (2026-10-12):** steps 1–4 are done (progress log entries of 2026-10-11/12:
one shading entry point per kind of back end with a `Surface`; one face drawer with face
sources and the host-side `FaceDrawer`; every shadow and bounce ray through a storage
component, the heightmap as a component; the architecture guard test
`tests/architecture.rs`). Builds force code alignment (`.cargo/config.toml`). Step 5
(devices handed to back ends) is in progress; steps 6–8 follow. Open, deferred by the
user to a later optimisation pass: gpu-voxelspace's lit cost (column threads shade
serially; proposed per-pixel deferred shading, sweep entry of 2026-10-12), gpu-compute
pathtrace codegen variance (accepted), cpu-discover's per-ray `dyn World` call.
A/B tooling used for the zero-cost checks lived in the session's temporary directory;
the method: save a release build before a step, then render with both builds and
compare saved frames pixel by pixel (`output.save`), and interleave `run` invocations of
both for GPU ms / fps (`-u output.compare -u output.save`, 3+ rounds, 300 frames).

The goal and requirements are in IDEAS §17 (agreed 2026-10-11): no cost from composability
while a pipeline runs; no opinion on hardware configuration; maximum practical code reuse;
maximum practical combinations, grown with adapters; combinations without a valid path do
not exist; generic precompiled variants plus variants compiled on demand for the user's
machine (cached), and a user-triggered break-in mode. Plus: every newly compiled pipeline
is checked against the reference before it is trusted; caches keyed by source hash,
toolchain, CPU and driver; the generic build runs while the optimised one compiles, then
hot-swaps; generated code runs in worker processes.

**Zero-cost check (every step):** a composed pipeline is as fast as a hand-written pipeline
of the same pieces on the same devices. In practice: no composition affected by a step may
get slower than the baseline `baselines/sweep-20261011/` (GPU ms within ±1–2%, fps within
run-to-run noise, ±4%; interleaved repeats for anything in doubt), and image quality
(PSNR) must not drop. Steps that widen combinations add rows; existing rows must hold.

Steps, in order; one at a time (verify, commit and push, report, wait for the go):
1. **Done 2026-10-11 (progress log).** **One shading implementation per kind of back end** (IDEAS §16 step 1). Every visibility
   strategy produces a hit (position, normal, block, face) and calls the shading
   component: `shade_hit` (GPU), `shade` (`components/src/shading/direct/cpu.rs`, CPU).
   Move the own lighting of gpu-splat (`splat.wgsl`), gpu-voxelspace (`vs.wgsl`),
   cpu-discover (`raster.wgsl`), cpu-raster and cpu-voxelspace onto it. Then every
   shading component (direct, pathtrace) is offered wherever a hit exists (e.g.
   pathtrace on gpu-discover). Decided 2026-10-11 (the user: "your call"): the shading
   input becomes an explicit surface point (position, face or normal override, block,
   cell, lod), filled from `t` by ray casters, so gpu-splat's clamped point and
   voxelspace's slope normal fit; both voxelspace back ends move onto it (no fused
   exception). cpu-voxelspace gains point lights (gpu-voxelspace's model; shadows for both
   voxelspace back ends come with step 3, through the heightmap). cpu-raster: try `shade`
   (f32) and keep the reference's f64 `shade_surface` if PSNR drops (a fundamental
   difference, so not redundant). Pathtrace goes where a `ctrace` exists (gpu-discover,
   gpu-splat); cpu-discover and voxelspace stay greyed with the reason until step 3.
2. **Done 2026-10-11 (progress log).** **One shared face drawer** (§16 step 2): `face_axes`, `block_at`, `pixel_ray` and the
   quad drawing of gpu-raster `main.wgsl`, gpu-discover `raster.wgsl` and cpu-discover
   `raster.wgsl` become one component; visbuffer and gbuffer become choices for both
   discovery back ends.
3. **Done 2026-10-12 (progress log).** **Shadow and secondary rays through the composed storage everywhere** (§16 step 3):
   cpu-discover's lit mode and gpu-voxelspace walk their own structures today.
4. **Done 2026-10-12 (progress log).** **The guard test** (§16 step 4): no WGSL function defined in more than one back end's
   shaders; every back end's lighting goes through the shading component.
5. **Devices handed to back ends** (requirement 2): today seven back ends call `Gpu::new`
   themselves. A device becomes a resource given to a back end (and later to stages), so
   several GPUs, or a GPU and the CPU, are a configuration. Also the first step toward
   the library interface (IDEAS §14: render into the caller's device and texture).
6. **One capability model with adapters:** each interface has a required core and optional
   capabilities; components declare what they provide and need; adapters supply missing
   capabilities at compile time; the registry derives what is valid. First applied to the
   hit record (optional: position within the face, LOD level, material layer, step counts,
   mesh id), generalising the existing `s_*` + `fallbacks.wgsl` pattern. Unused
   capabilities compile away.
7. **Split visibility into traversal kernels and visibility strategies** (§16 step 5,
   IDEAS §18): kernels (refloor, incremental, sphere, rowrun, rayquery) answer ray queries
   against any storage with the needed capabilities; strategies (ray cast, raster,
   discovery, splat, columns) organise primary visibility and use kernels. Discovery's
   pieces (sampler: quadtree, cluster, heads; hole detection; retention policy; drawer)
   become choices. Pipelines become graphs (shading reads the hit and queries the
   storage; history feeds frames forward). The GUI's stage grid grows with it.
8. **CPU pipelines compiled on demand** (IDEAS §17, option 2): generate Rust for one
   composition, compile it with `rustc` into a library, load it in the worker process;
   cache by source hash, toolchain and CPU. Start with the `cpu` back end, whose ahead-of-
   time product (storage 8 × traversal 3 × packet width 4) it replaces; keep generic
   precompiled variants. Then the break-in mode, cheapest layer first: `target-cpu=native`,
   LTO and constants specialised for the run; profile-guided optimisation with a short
   sweep as the training run; then knob autotuning and composition choice (the planner,
   §14). On the GPU: specialised constants and persistent pipeline caches.

Watch: WGSL text-joining strains as pieces multiply (adopt `naga_oil` or WESL around steps
6–7); the number of valid pipelines will outgrow overnight sweeps (the per-pipeline
reference check keeps that safe); wgpu shares no memory between adapters, so multi-GPU
transfers go through the host.

Findings from the sweeps still to act on (between steps, or after): the `gpu-rt: naive`
out-of-memory on far; cpu-discover's quality below gpu-discover's; the catalogue's rule
for path tracing; diagnostic views where the catalogue surprises (shadow-ray cost,
raster overdraw); knob sweeps around the best compositions; planner groundwork (§14).

Other candidates, sized for single steps:
- More techniques as components (the backlog below), e.g. the persistent-kernel
  discovery (3b), mesh shadows (7d), transparent cubes. Transparency and the next round
  of discovery work should come after steps 1–4 (they land on the duplicated parts).
- Known issues above (tree64's 5%, gpu-splat culling / incremental uploads).

### History: the step plan of 2026-10-09

Estimates are rough, in hours of my working time. For scale, the §2 + §1 + §3 stretch on
2026-10-09 was about 2½ hours. The user can judge what share of a usage window that was;
each step below is sized relative to it. Steps that need new correctness proofs or tests
sit at the high end.

| # | Step | Time | vs. the 2½ h stretch | Notes |
|---|---|---|---|---|
| 1 | ~~Prefix structure with an **incremental-level hierarchical DDA**, CPU~~ | done | | Done 2026-10-09 as the `traversal` knob on every `cpu-simd-*` variant. Mixed result; see IDEAS §1 "Result 2". |
| 1b | ~~The same on the GPU~~ | done | | Absorbed into 6′: `gpu-compose storage=prefix`. |
| 2 | ~~**Event-driven discovery** in gpu-discover~~ | done | | Done 2026-10-09: static frames 17× cheaper; pure pans need the face-id raster after all (pixel sampling); exact change areas. IDEAS §3 "Result 2". |
| 2b | ~~**Greedy merge** of the discovered set~~ | done | | Done 2026-10-09 as one-pass row merging (`merge = rows`): half the quads, lit −10%, T-junction cracks −2 dB. IDEAS §3 "Result 3". |
| 3 | ~~**Morton-ordered breadth-first work queues**~~ | done | | Done 2026-10-09: no gain from ordering (the cost is per-level pass latency); inlining small footprints gave −17–21% on edits. IDEAS §8 "Result". |
| 3b | Persistent kernel with work stealing | 2–3 h | ~1 | Riskier: driver and forward-progress caveats. Now the measured bottleneck: ~0.6 ms of a 1.9 ms frame is per-level pass latency. |
| 4 | ~~**Back-to-front order without depth testing**~~ | done | | Done 2026-10-09: exact Manhattan-distance painter order; even or slightly worse, since overdraw is already 1.03. IDEAS §11 "Result". |
| 5 | ~~**Face-cluster discovery prototype** (§10)~~ | done | | Done 2026-10-10: `cpu-cluster`. More complete than the quadtree everywhere at about the same rays/px; CPU 2.5× slower. Far-field LOD and clip-space fixed point not needed yet. IDEAS §10 "Result". |
| 6 | ~~**Modular library**~~ | superseded | | Became the component refactor above (phases 1–2 done 2026-10-10). |

Further steps:

| # | Step | Time | vs. the 2½ h stretch | Notes |
|---|---|---|---|---|
| 6′ | ~~**Storage + traversal slice** of §12~~ | done | | Done 2026-10-09: `gpu-compose` with storages bricks/hier/prefix and traversals refloor/incremental. |
| 7 | ~~**Mesh mode, core** (§13)~~ | done | | Done 2026-10-09: PassMode, `meshes` tag, `update_meshes`, scene meshes with animation, reference triangles, GUI toggles. The tuner tunes knobs within a lane and doesn't pick renderers, so it needs no filter. IDEAS §13 "Result". |
| 7b | Meshes in the raster renderers (gpu-raster ~~first~~ done) | 0.5–1 h each | ~¼ each | **Shelved by the user (2026-10-10).** Will fall out of the GPU-raster back end (phase 3), which uses `mr_gpu::meshes`. |
| 7c | ~~Mesh compositing module for ray casters~~ | done | | Done 2026-10-10: `MeshComposite`; gpu-dda(-lod), gpu-compose, gpu-bitmask, gpu-sdf, gpu-beam. tree64 / svo-dag / clipmap / rtx need their own occlusion in the mesh pass. IDEAS §13 "Result 2". |
| 7d | Meshes shadowing voxels | 2–4 h | ~1–1½ | Later and optional; needs a mesh BVH in the shadow path. |

**Totals for the planned path.**
- Core path (1, 6′, 2, 3, 4, 2b, 7, 7c, 7b for ~4 renderers, 5): **17.5–28 hours**,
  about **7–11×** the 2½-hour stretch. Summing the "vs." column gives ≈ 9.
- With the optional steps (3b persistent kernel, 7d mesh shadows): **21.5–35 hours**,
  about **8.5–14×**.
- The rest of step 6 (the full modular library) is not included; it takes several
  sessions.
- These are rough. Unexpected findings, like today's sky memory and keep-unseen
  discoveries, tend to add 20–50% to the steps that involve research.

**Decision.** Adopt the modular architecture of IDEAS §12, built in stages rather than
first. Steps 1–4 change the inside of single components, so modularity wouldn't make them
cheaper, and step 1 tells us what the storage/traversal interface must offer.

**Order:** ~~1~~ → ~~6′~~ → ~~2~~ → ~~3~~ → ~~4~~ → ~~2b~~ → ~~7~~ → ~~7c~~ → ~~5~~ (done) → rest of 6, and more renderer variety (the user's priority, 2026-10-10). 7b is shelved by the user for a later date. Transparent cubes are the other remaining big feature; the user wants them later. → 3 → 4 → 2b → 7 → 7c → 7b → 5 → rest of 6. Step 7 can move
earlier if meshes become more urgent; 7c depends on 6′.

## Progress log

### 2026-10-09
- **Tuner:** explore/hold/resume, noise-aware acceptance, stop restores the best
  configuration, and best/diff/knob-trajectory views.
- **GUI:** tooltips on every control.
- **Renderers, batch 1** (13, agents): svo-dag, raster-naive, beam, frag-dda, hybrid,
  clipmap, visbuffer, pathtrace, voxelspace (GPU and CPU), splat, bitmask, cubes.
- **Renderers, batch 2** (agents): `cpu-simd-mem` (u16/u8/bits/hier/palette, hardware
  counters), `cpu-simd-loc` (brickmajor/morton/tiles/best), `gpu-discover`,
  `cpu-discover`.
- **§2:** `mr_mesh::Merge::Exposure` and the gpu-raster `merge` knob.
- **§1:** `cpu-simd-prefix`, with a one-pass Morton build and a Morton-walk decode for
  edits. A branch-free probe was tried and reverted.
- **§3:** gpu-discover `incremental` mode:
  - ID raster, then `mark`, `need_raw` (parallax with age accumulation, reprojected sky
    memory), `need_final` (dilation, changed-chunk rectangles, refresh), restricted
    level 0, and `sky_update`;
  - `keep_unseen` for faces in view, `keep_frames` for faces outside it.
- **Ideas recorded:** IDEAS.md §1–§13.
- **Decided:** adopt modular composition (§12), in stages; mesh mode as flags (§13).

### 2026-10-09 (later): step 1
- **Incremental-level hierarchical DDA** (`trace_incremental` in `cpu-simd-mem`):
  - knob `traversal` = `refloor` / `incremental` / `incremental-exact`, plus
    `skip2`/`skip4`/`skip8`/`skip16` to choose levels;
  - written against a new storage query, `Repr::occupied(level, cell)` with
    `Repr::LEVELS`, so all six `cpu-simd-*` variants get it (a first piece of §12);
  - counters `dda_steps_per_pixel`, `ascent_tests_per_pixel`, `descents_per_pixel`;
  - tests: `occupied` exact at every level; incremental agrees with refloor on 12k rays
    per representation, zero differences.
- **Findings:** see IDEAS §1 "Result 2". In short:
  - cheaper moves but more tests; terrain wins, short rays lose;
  - additive exit times drift by 1.5–3 dB, and the exact variant fixes it;
  - prefix's per-level test costs more than hier's.
- **Tried and reverted:** `ascent_delay` (bookkeeping cost more than it saved).

### 2026-10-09 (later): step 6′
- **`gpu-compose`:** storage and traversal as WGSL modules fused into one shader.
  - Storages: `bricks`, `hier`, `prefix` (per-slot side data kept in step with the pool
    via the new `VoxelWorld::slot_of` / `pool_slots`).
  - Traversals: `refloor`, `incremental` (with `exact`, `skip*`).
  - Live shader recompiles for traversal knobs; `view = "steps"` heat map.
- **Findings:** see IDEAS §1 "Result 3" and §12 "Result".
- **Fixed in ten other GPU renderers:** lit/unlit `select` evaluated lighting on unlit
  frames (closes the gpu-frag-dda known issue).
- **Open:** why incremental is slow on NVIDIA; LOD and the other ray casters as storages;
  rank-compressed ids for prefix on the GPU.

### 2026-10-09 (later): step 2
- **gpu-discover events:** frame kinds static / still / moving / full; `still_holes`
  now|late; holes told apart from sky after the raster; edits mark only their blocks'
  tiles (per-tile mask, no 64-rect cap); no forced full frames for boxes behind the
  camera.
- **Views:** `view = tiles | rays | churn`; counters `frame_kind`, `frames_*_pct`,
  `faces_new`, `faces_dropped`.
- **Findings:** see IDEAS §3 "Result 2".

### 2026-10-09 (later): step 3
- **gpu-discover:**
  - `inline_fp` (small footprints expanded inline; level-2 jobs 38k → 4.6k);
  - `morton` (Z-curve level 0, off: no gain);
  - order-preserving appends;
  - counters `jobs_L*` and `rays_L*`.
- **Finding:** deep levels are bound by each pass's slowest ray and the gaps between
  passes (~0.6 ms per frame), not by ray order. This points at 3b.

### 2026-10-09 (later): step 4
- **gpu-discover:** `order = painter` (counting sort by Manhattan distance, no depth test)
  and `view = overdraw`.
- **Findings:** overdraw 1.03 for full discovery, so there's nothing to win; ~5 for
  incremental on edits (kept unseen faces). See IDEAS §11 "Result".

### 2026-10-09 (later): step 2b
- **gpu-discover:** `merge = rows` (runs along u with uniform v-neighbour signature; seam
  edges grown 1/16 px; per-cell block lookup) and `view = quads`.
- **Finding:** T-junctions dominate the quality cost of merging; growing silhouettes
  bleeds more than it fixes.

### 2026-10-09 (later): step 7
- **Mesh mode:**
  - `PassMode` with `+meshes` modes, the `meshes` tag, `Renderer::update_meshes`, IPC v3;
  - scene meshes (shapes, copies, animation);
  - triangles in cpu-reference;
  - `mr_gpu::meshes` and gpu-raster support;
  - GUI Lit and Meshes toggles (L, N);
  - plans/meshes.toml;
  - tests, including a live end-to-end mesh test.
- **Simplification:** meshes cast no shadows yet (7d), so every renderer can match the
  reference now.

### 2026-10-10: step 7c
- **`mr_gpu::meshes::MeshComposite`:** meshes rasterised first, rays capped at mesh depth.
- **Mesh support for ray casters:** gpu-dda(-lod), gpu-compose, gpu-bitmask, gpu-sdf and
  gpu-beam, all at 71.3 / 73.5 dB on the mesh scene for +0.01–0.05 ms.
- **Live mesh test:** now uses gpu-tree64 as the renderer without mesh support.

### 2026-10-10: step 5
- **`cpu-cluster`** (cpu-discover `algorithm = cluster`): seed lattice, coplanar flood
  fill confirmed by shared corner rays, coverage of confirmed faces; sky rules
  `corners` / `sparse`; views `clusters` and `kinds`; a completeness unit test.
- **Findings:** see IDEAS §10 "Result".
- **Follow-ups:** a scanline fill (speed), far-field super-faces, a GPU version.

### 2026-10-10: composition step
- **Interface design and inventory** in IDEAS §12.
- **gpu-compose:** optional storage queries with generic fallbacks (`s_dist`, `s_row`);
  storages `distance` (from gpu-sdf) and `rows` (from gpu-bitmask); traversals `sphere`
  and `rowrun`. That is 5 × 4 = 20 combinations, all matching the reference; the
  composed ports are as fast as the originals.
- **Next ports by yield:** tree64 / SVO-DAG, clipmap, the beam prepass, a path-tracing
  shading component, LOD.

### 2026-10-10: component refactor, phase 1
- **`components/` (mr-components):** storage (bricks, hier, prefix, distance, rows),
  visibility (refloor, incremental, sphere, rowrun) and shading (direct), each a
  directory with its WGSL and CPU builder. The crate also holds the metadata
  (slot, back ends, provided / used queries), the compatibility rules and
  `Composition`.
- **`backends/gpu-compute`** (was gpu-compose; that name remains a legacy alias)
  assembles shaders from the components. The distance-field builder moved from gpu-sdf
  into its component.
- **Plans** use `compose = { backend, storage, visibility, shading }`, validated and
  resolved at load, and saved as the compose table only. CLI: `-r
  backend:storage+visibility[+shading]`; `list` shows components by slot.
- **GUI:** the component chooser. Back ends and components that cannot run are disabled
  with the reason; fallback pairings get a note. Monoliths sit under "legacy".
- **Tests:** the registry is consistent with its implementations; plan round trip; all
  20 GPU combinations render like the originals.
- **Next phases:** 2 CPU back end, 3 GPU-raster back end, 4 the remaining unique pieces,
  5 remove the legacy section.

### 2026-10-10: component refactor, phase 2
- **`backends/cpu`:** renderer `cpu`, one monomorphised instance per storage. Pieces
  moved into `components/`:
  - storage: `storage/cpu.rs` (the CPU storage interface and chunk grid) and
    `storage/*/cpu.rs`;
  - visibility: `visibility/{refloor,incremental}/cpu.rs`;
  - shading: `shading/direct/cpu.rs`.
- **Retired:** cpu-simd-u16/u8/bits/hier/prefix/palette, now cpu compositions.
  Knobs: `visibility` plus `exact` replace `traversal`.
- **Results:** all 12 CPU combinations match the reference (63.0 dB terrain) at the
  earlier speeds.
- **Plans:** gpu-dda, gpu-sdf and gpu-compose entries replaced by exact compositions
  (gpu-dda-lod stays until LOD is a component); a test loads every shipped plan.

### 2026-10-10: component refactor, phase 3 (GPU-raster back end)
- **Slots per back end:** `BackendInfo.slots` lists each back end's slots with defaults;
  `Composition` gains `geometry`, fills missing slots from the back end's defaults,
  rejects slots a back end lacks, and adapts on a back-end switch. CLI: the back end's
  slots in order (`gpu-raster:bricks+greedy+visbuffer`); GUI shows only the back end's
  slots; `list` shows slots and defaults.
- **Components:** `geometry/{greedy,naive,cubes}` (CPU mesher + WGSL vertex interface;
  naive and cubes moved out of their renderers), `visibility/{raster,visbuffer}`
  (raster-only WGSL entry points built on the back end's `face_hit`).
- **`backends/gpu-raster`:** one renderer for all 30 combinations; geometry,
  visibility, shading, merge, cull and depth_prepass switch live. Shading is the same
  `shade_hit` as on gpu-compute; shadow rays walk the composed storage (refloor).
  Meshes work in every combination (new for naive, cubes and visbuffer).
- **Retired:** gpu-raster, gpu-raster-naive, gpu-cubes, gpu-visbuffer; plans and GUI
  defaults use `compose = { backend = "gpu-raster", … }`.
- **Results:** same images as the monoliths (terrain greedy 53.6 dB, naive 55.5, cubes
  55.4; meshes 63.4 / 71.1); equal GPU time unlit, 7–18% faster lit; `cubes +
  visbuffer` new and 1.9× faster lit than cubes + raster. Details in IDEAS §12.
- **A detail found on the way:** clamping the texel cell into the quad (as gpu-visbuffer
  did) costs ~1 dB on naive / cubes against the reference; the unclamped cell (the
  former gpu-raster's `fract`) is used for texels, the clamped one only for the
  exposure-merge block lookup.

### 2026-10-10: phase 4a (retire covered renderers)
- Retired gpu-sdf, gpu-bitmask, the plain gpu-dda entry and the gpu-compose alias; their
  exact compositions (`distance + sphere`, `rows + rowrun`, `bricks + refloor`) match them
  (phase 1 results). The gpu-dda crate now holds only gpu-dda-lod (until 4g).
- The distance-field test moved into `storage/distance`. GUI tests and `scale_curve`
  use compositions (`RENDERER=gpu-compute:prefix+refloor`).

### 2026-10-10: phase 4b (tree64 as a storage)
- **Storage interface, top level:** a storage now defines its top cell (`S_TOP`, 5 for
  chunks, 6 for 64^3 regions) and `s_top(key)`; the four walks, the raster block lookup
  and the incremental LEVEL_MASK use it instead of the voxel world's chunk table.
  `s_row` takes a 32-cell segment offset `x0`.
- **Two host-side kinds** (`storage::Kind`, kept by `storage::gpu_data::StorageData`,
  shared by gpu-compute and gpu-raster): *pool* storages (voxel pool + side data per
  slot, as before) and *region* storages (a blob per region built on the CPU, 16-byte
  aligned in `aux` with a vec4 view `aux4`, found through a hash table). Region storages
  turn the voxel pool off. (4c adds a third kind, `Dag`.)
- **`storage/tree64`:** gpu-tree64's tree as a region storage (levels 0, 2, 4, 6); works
  with all four walks, as raster shadow storage, and with meshes.
- **Mesh shadows through the composed storage:** `mr_gpu::meshes::Occlusion` lets the
  mesh pass use the back end's storage and refloor walk (bind group 2). Without it,
  tree64 lit+meshes lost voxel shadows on meshes (57.8 dB); with it 73.5 dB like the rest.
- **Results** (terrain, 3 interleaved repeats): same image as gpu-tree64 (59.9 / 58.7
  dB). `tree64 + refloor` 1.00 ms unlit / 1.48 lit against 0.95 / 1.42 for the
  monolith: **5% / 3.5% slower**, after making nodes one 16-byte load (was 15%) and a
  dedicated probe loop. Not explained by max_steps; the rest is open (region table
  header read, pointer arithmetic, ...). far.toml: 1.30 ms and 183 MB vs 2.11 ms and
  1037 MB for bricks + refloor. Other walks over tree64 (unlit): incremental 1.73,
  sphere 3.30, rowrun 4.12 ms (single runs; through fallbacks).
- **Retired:** gpu-tree64. Plans use `storage = "tree64"`; the live mesh test's control
  lane without meshes is now the `cpu` back end.

### 2026-10-10: phase 4c (SVO-DAG as a storage)
- **A third storage kind, `Dag`:** one global hash-consed node pool in `aux`
  (append-only, rebuilt at 2× growth), chunk roots in the region table. `storage/svo_dag`
  holds the builder (moved from gpu-svo-dag) and the WGSL (levels 0-4).
- **Found on the way:** the 5-7% gap of the tree storages came partly from the region
  table's mask and probe length living in the table (a storage read per lookup). Moved
  to a uniform (`region_head`, group 2 binding 3): svo_dag went from 0.97 to 0.92 ms
  (gpu-svo-dag 0.91). tree64 stays at 1.00 vs 0.95 (64³ regions mean few lookups); its
  remaining 5% is open. Removing the steps counter `ct_iters` from refloor changed nothing.
- **Results** (terrain, 3 interleaved repeats): same image as gpu-svo-dag (59.9 / 58.7
  dB); `svo_dag + refloor` 0.92 / 1.37 ms vs 0.91 / 1.36: **matches**. Meshes work
  (71.3 / 73.5 dB). Single runs of other walks over svo_dag: incremental 2.96, sphere
  4.31, rowrun 2.78 ms unlit.
- **Retired:** gpu-svo-dag.

### 2026-10-10: phase 4d (clipmap as a storage)
- **A fourth storage kind, `Clipmap`:** toroidal 3D textures in group 2 bindings 4-7
  (the layout now depends on the storage), with its own `s_top`: the tag check, returning
  the slot as the handle. `storage/clipmap` holds the texture management (moved from
  gpu-clipmap) and the WGSL (levels 0, 2).
- **Results** (terrain, 3 interleaved repeats): same image (59.9 / 58.7 dB).
  `clipmap + refloor` 1.40 / 1.92 ms against gpu-clipmap's 2.20 / 3.15: **36–39%
  faster**, because the slot wrap (a modulo) now happens once per chunk in `s_top`
  instead of on every texture access. A single run had suggested incremental beats
  refloor on the clipmap; the repeats say no (1.79 vs 1.40).
- **Retired:** gpu-clipmap.

### 2026-10-10: phase 4e (the beam prepass as a slot)
- **A `prepass` slot** (gpu-compute only; `none` / `beam`): a stage before the walk with
  its own compute entry point and `start_t(pixel)`. Rays start at `o + d * t0` and the
  hit's t is shifted back, so the walks need no change. The beam is generalised from
  32-block chunks to the storage's top cell (`s_top`), so it works over every storage.
  `optional` slots (prepass, shading) are left out of labels at their default and come
  last in the CLI form (`gpu-compute:bricks+refloor+direct+beam`).
- **Results** (terrain, 2 interleaved repeats): `bricks + beam + refloor` 1.07 / 1.43 ms
  vs gpu-beam 1.08 / 1.45: **matches**, same image. As before, beam is a net loss
  (bricks + refloor 0.85 / 1.20). Over tree64 it costs much more (1.85 vs 1.00 ms):
  64^3 top cells make the cone's box checks coarse and expensive.
- **Retired:** gpu-beam. Avg skipped distance is the counter `avg_skip_blocks`.

### 2026-10-10: phase 4f (path tracing as shading)
- **Shading interface:** `shade_hit(h, o, d, lit, px)`; the pixel gives per-pixel state
  (random sequence, accumulation). gpu-compute binds an accumulation buffer (group 0
  binding 4, full size only while pathtrace is chosen) and passes frame, samples so far,
  bounces and spp in its camera uniform; it resets the count when view, mode or world
  change.
- **`shading/pathtrace`** (gpu-compute only; on the raster back end overdraw would
  corrupt the accumulation, visbuffer could host it later): bounce rays use the composed
  walk, so path tracing works over every storage and walk.
- **Results** (terrain, 2 interleaved repeats): same images as gpu-pathtrace (58.7 dB at
  bounces = 0, 32.2 dB at 1 bounce, by design). GPU time 1.33 vs 1.43 ms (bounces 0) and
  3.47 vs 3.69 ms (1 bounce): **6–7% faster**. Over svo_dag: 4.37 ms (1 bounce).
- **Retired:** gpu-pathtrace.

### 2026-10-10: phase 4g (LOD in the storage interface)
- **Interface:** `s_top_lod(key, lod)` (pool storages: the chunk table at that LOD;
  others: level 0 only, `S_HAS_LOD = false`). `refloor` walks LOD chunks when the
  assembler sets the compile-time `LOD` (cells, top cells and probe sizes scaled by
  2^lod; `lod_of` picks the level), so it costs nothing without LOD. Shading tiles
  textures per block in LOD cells.
- **`VoxelWorld::changed_keys`** reports LOD chunk changes too, so pool storages' side
  data (prefix, hier, ...) covers LOD chunks.
- **gpu-compute** declares max_lod 6. LOD runs with a pool storage + refloor + no prepass;
  other compositions warn and run without (live switches that would break it are
  refused).
- **Results** (far.toml, 3 interleaved repeats): images identical to gpu-dda-lod (25.09
  dB r12+1, 17.59 dB r4+3, also for prefix). GPU ms: bricks r4+3 1.71 vs 1.70, r12+1
  1.19 vs 1.18 (unlit); lit 2.11 vs 2.09, 1.55 vs 1.53: **matches**. `prefix` with LOD
  1.32 ms unlit (−22%), 1.61 lit (−23%).
- **Retired:** gpu-dda-lod (the gpu-dda crate is gone).

### 2026-10-10: phase 4h (hardware RT as a back end)
- **`backends/gpu-rt`** (slots geometry, visibility, shading): geometry `greedy` /
  `naive` become triangles (one RT implementation, `geometry/greedy/rt.wgsl`), the new
  `boxes` geometry brick AABBs with a software DDA inside (from gpu-rtx-bricks).
  Visibility `rayquery` turns `g_rt_trace` into `ctrace` / `occluded`, so the shared
  shading components run unchanged; `pathtrace` on RT is new (bounce rays are ray
  queries; accumulation as on gpu-compute).
- **Results** (terrain, 2 interleaved repeats): images and GPU times identical to the
  monoliths: greedy 1.07 / 1.17 ms (gpu-rtx 1.07 / 1.17), boxes 1.02 / 1.47 (1.03 /
  1.47). naive triangles 2.05 / 2.15 ms; pathtrace at 0 bounces 1.26 ms lit.
- **Retired:** gpu-rtx, gpu-rtx-bricks.

### 2026-10-10: phase 4i (fragment dispatch, G-buffer)
- **gpu-compute knob `dispatch = compute | fragment`** (was gpu-frag-dda): the per-pixel
  code is one `pixel_color()`, entered from a compute shader or from a fragment shader
  over a full-screen triangle (group 0 then binds a 1×1 stand-in for the storage-texture
  output; the mesh-composite group became fragment-visible). Works with every
  composition, prepass and meshes.
- **Raster visibility `gbuffer`** (was gpu-hybrid): the raster pass writes hit point +
  face (Rgba32Float, allocated only while chosen) and block id; a compute pass rebuilds
  the `Hit` and calls the shading component. The visbuffer path is now a general
  deferred path.
- **Results** (terrain, 3 interleaved repeats): fragment dispatch 0.87 ms vs gpu-frag-dda
  0.92 (−5%), same image (60.2 dB; the fragment path differs from compute by 0.3 dB,
  as before); fragment and compute run at the same speed. `gbuffer` lit 0.75 ms vs
  gpu-hybrid 0.98 (−23%), visbuffer 0.73; same images (53.5 / 50.4 dB). Unlit
  gpu-hybrid drew forward (0.28 ms), which `greedy + raster` covers (0.26); `gbuffer`
  always defers (0.36).
- **Retired:** gpu-frag-dda, gpu-hybrid (the gpu-frag-hybrid crate).

### 2026-10-10: phase 4j (CPU)
- **cpu-simd** is `cpu: bricks + refloor` (identical image). The cpu back end's work
  loop is now per rectangle: image rows, or with knob `schedule = tiles` screen tiles
  along a Morton / Hilbert / linear curve with 2D packets (`tile_order`, `tile`; from
  cpu-simd-tiles). Writing pixels through rectangles made plain rows 9% faster than
  before (18.6 vs 17.0 fps unlit), so `cpu: bricks` now beats cpu-simd (17.3).
- **Storages `brickmajor` and `morton`** (cpu-simd-loc's block-id layouts): one generic
  `storage::layout`. Measured with the counters: L1d misses halve (0.53 vs 1.01 per
  pixel) and instructions drop slightly, but IPC drops too (2.08 vs 2.19), so cycles do
  not improve; they run 3–4% behind cpu-simd-brickmajor/morton and behind plain
  `bricks`. The former +5% of those renderers was their own codegen, not the layout:
  CPU traversal is not memory bound (as found before). Tiles: 19.3 vs 19.6 fps unlit
  (−1.5%), 14.8 vs 15.2 lit. Software prefetch was not ported (measured: only adds
  instructions).
- **Back end `cpu-raster`** (the renderer's crate moved to `backends/cpu-raster`):
  geometry greedy or naive (the geometry component's CPU mesher), visibility raster,
  shading direct (the reference model). Same image and speed (15.6 / 12.3 fps vs 15.3 /
  12.0); naive on the CPU is new (12.4 / 10.4 fps).
- **Retired:** cpu-simd and the presets cpu-simd-brickmajor / -morton /
  -tiles / -best (the cpu-simd and cpu-simd-loc crates).

### 2026-10-10: phase 4k (discovery as components)
- **Design:** discovery (the user's idea) replaces both geometry and raster visibility, so
  it gets its own back ends rather than a slot on gpu-raster. The discovery machinery is
  kept whole (all knobs, events, incremental mode, merge, painter order, audit, views
  tiles / rays / churn / overdraw / quads / clusters / kinds); only its access to the
  world became the composed storage.
- **`backends/gpu-discover`** (slots storage, visibility `quadtree`, shading): rays,
  exposure tests and lit shadow rays read the composed storage (group 2) with the
  refloor walk (`ctrace` replaces the pool `trace`; `block_at` goes through `s_top` /
  `s_block`), and fragments shade through `shade_hit`. Works over every GPU storage,
  including tree64 / svo_dag / clipmap (no voxel pool then).
- **`backends/cpu-discover`** (slots storage, visibility `quadtree` / `cluster`,
  shading): `grid.rs` (a copy of cpu-simd's grid and DDA) became a `World` trait over the
  components' `Grid<R>` with the refloor packet walk, one store per CPU storage; the
  discovery and cluster code is unchanged. cpu-cluster is `cpu-discover: … + cluster`;
  the `algorithm` knob is now `visibility` (old name still accepted).
- **Results** (terrain, interleaved repeats): identical rays per pixel, face-set sizes,
  images and audits to the three former renderers in every configuration measured
  (gpu-discover plain / incremental / audit; cpu quadtree and cluster). gpu-discover GPU
  time equal unlit, −4% lit (shadow rays take the composed walk); over `prefix` −8–10%,
  over `tree64` +25% (exposure tests descend the tree). cpu-discover discovery time
  equal (41.4 vs 41.3 ms quadtree, 95.8 vs 95.6 cluster); over `hier` −7%, cluster over
  `prefix` −4%. All views checked visually.
- **Retired:** the gpu-discover, cpu-discover and cpu-cluster renderers (their crates
  moved to `backends/`).

### 2026-10-10: phase 4l (own representations)
- **Back ends `gpu-splat`** (storage for shadow rays, geometry `points`, visibility
  `splat`, shading direct; shadows walk the composed storage, so the voxel pool is off
  for region storages), **`gpu-voxelspace` and `cpu-voxelspace`** (storage `heightmap`,
  visibility `columns`, shading direct: single-option slots for now, ready for more).
  New helpers `mr_components::require` / `names_on` for back ends' option checks.
- **Results** (terrain, 2 interleaved repeats): identical images to the three former
  renderers (splat 27.34 / 23.97 dB, voxelspace 20.42 / 17.54 GPU, 20.42 / 17.50 CPU);
  GPU time equal (splat 1.29 / 1.79 ms vs 1.30 / 1.85; voxelspace 1.13 / 2.80 vs 1.13 /
  2.79). Splat over prefix equal, over tree64 +10% lit.
- **Retired:** gpu-splat, gpu-voxelspace, cpu-voxelspace (crates moved to `backends/`).
  `renderers/` now holds only cpu-reference.

### 2026-10-10: phase 5 (stage grid, legacy removed)
- **Knobs belong to components:** `ParamSpec.owner` (`ParamSpec::of(slot, components)`),
  set by every back end (e.g. `exact` / `skip*` → visibility incremental, `bounces` /
  `spp` → shading pathtrace, `merge` → geometry greedy, discovery knobs → visibility
  quadtree / cluster). The field is always serialised: renderer infos cross the worker
  boundary as postcard, which cannot skip fields (a `skip_serializing_if` broke the
  workers until the GUI test caught it).
- **Stage grid** (`src/gui/stages.rs`), used by the plan editor and the live Metrics
  tab: back end + its own knobs, then per stage the component picker and that
  component's knobs. All components of a slot are listed, unavailable ones greyed with
  "Conflicts: …" on hover. Rows wrap knob by knob (a grid or nested scopes widened the
  side panels and squeezed the Metrics panel out). Live: component changes go through
  `set_param` (in place or hot swap), back-end changes hot-swap the lane.
- **Legacy removed:** no legacy pickers; "+ renderer" and "+ cpu-reference";
  `name = "<back end>"` in a plan (e.g. the playground's gpu-discover) loads as that back
  end's composition, stages from the options; `list` shows standalone renderers.
- **Back ends** use `mr_components::require` / `names_on` instead of copies of
  `check` / `names`.
- **GUI screenshot test** (`gui_screenshot`, ignored): renders the real GUI offscreen
  with egui-wgpu to `target/gui-shot.png`, so layout changes can be checked visually.

### 2026-10-10: colour scales on the diagnostic views
- **`mr_core::legend`:** `Legend::Ramp { quantity, max, log }` or `Legend::Categories`; one
  shared ramp (Turbo polynomial) with identical Rust (`Legend::color`) and WGSL
  (`ramp_wgsl()`: `ramp_value(v, max, log)`) versions; white above `max`. `Legend::read`
  inverts a pixel colour to a value (nearest of 256 ramp steps; ±3% on a log scale).
- **`FrameStats::legend`:** renderers report what the active view's colours mean with
  every frame; the GUI stores it per lane, draws it in the lane's bottom-right corner
  (ramp with five numeric ticks and an "over" swatch, or category swatches), and shows
  the value under the cursor with a marker on the ramp.
- **Views converted:** gpu-compute `steps` (was red/green bit-packing) → ramp, log, knob
  `view_max` (default 256). gpu-discover `overdraw` → exact counts (1/255 per fragment
  in red), copied to a scratch texture and ramped by the overlay pass, linear, `view_max`
  (default 8, max 255); `tiles` / `rays` / `churn` / `quads` and cpu-discover
  `clusters` / `kinds` get category legends.
- **New view:** `cpu` back end `view = steps`: refloor = lockstep iterations of the
  pixel's packet, incremental = occupancy tests of the pixel's ray (log, `view_max`).
- **Cost in image mode:** none measurable. The first version put the view in the pixel
  loop and cost the CPU back end 2–3% (incremental); moving it to a separate pass after
  each packet brought it back within noise (interleaved old/new ×3, 3 repeats each:
  hier + refloor 101.0/100.3, 98.7/98.4, 97.5/98.2 fps; prefix + incremental
  91.4/91.2, 90.1/90.0, 89.1/89.6). gpu-compute GPU time unchanged (0.35 ms).
- **Verified:** zero warnings in both builds; 41 unit tests (new: ramp read-back); the 3
  GPU end-to-end tests; GUI screenshot with four views (`SHOT_PLAN`).
- **Open:** auto-ranging (the scale following the frame's maximum, as thermal cameras
  do) needs a GPU reduction read back a frame late; fixed `view_max` for now.

### 2026-10-10: lighting and meshes as stages
- **Design (the user's note):** stages around the walk, so storages and walks stay
  unaware. The pass mode stays what the scene asks for; no component implements four
  variants.
  - *Meshes:* a new slot `Slot::Meshes` (optional, last in labels and the CLI form) on
    the back ends that can draw meshes: gpu-compute (`composite`, default; `none`) and
    gpu-raster (`depth`, default; `none`). Back ends without the slot draw no meshes,
    as before. `none` reads "no meshes" in labels.
  - *Lighting:* stays with the shading component (every shading component renders lit
    and unlit). The lighting knob `shadows` is now owned by the shading stage on every
    back end (`mr_components::lighting_knobs`), so the grid shows it there. A separate
    lighting slot would have one choice per back end today; it becomes worth having
    with a second lighting technique (e.g. flood-fill light, shadow maps, AO).
- **Mode support from the composition:** `Composition::supports(PassMode)` and
  `RendererSpec::supports(info, mode)` replace the static `meshes` tag for compositions
  (benchmark pass filter, the GUI's "(no meshes)" label). The tag remains for the
  standalone cpu-reference.
- **Tuner:** never tunes `view`, `view_max` or `meshes` (they change what is shown).
- **Verified:** plans/meshes.toml, 60 frames: `… + no meshes` skipped in both mesh
  passes; mesh-mode PSNR identical to the previous build (gpu-raster 62.9 dB unlit+meshes,
  gpu-compute prefix 68.1 dB); in `lit`, `none` and the default match (72.7 / 76.3 dB).
  New tests (registry, plan/CLI); 41 unit tests; the 3 GPU end-to-end tests; GUI
  screenshot shows the meshes stage and `shadows` under shading.
- **Next possible meshes components:** gpu-discover and cpu-raster could reuse `depth`
  (they rasterise); gpu-rt could put meshes into its TLAS (meshes would then shadow
  voxels, step 7d).

### 2026-10-10: sweeps, observations and the catalogue
- **Goal (the user):** metrics on how each renderer performs in various scenarios, to see
  what each is good at, and data on every combination for a future planner that learns
  this machine (IDEAS §14).
- **Sweeps in plans:** `"*"` or lists in `compose` expand into every valid combination
  (`expand_sweeps` in `plan.rs`; `RendererSpec::swept`). The entry's options go only to
  back ends that declare them. A named component on a stage a back end lacks skips that
  back end; `"*"` there is ignored.
- **Per-pass dedup:** `Composition::effective(mode)` resets stages a mode cannot tell
  apart: shading in unlit, meshes without meshes, and `BackendInfo::lit_only` (the
  storage of gpu-raster and gpu-splat) in unlit. The full sweep is 458 compositions:
  121 distinct unlit, 258 lit.
- **Observations:** every `run` writes `observations.jsonl` (`src/observe.rs`). Each
  record holds the machine (CPU, threads, memory, GPU, driver via
  `mr_gpu::adapter_info`, OS), the scenario (plan name, scene spec, resolution, view
  radius, frames, streaming, mode), the composition and knobs, and the metrics (fps,
  frame-interval mean / p50 / p99, GPU / CPU ms, latency, init, load, memory, PSNR
  mean / min, bad %, counters averaged). `PassReport` now carries the composition,
  options and view radius.
- **`multirenderer catalogue <dirs>`** (`src/catalogue.rs`) writes `catalogue.md` and
  `catalogue.csv`:
  - per scenario: a ranking, with a PSNR floor (`--min-psnr`, default 35) so incorrect
    renderers are ranked but never best;
  - per renderer: the geometric mean of relative speed, wins, *good at* (within 10%),
    *stands out* (comparative advantage ≥ 1.25×), weak, incorrect, failed;
  - per back end and stage: the paired component effect (other stages and knobs equal).
- **`scripts/sweep.sh [plans]`:** the full sweep over the ten scenario plans, then the
  catalogue. New `-u key` removes plan keys (`-u output.save`).
- **Verified:** unit tests for sweep expansion and the catalogue (43 total); a sweep over
  all back ends on terrain (20 passes); the **full sweep on showcase**: 379 passes in
  3.5 min, none failed.
  - Showcase is tiny and host-bound (2,000–4,000 fps), so the ranking compresses there.
  - The component effects agree with earlier findings: CPU hier is the best storage
    (prefix 0.87); CPU refloor beats incremental (0.87); GPU prefix ≈ hier best
    (bricks 0.76); refloor ≫ incremental (0.64) on this NVIDIA GPU; beam 0.96; cluster
    discovery 0.21 of the quadtree's speed.
- **Caveats:**
  - Renderers that only appear in lit passes are averaged over fewer scenarios.
  - Above ~1,000 fps, host overhead dominates fps; GPU ms is in the tables.
  - Knobs are swept only at their defaults.
- **Next for the data:** record measured scene features (loaded chunks, solid and surface
  fractions, camera speed, edits per frame) beside the plan spec, so the planner can
  generalise from scenario names to features (IDEAS §14).

### 2026-10-10: scene features in the observations
- **Why:** a planner (IDEAS §14) must generalise from the sweep's scenarios to scenes it
  has not measured, so observations need properties of the scene, not only its name.
  This was done before the first overnight sweep, so that data has them.
- **`SceneFeatures`** (`src/features.rs`): renderer-independent, measured once per run
  after world pre-generation (0.0–0.3 s), at `run.view_radius`:
  - camera speed (blocks/frame) and turn rate (°/frame), mean and max;
  - non-empty chunks at frame 0, and chunk loads per later frame (mean, max);
  - at three sampled frames: non-empty chunk share, solid voxel share, and surface voxel
    share (solid with an air neighbour, across chunk borders) in the chunk cube;
  - a 32×18 ray grid per sampled frame: sky fraction and hit distance (mean, median);
  - scripted edits per frame, lights (animated?), a moving sun, and mesh triangles.
- **Plumbing:** `Observation.features` (version 2; version-1 files still read);
  `run_plan` returns them; the catalogue prints them under each scenario heading and
  adds them as CSV columns.
- **Measured on the ten plans:** they separate the scenarios. Some examples:
  - far: 69% sky, hits at 465 blocks, 35 loads/frame;
  - streaming: 6.9 blocks/frame, 31 loads/frame;
  - random: 16% sky, 1% surface;
  - edits: 2,091 edits/frame;
  - many-lights: 4,000 animated lights;
  - meshes: 5,520 triangles.
- **Verified:** new unit test (flat ground, static camera); 44 unit tests; the 3 GPU
  end-to-end tests; zero warnings in both builds; a run → observations → catalogue check
  on menger.
- **Limits:** the features are measured at the run's view radius (per-renderer overrides
  are not reflected) and from the plan's camera path, not from what a renderer streams.

### 2026-10-10: the first overnight sweep, and comparisons off the frame loop
- **The sweep** (`target/sweep/20261010-1858`, 18:58–23:07): all ten plans, 3,047
  observations, 3 failures (`gpu-rt: naive + rayquery` out of GPU memory on far).
  Repeatable: against the earlier showcase sweep, 90% of passes within ±4% fps and ±1%
  GPU ms.
- **Its fps were wrong.** Nearly every pass had a p99 frame interval of 18–24 ms, even at
  1,200 fps. Each compared frame (every 50th) ran SSIM and MS-SSIM (~18 ms at 960×540) on
  the host's frame loop and stalled the pipeline. menger, `gpu-compute: bricks + refloor`:
  1,250 fps with comparisons, 2,840 without; `gpu-raster: bricks + greedy + raster` 675 vs
  970. GPU ms were unaffected. The catalogue ranks by fps, so its rankings are squeezed
  and should not be used.
- **Fix:** `Outputs::deliver` hands the comparison (and the diff image) to two comparer
  threads; the results are attached to the frame records when the pass ends
  (`Lane::pending_quality`). Now 2,370 vs 2,800 fps (the rest is the readback, see known
  issues), 915 vs 965 for the rasteriser; the CPU back end unchanged (137 vs 137);
  identical PSNR; diff images still saved.
- **Findings that hold (GPU ms and correctness, unaffected by the stall):**
  - Never incorrect: gpu-compute direct, gpu-raster, gpu-discover, gpu-rt direct, cpu,
    cpu-raster. Always below 35 dB: gpu-splat, both voxelspace back ends.
  - `distance + sphere` has the lowest GPU ms almost everywhere (terrain 0.61 vs 0.76 for
    prefix), but its fps collapse on streaming scenes (terrain 132, far 47; p99
    123–205 ms): the distance field rebuild on chunk loads. On static scenes (room,
    meshes) it is the fastest gpu-compute composition.
  - `prefix + refloor` has the lowest GPU ms of the grid storages on open scenes (terrain
    0.76 vs bricks 0.86; far 0.77 vs 0.88), as found before.
  - Rasterisers win small and closed scenes (room 0.053 ms GPU); on large ones CPU meshing
    dominates (terrain 2.5 ms CPU). In gpu-raster, `naive` beats `greedy` overall (0.93 vs
    0.87); visbuffer > gbuffer > raster.
  - gpu-rt: `boxes` is the best geometry (no meshing); greedy / naive are bound by BLAS
    builds while streaming (4–11 ms CPU). `gpu-rt: greedy` wins many-lights.
  - Confirmed again: CPU hier best storage (26 of 30), CPU refloor > incremental (0.89),
    GPU incremental 0.60 (NVIDIA), beam 0.93, path tracing 0.39, cluster 0.50 of quadtree.
- **Caveat:** thermal drift over the four hours is not ruled out (the user's room got
  warm); the repeatability check covers only showcase, the first plan.

### 2026-10-11: the sweep re-run (comparisons off the frame loop)
- `target/sweep/20261011-0115`: 3,047 observations, the same 3 failures (`gpu-rt: naive +
  rayquery` out of GPU memory on far). Built after the fix (7748f50).
- The stall is gone: median p99 frame interval 22.9 → 9.6 ms. GPU ms match the first sweep
  (median ratio 1.001, 90% within ±2%), so four hours of running showed no thermal drift.
  fps rose ~12% (median) on the GPU back ends and not at all on the CPU ones, more on fast
  scenes (room unlit best 1,520 → 5,720 fps).
- Rankings: gpu-compute `bricks + refloor` still wins the streaming scenes on fps (terrain,
  far, streaming), though `prefix` and `hier` have lower GPU ms there: the difference is
  now host-side (update) cost, not measurement. Rasterisers win the closed scenes (room,
  meshes, showcase); `tree64` wins edits; `hier + refloor` wins menger; `gpu-rt: greedy`
  wins many-lights. This catalogue is the first to read with the user.

### 2026-10-12: the sweep after steps 3–4 (aligned build)
- `target/sweep/20261012-0057`: 3,854 observations, the same 3 known failures on far, no
  PSNR drop above 0.5 dB. Against the previous sweep (3,735 shared rows): GPU ms median
  0.9994, 84% within ±2%, 91% within ±4% (tighter than before); against the baseline
  (3,047): median 0.9994. Interleaved re-runs of the largest ratios: gpu-compute on room
  identical (±0.4%); gpu-raster on menger lit bimodal in both builds (1.62 / 2.5 ms,
  1.21 / 1.45 ms), i.e. noise.
- **Real, by design: gpu-voxelspace lit** is slower because shadows are now on by
  default: terrain 2.7 → 7.6 ms, but menger 10.5 → 96 ms (sun) and many-lights 15 → 60 ms
  (point lights). The CPU twin pays only 3× on menger (3.0 → 10.0 ms) and is faster than
  the GPU even without shadows (3.0 vs 10.5 ms): gpu-voxelspace runs one thread per screen
  column (1,920 threads, 30 workgroups), so each thread shades its whole column and the
  shadow walks run serially. **Proposed fix (not started):** lit mode writes each pixel's
  surface (point, face, column, block) in the column pass, and a one-thread-per-pixel
  compute pass shades it, the visbuffer pattern of the face drawer; unlit keeps writing
  texels directly. Alternatively default voxelspace's `shadows` to `sun` or `none`.

### 2026-10-12: architecture refactor step 4 (the guard test)
- **`tests/architecture.rs`** reads every back end's and every shared `.wgsl`, plus WGSL
  composed in Rust string literals, and fails on: a WGSL function defined by two back
  ends (except interface functions every face source implements, `src_*`, and entry
  points); a back end redefining a shared function; an identical body under another name
  (as `face_axes_d` was); lighting internals (`shade_lit`, `direct_light`, light lookups,
  point-light math in Rust) used outside the shading components; a back end without an
  entry in its table of shading entry points. Exceptions carry reasons: the meshes stage
  shades vertex-coloured fragments (`mesh.wgsl`), cpu-raster keeps the reference's f64
  model. A self-test checks the parser; planting a renamed `face_axes` and a `shade_lit`
  call in gpu-splat made it fail as intended.
- **What it found, now fixed:** `fn occluded(...) { return ctrace(...).hit; }` written
  into five back ends (one `visibility::OCCLUDED_WGSL`, and four hand-assembled
  "storage + refloor" walks now `visibility::occlusion_wgsl`); gpu-voxelspace's own
  `pixel_ray` and, beyond what the guard sees, the primary-ray formula inline in nine
  places (gpu-compute, gpu-rt, gpu-splat, beam prepass, discovery ×3, the face drawer,
  voxelspace): now `camera_ray` / `camera_dir` in `common.wgsl`. The heightmap storage
  offers its walk as `ctrace`.
- **Checks:** bit-identical images on every GPU back end (incl. beam, incremental
  discovery); GPU ms within ±0.8%, fps within ±2.3% (both binaries built with the new
  forced alignment).
- **Not covered by the guard:** copies inside function bodies (as the inline ray formula
  was) and Rust-side duplicates; a body-level similarity check would be the next level.

### 2026-10-12: architecture refactor step 3 (secondary rays through the composed storage)
- **Heightmap storage component** (`components/src/storage/heightmap`): the ring building
  both voxelspace back ends had separately, plus a ray walk through the heightmap on the
  GPU (`hm_trace`) and the CPU (`Heightmap::trace`): a 2D DDA over the columns (side faces
  when entering below a top, tops when descending below them), skipping 32×32 tiles the
  ray passes above (per-tile maxima after the entries) and stopping rising rays above the
  highest top. Without the skip, sun shadow rays cost 196 ms per frame; with it 7.6 ms.
  Unit test for the CPU walk.
- **Voxelspace:** knob `shadows` (default all, as elsewhere); shadow rays walk the
  heightmap; gpu-voxelspace offers pathtrace (bounces walk it too). Terrain lit GPU ms:
  2.67 (none) → 5.18 (sun) → 7.6 (all); PSNR 17.2 → 17.6 dB, bad pixels 26.7 → 24.9%;
  CPU and GPU give identical scores. cpu-voxelspace lit 119 → 95 fps.
- **cpu-discover:** its GPU shadow rays walk a composed GPU storage with refloor (knob
  `shadow_storage`, owned by shading, default bricks) instead of the voxel world's DDA:
  pixel-identical with bricks, lit GPU 2.30 → 2.19 ms; prefix 2.02, tree64 2.95 (98 MB GPU
  instead of 535: region storages skip the pool). The storage slot stays the CPU
  structure discovery walks: two storages on two devices wait for per-stage slots (step
  7). Pathtrace on cpu-discover: 10.9 ms forward, 2.6 with `draw = visbuffer`.
- **Removed:** the voxel world's own DDA (`trace` in `voxels.wgsl`, 71 lines,
  `DDA_OCCLUSION_WGSL`) and the mesh shading's default walk (a composed walk is now
  required). Every shadow and bounce ray now goes through a storage component.
- **Checks:** pixel-identical to the step-2 build where behaviour is unchanged (cpu-discover,
  gpu-compute, gpu-raster, gpu-splat, meshes, voxelspace with shadows off). GPU ms within
  ±2% (cpu-discover −4…−6%). fps of gpu-compute and gpu-raster read −6…−8.5% on terrain;
  with forced function / block alignment both builds are within ±1.4%: code layout
  again (second time). Suggestion for the user: build measured binaries with forced
  alignment by default.
- **Found, not fixed:** cpu-discover's CPU discovery calls the composed storage through
  `dyn World` (a virtual call per discovery ray; `grid.rs`): a composition cost the `cpu`
  back end avoids by being generic over the storage. Candidate for a later step (make
  discovery generic over the storage, or compile it on demand, step 8).
- **Slip:** 3b was committed with the registry test failing (it hard-coded `direct` for
  cpu-discover's shading); fixed in the next commit. Commits now run only after the
  tests pass.

### 2026-10-11: architecture refactor step 2 (one face drawer)
- **2a, shared helpers:** `face_axes` in `common.wgsl` (4 copies, incl. discover.wgsl's
  `face_axes_d`); the derived storage query `s_block_at` (3 copies); `pixel_ray`,
  `face_plane_hit` and `face_surface` in the drawer module. `face_surface` keeps the drawn
  face instead of inferring it from the ray: one grazing pixel of cpu-discover depended on
  it (checked against the reference).
- **2b, one face drawer:** `visibility/faces.wgsl` owns the vertex stage and the hit
  construction; `raster` / `visbuffer` / `gbuffer` supply the fragment and compute entry
  points for any **face source**. A face source (gpu-raster's geometry instances,
  gpu-discover's face set, cpu-discover's face list) provides `src_vertex`,
  `src_origin`, `src_block`, `src_lookup` (merged quads) and `src_view` (its diagnostic
  tints). Fragment data comes from the records (gpu-raster: extra vertex outputs cost its
  36-vertex cubes 8–15%) or from flat varyings (discovery: a record read cost
  cpu-discover's cheap unlit fragments 15–17%), chosen per source (`faces_wgsl`). Where
  the ray runs along the face plane (horizon rows) the fallback point now comes from the
  fragment's depth (free): fixes one pixel per frame on gpu-raster naive / cubes lit.
- **2c, host side and new choices:** `visibility::drawer::FaceDrawer` owns the pipelines
  (forward fill / prepass / equal; deferred ids or G-buffer plus the compute shade
  pass), the deferred targets and the passes, for all three back ends. gpu-discover and
  cpu-discover get the live knob `draw` = raster | visbuffer | gbuffer. Terrain lit GPU
  ms: cpu-discover 2.30 → 0.62 (visbuffer), gpu-discover 3.57 → 2.50, gpu-discover
  pathtrace 11.7 → 4.6 (deferred needs no depth-prepass rule); unlit slightly slower
  (cpu-discover 0.19 → 0.31). Same PSNR in every draw mode (merged rows: 49.9 dB in all
  three). Painter order and the overdraw view stay forward (an error names the reason).
  Likely cause of cpu-discover's forward cost: unit-face quads are small on screen, so
  2×2 fragment quads run many helper lanes, each paying for shadow rays.
- **Zero-cost check** (interleaved against the step-1 build, terrain, 3–8 rounds): GPU
  ms within ±1.2% on every changed composition (gpu-discover lit −4…−5%). Forward images
  pixel-identical apart from the fixed pixels; gpu-discover's merged runs and overdraw
  view differ between runs of the same old build too (GPU-ordered lists).
- **Measurement lesson:** gpu-raster fps read −5% (host-bound, CPU mesher +8%) in one
  build with no mesher change; with `-C llvm-args=-align-all-functions=6
  -align-all-nofallthru-blocks=5` both builds were equal, and the next build lost the
  effect. Host-bound fps can move ±5% from code layout alone; a candidate for the
  zero-cost check is building both sides with forced alignment.
- **Remaining duplicates** across back-end shaders: only the face-source functions (each
  reads its own record format) and the compute entry points named `main`.
- **Full sweep after steps 1–2** (`target/sweep/20261011-1700`, 3,735 observations, the
  same 3 known failures on far) against `baselines/sweep-20261011/` (3,047 shared rows):
  GPU ms median ratio 1.0002 (per back end 0.97–1.01), 78% of rows within ±2%, 89%
  within ±4%; no PSNR drop above 0.5 dB; 688 new rows (pathtrace on more back ends).
  Interleaved re-runs of the worst rows against a build of d52b9fd: most are noise
  (gpu-compute + meshes ±1%; distance storage on edits / far and gpu-raster gbuffer on
  terrain / edits swing ±10–30% between runs of either build, some bimodal). **One real
  shift:** gpu-compute **pathtrace** kernels moved −9…+24% (median +0.6%), deterministic,
  from step 1; the worst is many-lights `prefix + rowrun + pathtrace` 22.6 → 28.0 ms
  (bricks + refloor +1.8%, gpu-rt +1.0%). Not the bounce loop's shape and not the
  separate `sp` uniform (both tried); most likely NVIDIA register allocation of the
  largest kernels (walk + 400-light loop + bounces). **Accepted as codegen variance**
  (the user, 2026-10-12): an optimisation pass over what exists will come after more
  growth.

### 2026-10-11: architecture refactor step 1 (one shading implementation per kind)
- **Interface:** shading reads a `Surface` (camera-relative point, normal, block-local
  point, block, face): `shade_surface(s, d, lit, px)` on the GPU (`Surface` and the ray
  walks' adapter `surface_of` in `voxels.wgsl`), `direct::cpu::shade(ctx, &Surface, mode)`
  on the CPU (`Surface::at_cell` for ray walks). A surface instead of a ray hit, so that
  gpu-splat's clamped point and voxelspace's slope normal fit without special cases.
- **Moved onto it:** gpu-splat (`splat.wgsl`), gpu-voxelspace (`vs.wgsl`, slope normal in
  lit mode), cpu-discover (`raster.wgsl`) and cpu-voxelspace (now ambient + sun + point
  lights, no shadows, i.e. gpu-voxelspace's model; the two differ on 0.005% of lit pixels).
- **cpu-raster keeps the reference's f64 shading** (the user's rule: same algorithm, a
  fundamental difference, here precision). Tried the f32 component: lit PSNR fell
  (terrain 63.1 → 57.7 dB, room 99.0 → 76.6, many-lights 64.8 → 61.0) at equal speed.
- **Shading state:** `shading::state::ShadeState` owns what pathtrace keeps across frames
  (its uniform: width, frame, samples so far, bounces, spp; the accumulation buffer), its
  knobs, WGSL declarations and binding entries; back ends reserve two bindings and call
  `begin` per frame (nothing per frame for `direct`). Replaces the Accum copies of
  gpu-compute and gpu-rt.
- **Pathtrace on more back ends:** gpu-splat, gpu-raster (raster, visbuffer, gbuffer) and
  gpu-discover. Fragment shading with per-pixel state forces early depth tests
  (`@early_depth_test(force)`, wgpu `SHADER_EARLY_DEPTH_TEST`, requested as optional) and
  needs the lit depth prepass (and depth order on gpu-discover) so exactly one fragment per
  pixel updates the accumulation; otherwise the render fails with the reason (not yet
  greyed in the GUI: knob-level conflicts have no model before step 6). Checks: PSNR
  30.3 dB on every back end (as gpu-compute; the reference is direct-only); `bounces = 0`
  equals `direct`; raster vs visbuffer pathtrace differ on 0.01% of pixels (edges).
  Cost (terrain lit, GPU ms): gpu-raster raster 5.8 vs visbuffer 2.9 (fragment path
  tracing pays for quad shading), gpu-discover 11.7 (direct 3.9), gpu-splat 3.9 (1.7).
  Not offered: cpu-discover and voxelspace (their shadow walks are not the composed
  storage; step 3).
- **Zero-cost check:** every direct image (and pathtrace on gpu-compute, gpu-rt) is
  pixel-identical to the pre-step build. GPU ms, interleaved against that build (terrain,
  300 frames, 3–6 rounds): 32 passes over 16 compositions within ±1%, except gpu-raster
  `raster` lit +0.8% (consistent), gpu-voxelspace unlit +1.0–1.5%, cpu-discover unlit
  +1.6% (3 µs). Lesson: one `shade_surface` call with a runtime `lit` flag compiled
  slower on NVIDIA than the old inline branch (gpu-voxelspace unlit +6%, cpu-discover
  +4.5%); calling it twice with a constant flag (the caller branches) fixed both.
  gpu-raster and gpu-rt fps read 1.5–2% lower (host-bound; within the ±4% noise rule).
  CPU (one CCD): `cpu` +0.7..3% fps, cpu-voxelspace unlit +1%, lit −20% (the point
  lights it now computes). Not yet run: the full sweep against the baseline (other
  scenes); the A/B above is against the build of d52b9fd.

### Notes from the user (2026-10-10)
- **A colour scale for every diagnostic view,** like a thermal camera: the colour ramp
  with numeric values (steps, rays, overdraw, …), so a view shows how much work is done
  where, not only where relatively. Applies to gpu-compose `steps`, gpu-discover
  `tiles` / `rays` / `overdraw`, cpu-cluster views, and future ones.
- **Lit / unlit and meshes may become components,** but without forcing each affected
  component to implement four variants. Likely design: lighting and meshes as stages
  around the visibility walk (meshes: composite before / cap the walk, as
  `MeshComposite` already does; lighting: the shading component takes the mode),
  so storages and walks stay unaware of them.
- **A sweep option in plans** (e.g. `compose = { storage = "*", visibility = "*" }`)
  expanding to every valid combination, for overnight benchmarks of the whole space.

## Future additions (backlog by category)

A menu to choose from, not a commitment. The user will add their own; ideas the user
voices also go into IDEAS.md with analysis.

### New renderers and techniques
- Hardware RT with ray-tracing pipelines (closest-hit and any-hit shaders) instead of
  ray queries.
- Mesh shaders: chunk meshlets with GPU-driven culling.
- GPU-driven raster: Hi-Z occlusion culling of chunks, two-pass occlusion (Nanite-style).
- Cone tracing over the prefix or occupancy hierarchy (one cone per tile, rays only at
  surfaces).
- Brickmap with a 3D-texture atlas (hardware-filtered bricks) as a GPU storage variant.
- Surface caching or radiance caching (lighting stored per face, reused across frames).
- Splatting with proper footprints (EWA-style) to fix `gpu-splat`'s fattened edges.
- A CPU renderer from the user's §1 traversal, plus SIMD packets once the per-step cost
  is low.
- Software rasteriser variants: binned tiles, coverage masks (like the GPU's).
- An octree sparse voxel renderer with child-pointer-free (implicit) layouts.
- Hybrid: ray-traced near field plus a rasterised far field (or the reverse) with a depth
  composite.

### Optimising existing renderers
- An incremental-level hierarchical DDA for every ray caster (the §1 traversal).
- A hierarchical, brick-level beam pre-pass (`gpu-beam`).
- `gpu-clipmap`: power-of-two slots (masks instead of `%`), a uniform-chunk flag, an
  occupancy mip.
- `gpu-svo-dag`: free or garbage-collect nodes after edits; a parallel merge into the
  global table.
- `gpu-splat`: per-chunk sub-allocation, incremental uploads, frustum culling.
- `gpu-pathtrace`: a temporal denoiser, jittered primary rays, blue-noise sampling,
  emissive blocks.
- Fewer instructions per step on the CPU: register spills, saturating float→int casts,
  bounds checks and `powf` in the hot loop.
- Greedy merge: a cheaper per-fragment lookup (voxel slot per chunk instead of a hash
  probe); binary bitwise greedy meshing.
- LOD for the rasterisers and trees, and LOD that reflects edits.

### Discovery line (IDEAS §3, §5, §7, §8, §10, §11)
- Tune or adapt `keep_unseen`: the overdraw view shows incremental keeps ~2× the faces it
  needs on occlusion-heavy scenes.
- Cut moving-frame fixed cost: fuse `mark` and `need_raw`, quarter-resolution depth for
  the edge test, a one-sided parallax test, a persistent kernel (3b) for level latency.
- Event-driven discovery: skip when static, edge strips for pans, hysteresis.
- Incremental greedy merge of the discovered face set.
- Morton-ordered breadth-first queues; a persistent kernel.
- Face-cluster discovery in clip-space fixed point, with a sparse sky pass and far-field
  consolidation.
- The "heads" explorer with movement rules that keep trails disjoint.
- Back-to-front order without depth testing.
- Max-height mipmaps and temporal ray start distances (IDEAS §4).

### Lighting and shading
- ~~Mesh mode (IDEAS §13)~~: core done (step 7). Next: mesh shadows (7d), more renderers
  (7b, 7c), skinned or textured meshes.
- Baked or propagated flood-fill light (Minecraft-style) on the quality/speed curve
  against ray-traced shadows.
- Ambient occlusion: per-vertex voxel AO versus ray-traced AO.
- Soft shadows from area lights; coloured light propagation.
- **Transparent and translucent blocks** (glass, water, leaves with alpha), which every
  renderer would need to handle. The user named this the other remaining big feature,
  to tackle at a later date (2026-10-09).
- Volumetric fog and god rays through the voxel grid.
- Reflections (screen-space versus ray-traced) for water and glossy blocks.

### World and content
- Larger worlds and longer streaming runs; disk-backed chunk storage.
- More generators: caves-only, cities, overhang-heavy, very sparse scenes (floating
  islands).
- Non-cubic blocks (slabs, stairs) as a stress test of each technique's assumptions.
- Recorded camera paths from GUI sessions (fly around, save as a plan path).

### Benchmarking, metrics and the tuner
- Modular composition as a tuner knob (IDEAS §12).
- The tuner filtering renderers by mode flags (lighting, meshes).
- Perceptual metrics (FLIP, LPIPS-style) beside PSNR and MS-SSIM.
- Temporal stability metrics: flicker and popping between frames, which matter for
  discovery and LOD.
- Frame-time pacing metrics (variance, stutter) beside the averages.
- Power and energy per frame (GPU board power, CPU package power).
- An automatic Pareto front of speed against quality across renderers and settings.
- A catalogue report as an HTML page with charts and image diffs.

### GUI and usability
- Latching **Lit** and **Meshes** toggles (IDEAS §13).
- A composition editor once the modular slice exists.
- A side-by-side split view with a draggable divider, and a diff overlay live in the
  viewport.
- Saving screenshots and short frame captures from the GUI.
- Per-renderer knob presets, saved and loaded.
- Debug views (the user values these highly): done so far are gpu-compose `steps` and
  gpu-discover `tiles` / `rays` / `churn`. Candidates:
  - the hierarchy level each ray finished at (gpu-compose);
  - chunk-cache misses per pixel;
  - face ids;
  - LOD levels;
  - overdraw for the rasterisers;
  - per-pixel shadow-ray cost in lit mode;
  - the CPU renderers' step counts.

### Output and display
- An HDR output pipeline (the panel supports it): linear float output from renderers,
  tone mapping in one place.
- Temporal upscaling or anti-aliasing (TAA- or FSR-style) as a host-side stage.
- VR or stereo rendering (two views) as a stress test.

### Infrastructure and code health
- Fix the 2 GiB `GrowableBuffer` rounding issue.
- Continuous checks: a script running builds, tests and a short catalogue, comparing
  against stored baselines.
- Shared GPU modules (the start of §12): one place for chunk tables, shading and
  timestamps. *(Started: gpu-compose.)*
- Pipeline statistics (registers, spills) per shader, to explain compiler pathologies
  like the NVIDIA incremental slowdown. Needs Vulkan's
  `VK_KHR_pipeline_executable_properties`, which wgpu doesn't expose.
- Golden-image tests per renderer, to catch regressions in the incorrect-frames
  category.
