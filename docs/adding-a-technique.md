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

## A walk or traversal

Not rehearsed yet. Expect: a CPU `Kernel` and/or a GPU kernel module, the registry entry
with `needs` on the storage's caps, the walk agreement test.

## A new shape (a drawer with its own region)

Not rehearsed yet. Expect: a region module in an executor with its `REGION: &Builds`, its
nodes, and `graph::REGIONS`; the contract's visibility probe states what a region is.

## A world generator

Not rehearsed yet. Expect: one entry in `mr_world::generators::GENERATORS`.

## Friction found

Fixed while rehearsing:

- The stateful shading knobs (`bounces`, `spp`, `accumulate`) were copied into seven region
  knob structs and passed through every region's core options; a new state knob meant
  editing all of them. The shading state now reads its node's knobs itself.
- The state's knobs were owned by every stateful shading alike; now by cap (`bounces` by
  path tracers, `reach` by `Cap::Occlusion`).
- The GPU composition tests bound a `bounce` input only for a shading named `pathtrace`;
  now for every stateful shading.

Open:

- A new lighting model touches the host in three places (`Lighting`, `lighting()`, the
  reference option in `main.rs`) and the reference: the reference's models could be a
  registry of their own keyed by cap.
- `new.py` scaffolds CPU storages only; shadings, walks and GPU storages are written by hand.
- `shading::gpu_wgsl` is a hand-kept match of names to sources.
- `scripts/new.py accept-sweep` builds and runs the release test suite to read the new size;
  it takes minutes (the size could be printed by `multirenderer` from the plan instead).
- The full sweep grows by the product of the choices it varies together; with S adding many
  components it needs pairwise coverage as the turn sweep has, or it stops being runnable.
