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
- **New rendering techniques are built as components** (storage, visibility, geometry,
  shading, back end; IDEAS §12 design), not as new monolithic renderers. Existing
  renderers are ported by yield and retire only when a composed equivalent matches them.
- No subagents until the user re-enables them. Usage is self-metered.
- Ideas the user shares go into `IDEAS.md` with analysis. Implement only after asking.
- Verify everything: zero warnings, unit tests, GPU end-to-end tests, a comparison
  against the reference renderer, and interleaved repeats for performance claims.
  Report negative results honestly.
- **One step at a time** (the user's request after hitting a rate limit, 2026-10-10):
  finish a step, verify, commit and push, report, and wait for the go before the next.
- Priority: the component refactor (phases 3–5 below), then a wider space of
  techniques, each built as components. The user prefers new techniques over completing
  coverage of a feature across every renderer (7b is shelved for that reason).
- The user values diagnostic `view` modes highly; add them where a component has
  something to show (with a numeric colour scale once that exists).

## How to verify and measure

- Builds, zero warnings: `cargo build` and `cargo build --release`.
- Unit tests: `cargo test -q --workspace` (41, including: the component registry
  matches its implementations; storages are exact; the CPU walks agree; every shipped
  plan loads).
- GPU end-to-end tests: `cargo test -q --bin multirenderer -- --ignored --skip tuner_demo --skip scale_curve` (3, including mesh mode).
- `multirenderer list`: back ends, components by slot, legacy renderers.
- One composition against the reference:
  `./target/release/multirenderer run plans/terrain.toml -s 'renderer=[{compose={backend="gpu-compute",storage="prefix",visibility="refloor"}}]' -s output.dir=\"target/check\"`
  - A legacy renderer: `{name="gpu-rtx"}`. Further knobs go in `options={...}`.
  - CLI views: `-r gpu-compute:prefix+incremental`, `-r cpu:hier+refloor`,
    `-r gpu-raster:bricks+cubes+visbuffer` (the back end's slots in order).
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
- `gpu-svo-dag` never frees DAG nodes after edits. A full rebuild is meant to kick in at
  2× growth plus 65,536 nodes, but that path is untested.
- `gpu-splat` re-uploads every point on any change (slow loads and frames); it has no
  culling.
- `cpu-discover` uploads the whole face list every frame, not incrementally.
- `gpu-voxelspace` lit mode is slower than gpu-dda.
- `cpu: prefix` has higher peak RSS than `cpu: hier` despite less world data, probably
  transient build allocations; not investigated.
- The ~0.1 dB image difference of `cpu: prefix + refloor` comes from 2-cell steps
  rounding differently at grazing edges. It goes both ways against the reference.
- The incremental walk on the GPU is 3.5–8× slower than refloor on NVIDIA only (AMD
  1.27×), for an unknown compiler reason (IDEAS §1 "Result 3").

## Status (2026-10-10)

- **Architecture:** renderers are assembled from components (IDEAS §12):
  **back end × storage × geometry × visibility × shading**; back ends declare which slots
  they have (geometry only on raster).
  - `components/` (crate `mr-components`): one directory per piece, each with its GPU
    (`gpu.wgsl`) and/or CPU (`cpu.rs`) implementation, plus the registry, compatibility
    rules and `Composition`.
  - `backends/gpu-compute`, `backends/cpu` and `backends/gpu-raster`: the assemblers.
  - Plans use `compose = { backend, storage, geometry, visibility, shading }` (missing
    slots take the back end's defaults). The GUI chooser shows the back end's slots and
    disables combinations that can't run, with the reason.
- **Available components:**
  - gpu-compute: storages bricks, hier, prefix, distance, rows, tree64, svo_dag,
    clipmap; prepass none, beam; visibility refloor, incremental, sphere, rowrun;
    shading direct, pathtrace (128 combinations);
  - cpu: storages bricks, hier, prefix, u8, bits, palette; visibility refloor,
    incremental; shading direct (12 combinations);
  - gpu-raster: geometry greedy, naive, cubes; visibility raster, visbuffer; storage (for
    shadow rays) the eight GPU ones; shading direct (48 combinations).
- **Legacy renderers** (not yet split into components): cpu-reference (stays a
  monolith on purpose), cpu-raster, cpu-simd, cpu-simd-loc (4), cpu-voxelspace,
  cpu-discover, cpu-cluster, gpu-hybrid,
  gpu-frag-dda, gpu-discover, gpu-rtx(-bricks),
  gpu-splat, gpu-voxelspace.
- **Verification:** zero warnings in both builds, 41 unit tests and 3 GPU end-to-end
  tests pass.
- **Tools:**
  - GUI with the component chooser, live knobs, hot swaps, quality probes and an
    auto-tuner (local search, resolution controller, Bayesian). Tooltips everywhere.
  - Diagnostic views: gpu-compute `steps`; gpu-discover `tiles` / `rays` / `churn` /
    `overdraw` / `quads`; cpu-cluster `clusters` / `kinds`.
  - Benchmark plans in `plans/` (compositions where exact equivalents exist), catalogue
    scripts in `scripts/`, hardware counters on the `cpu` back end (`perf`).

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

### Current: the component refactor (IDEAS §12)

Goal (the user's): no chooser of monolithic renderers at all. Pieces in files; a renderer
is a back end plus one component per slot; the UI forbids unsupported combinations.

1. ~~Components crate, GPU-compute back end, compositions in plans / CLI / GUI~~ (done).
2. ~~CPU back end~~ (done).
3. ~~The GPU-raster back end~~ (done 2026-10-10, except the discovery components; see
   the progress log). What it needed:
   - **A `geometry` slot**, which turns storage into drawable faces:
     - `greedy`: greedy quads, from `crates/mr-mesh` (`mesh_chunk_merge`, `Merge::Block`
       / `Merge::Exposure`), as gpu-raster uses;
     - `naive`: one quad per exposed face (gpu-raster-naive);
     - `cubes`: whole cubes (gpu-cubes).
   - **Raster visibility components:**
     - `raster`: depth-tested rasterisation of the geometry (gpu-raster's culled
       multi-draw-indirect path);
     - `visbuffer`: a visibility buffer with deferred shading (gpu-visbuffer);
     - later the discovery components (`quadtree` from gpu-discover / cpu-discover,
       `cluster` from cpu-cluster), which produce a face set that is then rasterised.
   - **Shading `direct` on the raster back end:** the fragment shader's texel / lit
     shading (shared `shade_lit`, shadow rays through `VoxelWorld`).
   - **Meshes:** `mr_gpu::meshes::GpuMeshes` (gpu-raster already uses it).
   - **Compatibility rules gain slot dependencies:** the raster back end needs a
     geometry component; ray-walk back ends have no geometry slot. Generalise
     `mr_components::unavailable` / `Composition` so that slots can be "not applicable"
     per back end (and shown so in the GUI).
   - **Then retire** gpu-raster, gpu-raster-naive, gpu-cubes and gpu-visbuffer once
     their compositions match them, and update the plans.
   - **Still open:** the discovery components (`quadtree`, `cluster`) as raster
     visibility; a candidate for its own step.
4. **Current:** port the remaining unique pieces (the user, 2026-10-10: "we remove
   technical debt"). Sub-steps, each verified, committed and pushed on its own:
   - 4a. ~~Retire what compositions already cover~~: gpu-dda (plain), gpu-sdf,
     gpu-bitmask, the gpu-compose alias (done).
   - 4b. ~~tree64 as a storage~~ (done; retired gpu-tree64).
   - 4c. ~~SVO-DAG as a storage~~ (done; retired gpu-svo-dag).
   - 4d. ~~clipmap as a storage~~ (done; retired gpu-clipmap).
   - 4e. ~~the beam prepass as a stage~~ (done: slot `prepass`; retired gpu-beam).
   - 4f. ~~path tracing as a shading component~~ (done; retired gpu-pathtrace).
   - 4g. ~~LOD in the storage interface~~ (done; retired gpu-dda-lod and the gpu-dda crate).
   - 4h. hardware RT as a back end (gpu-rtx, gpu-rtx-bricks).
   - 4i. fragment-shader dispatch and the hybrid G-buffer (gpu-frag-dda, gpu-hybrid).
   - 4j. CPU: cpu-simd, cpu-simd-loc layouts and tiles, cpu-raster.
   - 4k. discovery as raster / CPU visibility (gpu-discover, cpu-discover, cpu-cluster).
   - 4l. their own representations: voxelspace (gpu, cpu) and splats.
   cpu-reference stays a monolith on purpose.
5. Remove the legacy section and the monolith crates.

Also wanted, sized for single steps:
- Colour scales on the views.
- Lighting / meshes as stage components.
- A plan sweep over all combinations.

(All three are in the notes below, and the backlog.)

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
