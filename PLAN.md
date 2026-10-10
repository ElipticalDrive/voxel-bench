# Plan and progress

The working state of the project for future sessions: what exists, what was learned, what
is next. Ideas and their analysis live in `IDEAS.md`; usage and architecture live in
`README.md`.

## Working agreements

- Repository: https://github.com/ElipticalDrive/voxel-bench (public, branch `main`),
  created 2026-10-09. Commits use the repo-local identity (GitHub noreply address).
  Commit and push only when asked.
- The project is about exploring the problem space, not chasing speed. Slower techniques
  and negative results are findings too.
- No subagents until the user re-enables them. Usage is self-metered.
- Ideas the user shares go into `IDEAS.md` with analysis. Implement only after asking.
- Verify everything: zero warnings, unit tests, GPU end-to-end tests, a comparison
  against the reference renderer, and interleaved repeats for performance claims.
  Report negative results honestly.
- Priority: more renderers and optimising existing ones, then the discovery line of
  work.

## How to verify and measure

- Builds, zero warnings: `cargo build` and `cargo build --release`.
- Unit tests: `cargo test -q --workspace` (31).
- GPU end-to-end tests: `cargo test -q --bin multirenderer -- --ignored --skip tuner_demo --skip scale_curve`.
- One renderer against the reference:
  `./target/release/multirenderer run plans/terrain.toml -s 'renderer=[{name="gpu-dda"}]' -s output.dir=\"target/check\"`
  - Options go in `options={...}`.
  - Use `-s run.repeats=3` for interleaved repeats, `-s run.frames=N` (more frames
    means a slower camera on the same path), and `-s run.modes=[\"unlit\"]`.
  - Results are in `<dir>/<stamp>/report.json` (a list of passes with `per_frame`
    counters).
- CPU performance: pin to one CCD (`taskset -c 0-7,16-23`) and read the `perf=true`
  hardware counters (`cycles_per_px`, `instructions_per_px`, `lane_steps_per_pixel`)
  rather than fps alone.
- Whole catalogue: `scripts/suite.sh`, then
  `python3 scripts/summarise.py target/catalogue`.
- Tuner on real terrain: `ALGO=bayes TARGET_FPS=900 SECONDS=60 cargo test --release --bin multirenderer tuner_demo -- --ignored --nocapture`.

## Known issues (not yet fixed)

- Some renderers produce incorrect frames. The user knows; fix later.
- `GrowableBuffer` (`crates/mr-gpu`) rounds sizes up to powers of two, so a 2 GiB
  request exceeds the storage-binding limit (2^31 − 4). `gpu-raster-naive` caps itself at
  2040 MiB to avoid it.
- `gpu-raster-naive` with `cull = false` overflows its buffer on terrain and edits and
  renders only part of the scene.
- `gpu-svo-dag` never frees DAG nodes after edits. A full rebuild is meant to kick in at
  2× growth plus 65,536 nodes, but that path is untested.
- `gpu-frag-dda` reported identical GPU times for lit and unlit on terrain, which is
  suspicious; its timestamps may measure something different.
- `gpu-splat` re-uploads every point on any change (slow loads and frames); it has no
  culling.
- `cpu-discover` uploads the whole face list every frame, not incrementally.
- `gpu-voxelspace` lit mode is slower than gpu-dda.
- `cpu-simd-prefix` has higher peak RSS than hier despite less world data, probably
  transient build allocations; not investigated.
- The ~0.1 dB image difference of `cpu-simd-prefix` comes from 2-cell steps rounding
  differently at grazing edges. It goes both ways against the reference.

## Status (2026-10-09)

- **Renderers:** 35 in the registry. See the README table. Some produce incorrect
  frames; the user knows and will deal with them later.
- **Verification:** both builds have zero warnings, all 28 unit tests pass, and both GPU
  end-to-end tests pass.
- **Tools:**
  - GUI with live knobs, hot swaps, quality probes and an auto-tuner (local search,
    resolution controller, Bayesian), with explore → hold → resume, noise-aware
    acceptance, and stop-restores-best. Tooltips everywhere.
  - Benchmark plans in `plans/`, catalogue scripts in `target/catalogue/`, and hardware
    counters on the `cpu-simd-*` renderers (`perf` / `counters` knobs).

### Key findings so far

- **CPU traversal is instruction-bound, not memory-bound.** About 80 cycles and 220
  instructions per step at IPC 2.6; cache misses are ≤3% of the time.
- **Steps per pixel are the lever.**
  - `cpu-simd-hier`, with one 8³ level, is the fastest CPU caster (+29% on edits).
  - `cpu-simd-prefix` (the user's structure) takes the fewest steps, 31–51% fewer than
    u16, but loses 3–9% per pixel to hier under a re-flooring traversal.
  - Under the incremental-level traversal (step 1, IDEAS §1 "Result 2") each move costs
    ~30% fewer instructions, but failed ascent tests add work. It wins on terrain (prefix
    +13–17%, hier +5–10%) and loses on short rays (menger −3–12%). Prefix comes within 2%
    of hier but does not pass it.
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

Estimates are rough, in hours of my working time. For scale, the §2 + §1 + §3 stretch on
2026-10-09 was about 2½ hours. The user can judge what share of a usage window that was;
each step below is sized relative to it. Steps that need new correctness proofs or tests
sit at the high end.

| # | Step | Time | vs. the 2½ h stretch | Notes |
|---|---|---|---|---|
| 1 | ~~Prefix structure with an **incremental-level hierarchical DDA**, CPU~~ | done | | Done 2026-10-09 as the `traversal` knob on every `cpu-simd-*` variant. Mixed result; see IDEAS §1 "Result 2". |
| 1b | The same on the GPU, as a new renderer | 1.5–2.5 h | ~¾ | Step 1 did not win clearly on the CPU, but the GPU's costs differ, and the project is exploratory. Fold it into 6′. |
| 2 | **Event-driven discovery** in gpu-discover: skip when nothing changed, edge strips for pure pans, hysteresis for a stable set | 1–2 h | ~½ | Directly addresses the churning face count. |
| 2b | **Incremental greedy merge** of the discovered set (re-merge only changed chunks) | 1.5–2.5 h | ~¾ | Worth it once 2 makes the set stable. |
| 3 | **Morton-ordered breadth-first work queues** in gpu-discover (§8) | 1–1.5 h | ~½ | Attacks the measured ray incoherence. |
| 3b | Persistent kernel with work stealing | 2–3 h | ~1 | Riskier: driver and forward-progress caveats. |
| 4 | **Back-to-front order without depth testing** (§11), as a gpu-discover knob | 0.5–1 h | ~¼ | One measured experiment. |
| 5 | **Face-cluster discovery prototype** (§10), CPU, clip-space fixed point (§7), sparse sky pass, far-field consolidation | 3–5 h | 1½–2 | A new renderer; needs careful completeness tests. |
| 6 | **Modular library, first step** (§9): swappable storage + traversal with per-query conversions, for a few ray casters | 3–6 h | 1½–2½ | The full library takes several sessions. |

Further steps:

| # | Step | Time | vs. the 2½ h stretch | Notes |
|---|---|---|---|---|
| 6′ | **Storage + traversal slice** of §12, done right after step 1. The prefix structure is its first GPU plugin, which absorbs 1b. | 3–4 h | ~1½ | About 2–3 h more than 1b alone; afterwards every storage pairs with every traversal. |
| 7 | **Mesh mode, core** (§13): mode flags and capabilities, mesh world updates and IPC, plan animation, triangle support in cpu-reference, GUI latching toggles, tuner filtering | 3–4 h | ~1½ | Nothing renders meshes yet except the reference. |
| 7b | Meshes in the raster renderers (gpu-raster first) | 0.5–1 h each | ~¼ each | Same depth buffer. |
| 7c | Mesh compositing module for ray casters (raster meshes first, rays capped at mesh depth) | 1.5–2.5 h | ~¾ | Gives every GPU ray caster on the slice mesh support at once. |
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

**Order:** ~~1~~ (done) → 6′ → 2 → 3 → 4 → 2b → 7 → 7c → 7b → 5 → rest of 6. Step 7 can move
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
- Event-driven discovery: skip when static, edge strips for pans, hysteresis.
- Incremental greedy merge of the discovered face set.
- Morton-ordered breadth-first queues; a persistent kernel.
- Face-cluster discovery in clip-space fixed point, with a sparse sky pass and far-field
  consolidation.
- The "heads" explorer with movement rules that keep trails disjoint.
- Back-to-front order without depth testing.
- Max-height mipmaps and temporal ray start distances (IDEAS §4).

### Lighting and shading
- Mesh mode (IDEAS §13): animated polygons with the voxel scene.
- Baked or propagated flood-fill light (Minecraft-style) on the quality/speed curve
  against ray-traced shadows.
- Ambient occlusion: per-vertex voxel AO versus ray-traced AO.
- Soft shadows from area lights; coloured light propagation.
- Transparent and translucent blocks (glass, water, leaves with alpha), which every
  renderer would need to handle.
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
- Debug views: step-count heat maps, discovery tiles, face IDs, LOD levels.

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
  timestamps.
- Golden-image tests per renderer, to catch regressions in the incorrect-frames
  category.
