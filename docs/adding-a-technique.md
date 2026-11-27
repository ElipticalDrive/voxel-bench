# Adding a technique

A technique is a component of a role (a storage, a walk, a shading, a drawer, ...) or, when
it is a new shape, a region with its own components. Adding one is: write the component,
register it, run what checks it, and let the rest be derived. This page is the recipe per
kind of addition, kept true by doing each kind once (the measurements below are from those
runs). Kinds not yet rehearsed say so.

What is derived from the registry entry alone (`crates/mr-components/src/lib.rs`
`COMPONENTS`), with no further edit: the node choices of every graph (`Graph::choices`), the
graph editor's and the tuner's choices, `*` in sweeps, `multirenderer list` and `audit`, the
catalogue rows, the reference class (from the caps), the knobs' owners where the knob is
owned by a cap. `src/contract.rs` proves it per role with a probe component; a listing that
needs a hand edit per component is that probe's `debt`.

## A CPU storage (rehearsed with `hashed`)

1. `scripts/new.py storage NAME "description"` scaffolds a working copy of `bricks` in
   `crates/mr-components/src/storage/NAME/` and adds the line every hand-kept list needs
   (registry, module list, the CPU dispatch and type names in `mr-exec-cpu/src/raycast.rs`,
   discovery's world in `visibility/discover/grid.rs`, the exactness, occupancy and walk
   agreement tests). `scripts/new.py sites` lists them; `scripts/new.py remove storage NAME`
   undoes exactly that.
2. Write `cpu.rs`: `Repr` (`build`, `probe`, `LEVELS`, `occupied`, `block`, the byte counts;
   `CAPS` if it provides more than `Voxels`). `occupied` must be exact at every level in
   `LEVELS`; `block` must answer every cell, see-through blocks too.
3. `scripts/new.py try NAME --sweep`: the component crate's tests (exactness against the
   dense chunk on random and edge chunks, every walk agreeing with the reference walk over
   it), the registry test, `scripts/check.py`, then a run of `raycast-cpu` with it. If only
   the sweep-size tests fail (the new component joins every `*` sweep),
   `scripts/new.py accept-sweep` rewrites the size.
4. Pixel check where it joins: `scripts/run1.py` with the storage on `raycast-cpu`,
   `discover-cpu` and `raster-cpu`'s shadow rays (the plan's compare against the reference).
5. Docs: the component's description is the catalogue's and the GUI's text; nothing else
   lists storages by name.
6. Timing (background): `.agent/bin/idle add -- env OUT=target/sweep/NAME scripts/sweep.py
   terrain` with `vary` on the storage, then the guard against the baseline.

Measured (`hashed`, 2026-11-26): 6 files touched by the scaffold outside the component
(6 hand-kept list lines plus the registry block), none by hand; about 15 minutes of work
(most of it the representation itself); its tests green the first time. Pixels on terrain:
`raycast-cpu` 63.6 dB unlit / 59.5 lit, `discover-cpu` settled 55.2 / 52.0, as with the other
storages.

GPU storages are not scaffolded: a GPU storage is a WGSL module (`storage/NAME/gpu.wgsl`,
the `s_probe` / `s_occupied` / `s_block` interface, `adapters.wgsl` for what it lacks) and
its upload in `storage/gpu_data.rs`. Not rehearsed yet.

## A GPU shading (rehearsed with `ao`)

1. Write `crates/mr-components/src/shading/NAME/gpu.wgsl`: `shade_point(albedo, p, n, d,
   lit, px)` and `shade_surface(s, d, lit, px)`; with transparency (`Cap::Layers`) also
   `radiance_point` / `shade_radiance` under `#ifdef LAYERS`. Available: `direct_light`,
   `occluded` (shadow rays), `btrace` (secondary rays through the composed bounce walk,
   meshes included where the program has them), `surface_texel`, `display`, `vp`.
2. Add the source to `shading::gpu_wgsl` (`shading/mod.rs`) and the registry entry. Its
   caps decide the rest: `Cap::Paths` gives it the `bounce` input, the per-pixel state
   (`sp`, `accum`: `shading::state`) and the knobs `spp` and `accumulate`; the knobs the
   state owns go to components by cap (`bounces` to path tracers, `reach` to
   `Cap::Occlusion`). The state reads the shading node's knobs itself, so no region changes.
3. A new lighting model needs its reference: a variant of `plan::Lighting` (derived in
   `RendererSpec::lighting` from the caps), the reference's model in `mr-reference`
   (`light_of`), the option the host passes it (`main.rs` `reference`). A shading computing
   an existing model needs none of this.
4. Knob defaults: one line in `crates/mr-components/src/knobs.rs` (a test keeps it equal to
   what the regions declare).
5. Tests: a reference unit test of the model; `lighting_class` (`src/plan/tests.rs`); a
   contract probe if it declares a new combination of caps (`src/contract.rs`); the GPU
   shader-composition tests iterate `Graph::choices("shading")`, so they compose it on every
   region (`scripts/check.py gpu`).
6. Pixel check: `scripts/run1.py` with it on `raycast-gpu`, `raster-gpu`, `rayquery-gpu`,
   `splat-gpu`, `columns-gpu` (lit frames scored against the reference's model, converged).
7. Docs: the shading row in `docs/pipelines.md`; `docs/plans.md` if it adds a reference
   class.

Measured (`ao`, 2026-11-26): files touched outside the component: registry, `shading/mod.rs`,
`caps.rs` (a new cap), `shading/state.rs`, `knobs.rs`, `plan/like.rs`, `plan/renderer.rs`,
`main.rs`, `mr-reference`, two docs; hand-kept list lines: 3 (`gpu_wgsl`, knob defaults,
the reference option match). About 45 minutes, most of it the friction fixed below. Pixels
on terrain, lit, settled (64 spp) against the reference's occlusion: `raycast-gpu`,
`rayquery-gpu` 52.1 dB, `splat-gpu` 51.8, `raster-gpu` 48.1 (`pathtrace` on `raycast-gpu`:
48.9); `columns-gpu` 17.5, as its `direct` (18.3) and `pathtrace` (18.1): the columns' own
approximation. The architecture test `no_wgsl_body_is_copied_under_another_name` caught the
sampling functions copied from `pathtrace`; they are shared (`shading/sampling.wgsl`).

Both together grew the full sweep (`scripts/full-sweep.toml`, every `*`) from 1883 to 2794
passes (+48 %): one more choice multiplies every graph varying that node with others.

## A GPU walk (rehearsed with `topdown`)

1. Write `crates/mr-components/src/traversal/NAME/gpu.wgsl`: `ctrace(o, d, max_t) -> Hit`
   (camera-relative origin; the first solid cell past the start cell, its entry `t` and entry
   `axis`, 3 when the ray starts inside the bounds), written only against the storage
   interface: `s_top(key)` (`EMPTY`, `UNIFORM_BIT | block`, or the chunk's handle),
   `s_probe`, `s_occupied` at the levels in `LEVEL_MASK`, `s_block`, and what
   `storage/adapters.wgsl` derives (`s_dist`, `s_row`). Count iterations in `ct_iters` (the
   `steps` view), stop after `MAX_STEPS`, clip to `vp.bounds_min/max` and `max_t`. Skipping
   the start cell is what lets the layer walk resume through the kernel (`Cap::Layers`).
2. Add the source to `traversal::gpu_wgsl` (`traversal/mod.rs`) and the registry entry: its
   caps (`Steps`, `Layers`; `Lod` only if it walks the LOD levels) and `needs` on the
   storage's caps (`Voxels`; `Dist`, `Row` for a walk relying on those).
3. What is derived: the walk joins every GPU site that reads a kernel (primary rays, shadow
   and bounce inputs, discovery's sampler), the sweeps' `*`, the GUI's choices, the catalogue.
   Test: `cargo test -p mr-components --lib secondary_walks` composes it as primary and as
   secondary walk over every GPU voxel storage (from the registry); `scripts/check.py`; if
   only the sweep sizes fail, `scripts/new.py accept-sweep`.
4. Pixel check: `scripts/run1.py` with `{node="kernel",component="NAME"}` and each GPU storage
   on `raycast-gpu`, on terrain and the stress worlds (lattice, mandelbulb), unlit and lit
   (the lit pass sends its shadow rays through the same kernel), against the reference;
   compare with `refloor` in the same run. There is no GPU walk agreement test: the pixels
   are it.
5. Docs: the traversal row of `docs/pipelines.md` names the kernels.
6. Timing (background): `.agent/bin/idle add -- env OUT=target/sweep/NAME scripts/sweep.py
   terrain` with `vary` on the kernel and storage.

Measured (`topdown`, 2026-11-26): files touched outside the component: the registry,
`traversal/mod.rs` (one hand-kept line), `docs/pipelines.md`; two tests that listed kernels
by hand made to derive or not to care. About 30 minutes, most of it the walk itself. Pixels
equal to `refloor` in the same run on every storage (terrain 58.6 dB unlit, 55.9 lit; lattice
52.4 / 52.3; mandelbulb 59.6 / 57.8; at most 0.01 % bad); first try. The full sweep grew
2794 -> 3433 passes.

A CPU walk is not rehearsed: it is a `Kernel` type with its trace function, and today it also
needs a `Traversal` variant and its dispatch in `mr-exec-cpu/src/raycast.rs` (`traversal`,
`trace_one`, the primary loop, the counters) and the hand list of CPU kernels in the
registry's site check (`lib.rs`, `["refloor", "incremental"]`); `traversal/cpu_tests.rs`'s
agreement test compares CPU kernels with the reference walk.

## A new shape (a drawer with its own region)

Not rehearsed yet. Expect: a region module in an executor with its `REGION: &Builds`, its
nodes, and `graph::REGIONS`; the contract's visibility probe states what a region is.

## A world generator

Rehearsed with `mandelbulb` (~20 minutes; 2 code files, 3 docs, 1 script line).

1. In `crates/mr-world/src/generators.rs`: a `GenSpec` variant (its fields are the knobs;
   sizes and counts take `deserialize_with = "within_world"`, so past 2^30 is refused by name),
   its entry in `GENERATORS` (name, description, default, each field's help and slider range),
   its arm in `GenSpec::build` (clamp what would make a block cost unbounded work or overflow),
   and the generator: `WorldGen` (`block`, `chunk_y_range`, a `spawn` in air; a `chunk` fast
   path returning `ChunkData::Uniform(AIR)` outside its bounds; `asks` if it places see-through
   blocks).
2. What is derived from the `GENERATORS` entry: the GUI's world choices and fields, plan
   parsing, and the generator tests (`every_generator_is_listed`, `generators_chunks_match_block`:
   chunks agree with `block` around spawn and origin, spawn in air, something solid;
   `generators_ask_for_the_blocks_they_place`; `generator_sizes_hold_out_to_the_worlds_reach`:
   every integer field at +-2^30 and 0, built and sampled with overflow checks). Add a test of
   the generator's own shape.
3. A plan in `plans/NAME.toml` (copy one of the stress worlds' plans, aim the camera) and the
   name in `SCENARIOS` in `scripts/mr.py`, which sweep.py, turn-sweep.py and suite.py run. Adding
   it re-deals the turn sweep's pairs among the plans (a plan's part is its place in the list).
4. Check: `cargo test -p mr-world --lib generators`, then the plan's pipelines against the
   reference: `scripts/run1.py '[...the plan's renderers...]' NAME -s run.frames=60 -s
   output.compare.every=30`.
5. Docs: the generator table in `docs/plans.md` (and the comment listing generators there),
   the README's plan list.

## Friction found

Fixed while rehearsing:

- The stateful shading knobs (`bounces`, `spp`, `accumulate`) were copied into seven region
  knob structs and passed through every region's core options; a new state knob meant
  editing all of them. The shading state now reads its node's knobs itself.
- The state's knobs were owned by every stateful shading alike; now by cap (`bounces` by
  path tracers, `reach` by `Cap::Occlusion`).
- The GPU composition tests bound a `bounce` input only for a shading named `pathtrace`;
  now for every stateful shading.
- The generator test that compared chunks with `block` listed the stress worlds by hand (a new
  generator went untested unless added); it now takes every `GENERATORS` entry.
- The scenario plans were three hand-kept lists (sweep.py, turn-sweep.py, suite.py, which had
  drifted apart); now one, `SCENARIOS` in `scripts/mr.py`.
- The GPU walk composition test listed storages and kernels by hand (a new kernel or GPU
  storage went uncomposed); both now come from the registry. A graph test spelled
  out every kernel in an expected error message; it now checks only what it is about.

Open:

- A new lighting model touches the host in three places (`Lighting`, `lighting()`, the
  reference option in `main.rs`) and the reference: the reference's models could be a
  registry of their own keyed by cap.
- `new.py` scaffolds CPU storages only; shadings, walks and GPU storages are written by hand.
- `shading::gpu_wgsl` and `traversal::gpu_wgsl` are hand-kept matches of names to sources.
- A CPU walk needs a `Traversal` variant and its dispatch in `raycast.rs` in four places, and
  the registry's site check lists CPU kernels by hand.
- `scripts/new.py accept-sweep` builds and runs the release test suite to read the new size;
  it takes minutes (the size could be printed by `multirenderer` from the plan instead).
- The full sweep grows by the product of the choices it varies together; with S adding many
  components it needs pairwise coverage as the turn sweep has, or it stops being runnable.
- `glass` has a plan but is in no sweep's scenarios (it was in none of the three old lists);
  whether transparency worlds join the sweeps is open.
