# Run plans

A plan is a TOML file describing a scene, the renderers and how to run them. `plans/` has
complete examples (benchmark workloads; `demo-*.toml` are set-ups for the GUI's 🎬 Demos). `multirenderer run <plan>` runs one; the GUI edits and runs them live.

```toml
name = "my-test"

[scene]
fov_deg = 70
generator = { type = "terrain", seed = 7 }   # terrain | flat | menger | room | random | showcase | empty
                                             # stress worlds: city | islands | lattice | gyroid | checker | glass
camera = { type = "flythrough", points = [[0,110,0],[260,140,120]] }
                                             # static | orbit | line | flythrough | spin
environment = { no_sun = true, ambient = [0.02,0.02,0.03], sky = [0,0,0], exposure = 1.0 }
lights = [{ type = "scatter", count = 400, extent = [600,80,300], on_surface = true }]
events = [{ type = "random_edits", count = 2000, radius = 40, block = "toggle" }]
                                             # sphere | box | random_edits | animate_lights | sun_cycle
# meshes: animated triangle meshes, see plans/meshes.toml

[run]
frames = 600            # measured frames; the camera path spans these
warmup = 30             # unmeasured frames at the first camera
resolution = [1920, 1080]
modes = ["unlit", "lit"]  # also "unlit+meshes", "lit+meshes"
view_radius = 16        # default reach of every client node (its `radius` knob)
max_lod = 0             # default `max_lod` of every client node (each level covers the radius at twice the cell size)
in_flight = 2           # frames a renderer may have outstanding
pacing = "max"          # or { fps = 60 }
streaming = "complete"  # or { budget = 400 } chunk loads per frame
parallel = false        # true: all renderers at once (side by side in the window)
                        # a renderer that fails there stops alone; the others' reports
                        # name it (`beside_failed`): they shared the machine with it part-way
isolation = "process"   # or "thread"
launcher = []           # a prefix for worker commands, e.g. ["nice", "-n", "5"]
repeats = 1             # >1: passes repeat round-robin; medians and spread are reported
devices = { gpus = ["nvidia"] }   # GPU adapter per slot by name; cpu_threads
compile = "off"         # or "native", "break-in" (see pipelines.md)
stage_times = true      # time every stage (GPU: +0.1–0.6%)
clock_fps = 60          # the simulated clock of scripted runs

[[renderer]]
graph = "raster-gpu"          # a graph file (shipped: by name) or the graph itself; or name = "cpu-reference"
edits = [{ node = "geometry", component = "naive" }]   # changes to it, in order; `vary`: sweeps (below)
label = "raster-sunshadow"    # optional; lets one pipeline run with several option sets
options = { shadows = "sun" } # the components' knobs

[output]
window = false
readback = "auto"             # auto | always | never
save = { every = 100, diffs = true }
compare = { reference = "cpu-reference", every = 100, settle = 2 }
dir = "runs"
```

The ways of writing a renderer (`graph` as a file or inline with `edits`, the command line's `-r`) are in
[pipelines.md](pipelines.md).

Two scores come out of `compare`. Every renderer is scored against the reference rendering
everything at full resolution (the run's `view_radius`, LOD 0): `quality`, how far its frames
are from the truth. A pipeline whose client holds something else (another `radius`, LOD
levels) is also scored against the reference given the same (`quality_like`, the region in
`like`; the reference's own pass is labelled `cpu-reference as pick(radius=3, max_lod=2)
(ref)`): how far its frames are from what they should show. A lossy pipeline scores low on the
first and high on the second; a wrong one low on both.

The same holds for the lighting model. A pipeline whose shading path traces its lit frames
(`pathtrace` with at least one bounce) is also scored against the reference path tracing with
the same bounces, converged (256 paths per pixel; `compare.reference_options = { paths = N }`
changes it), labelled `cpu-reference as pick(radius=16), pathtrace(bounces=1) (ref)`. What is
left in that score is the pipeline's noise (and any error): it rises about 3 dB with every
doubling of `spp` when the pipeline computes the same model. Such a reference takes seconds
per compared frame.

A pipeline whose graph has an edge that arrives a frame late (`prev`: discover-cpu's faces,
discovered while the previous frame is drawn) shows a moving camera partly as it was a frame
ago, and scores low for it. It is scored a third time, settled (`quality_settled`; the late
edges in `late`): in a pass of its own over the same frames, each compared frame is held
still for `compare.settle` more renders (2: one for the late edge, one for a prediction made
from the previous camera) and the last is scored, against the like reference where it has
one. A late pipeline scores low as shown and high settled; a wrong one low in both.
`settle = 0` leaves it out.

A pipeline reports what it supports for its graph as one typed set of features
(`mr_core::Features`: `lit`, `meshes`, `mesh rays`, `lod` with its levels), each missing one with
the reason; its executor declares the region's, and the graph narrows them by what its nodes and
components lack (no meshes node; a drawer without mesh rays). It is asked for nothing beyond
that. What a plan asks of one that lacks it is left out and named in its report
(`not_supported`, e.g. `max_lod 2: storage `tree64` lacks lod`, or `mesh rays: draw
`raster` (`draw`) lacks mesh rays: ...` for forward raster-gpu's lit `+meshes` passes, whose
meshes take voxel shadows only): it ran without, and its region and scores are those of what it
ran. Passes of a mode it lacks entirely (`+meshes` without meshes) are skipped.
Knobs too: a knob in `options` its pipeline does not have, one whose owning node the graph lacks
(`bounces` without a `pathtrace` shading), or a value whose need no node meets (`view = steps`
needs a primary walk that counts its steps; discover-cpu's `clusters` / `kinds` views a
`cluster` visibility) is left out and named the same way (`knob `bounces`: no shading node of
`pathtrace` (its shading: `direct`)`). Executors declare a value's needs on its knob
(`ParamSpec::needs`).

## Worlds

Every generator is deterministic and has a plan of its name in `plans/`. Each stresses
something different:

| generator | parameters | what it stresses |
|---|---|---|
| `terrain` | seed, caves, trees | the general case: streaming, distance, hidden caves |
| `flat` | checker | almost nothing (a floor) |
| `menger` | level | deep occlusion, many small faces, rays through holes |
| `room` | size | indoor lighting, everything near |
| `random` | size, density, seed | incoherent rays, no empty space, no regularity |
| `city` | seed, block, max_height | long rays down streets; hollow towers behind window openings (overdraw, many floors per ray) |
| `islands` | seed, spacing, height, fill | mostly empty 3D space: many chunks streaming in, small objects far away |
| `lattice` | period, thickness, height | thin beams everywhere: long rays past many near misses (ray casters 4× slower, rasterisers 10× faster than them), thin features at LOD |
| `gyroid` | period, thickness, height | curved cave walls: staircases greedy meshing cannot merge (CPU rasterising), short rays |
| `checker` | size, cell | every block shows six faces (worst for meshing/raster), yet perfectly regular (best for a DAG) |
| `glass` | size, panes | see-through blocks: parallel glass panes (a ray through all crosses `2 * panes` surfaces), a glass cube and a grate (cutout) cube around opaque blocks, partial shadows; pipelines without transparency draw them opaque |
| `showcase` | | one of every block, for textures |
| `empty` | | sky only |

Any key can be overridden from the command line with `-s dotted.key=value`, and removed with
`-u dotted.key`:

```sh
multirenderer run plans/terrain.toml -s run.resolution=[3840,2160] \
  -s 'renderer=[{graph="raycast-gpu",edits=[{node="storage",component="prefix"}]}]' -u output.save
```

## Run output

Each run writes `runs/<timestamp>-<name>/`: `plan.toml` (with the overrides applied),
`report.json` (every metric, including per-frame records), `summary.txt`, `observations.jsonl`,
`logs/` (one per worker) and `frames/` (saved frames and diff images).

## Sweeps

A renderer written as a graph may `vary` its edits. It then expands into one renderer per
combination that can be made and is a pipeline, each distinct pipeline once:

```toml
[[renderer]]
graph = "raycast-gpu"
edits = [{ node = "shading", component = "pathtrace" }]   # the same in every combination
vary = [
    { node = "storage", component = "*" },                  # everything the graph takes there
    { node = "kernel", component = ["refloor", "sphere"] }, # a list of values
    { any = [[], [{ remove = "meshes" }]] },                # a choice between lists of edits
]
```

- An entry of `vary` is an edit ([pipelines.md](pipelines.md#writing-a-pipeline)) whose value
  is a list: `component`, `from`, `when`, `device`, `api`, or a knob's `value`. `"*"` stands
  for everything the graph offers there: the components of the node's role, the APIs, `now`
  and `prev`, the other nodes (for `from`). Devices and knob values have no such list; name
  them.
- `{ any = [[...], [...]] }` chooses between lists of edits, for what one edit cannot say (a
  node added and bound, a node removed, several knobs together). An empty list changes
  nothing. The edits inside may hold lists themselves.
- The combinations are every choice of one value per entry, the first entry varying slowest,
  each made after the entry's `edits`. A combination that cannot be made (an edit names what
  the graph lacks) or that is no pipeline does not exist; a sweep that leaves none is an
  error, with the first reason. At most 100 000 combinations per entry.
- Nothing is asked of a table: what `"*"` keeps is what the pipeline check accepts for that
  graph.
- The entry's `options` go to every pipeline whose executor declares them; its `label` is
  dropped (each pipeline has its own).
- In each pass, combinations that the pass mode cannot tell apart run only once: shading in
  unlit passes, the meshes stage without meshes, and a rasteriser's storage (which only its
  shadow rays walk) in unlit passes.

The full sweep the scripts run (`sweep.py`, `ab/wgsl.py`) is `scripts/full-sweep.toml`: every
shipped graph with each node given every component it takes and its optional nodes present
and absent. `plans/stream.toml` sweeps the link, the generator's place and the client.

A component is never an option: `options` holds knobs only. In each pass, of the swept
pipelines the pass cannot tell apart the first runs (unlit: the shading's component and the
rays it casts make no difference; without meshes: the meshes node and its shadow rays).

## Observations and the catalogue

`observations.jsonl` has one record per renderer and pass:
- the machine (CPU, threads, memory, GPU, driver);
- the scenario: the plan's name and scene, the resolution, the view radius, the frames, the
  streaming and the mode;
- the scenario's **measured features**, independent of any renderer and measured once per run
  in well under a second (`src/features.rs`): camera speed and turn rate, chunks at start and
  loads per frame, the fractions of non-empty chunks, solid cells and surface in range, sky
  fraction and hit distances from a grid of rays, edits per frame, lights and mesh triangles;
- what rendered it: the label, the executor (`renderer`: `cpu`, `gpu`; a standalone
  renderer's name), the pipeline (`graph`, always, and `graph_key`) and its knobs (`options`);
  records of pipelines from before version 6 (a flat `composition`) are no longer read: `catalogue`
  leaves them out and says how many;
- the results: fps, frame interval (mean, p50, p99), GPU and CPU ms, latency, init and load
  time, memory, PSNR (mean and minimum), bad pixels, the same against the like reference
  (`like`, `psnr_like`, `bad_pct_like`) and settled (`late`, `psnr_settled`,
  `bad_pct_settled`), stage times, stream counters and the
  renderer's own counters.

`multirenderer catalogue <dirs…> [--out DIR] [--min-psnr 35]` reads any number of runs and
writes `catalogue.md` and `catalogue.csv` (the CSV has the scene features as columns, and a
pipeline as its label, executor and graph key). Per
scenario (plan × mode) it ranks the renderers by fps. Renderers below the PSNR floor count as
incorrect: they are ranked, but never best (the floor applies to `psnr_like` where a renderer
has one: fewer levels of detail are a loss, not an error; a path tracer is held to its own
lighting model, and its noise counts; and to `psnr_settled` where it has one: a frame late
is a property, not an error). For each renderer it lists:
- the geometric mean of its speed relative to each scenario's best;
- its wins, and where it is within 10% of the best (*good at*);
- where it is comparatively strong (*stands out*: at least 1.25× its own average relative
  speed);
- where it is weak (below 0.25 of the best), incorrect, or failed.

For each node it also shows the paired effect of each component: its fps against the best
among the pipelines whose graphs differ in that node's component alone (knobs equal). A
node is named by its executor, its role and the inputs leading to it from the output
(`gpu · storage (image > hits > primary > storage)`: the storage the primary rays walk;
`image > shadow > storage`: the one only shadow rays walk).

## Scripts

Python, for Linux and Windows (`scripts/`):

- `sweep.py [plans…]`: the full sweep over every scenario plan (one night's work at each
  plan's own length; `FRAMES=120` for shorter, faster camera paths), into
  `target/sweep/<time>/`, then the catalogue there.
- `check.py [only] [build|clippy|test|gpu|windows|wine|all]`: quiet verification (builds and clippy
  without warnings, tests, the GPU end-to-end tests, Windows cross-checks; `gpu`, `windows`, `wine`
  add to the default build clippy test, `only` runs just what is named); prints only problems and
  totals, full logs in `target/logs/`.
- `run1.py RENDERER_TOML [plan] [-s …]`: one quiet run of one renderer (default plan terrain)
  into `target/run1/`, then its summary; `report.py [dir|report.json]…` prints that summary
  (one line per pass, then its stages) for any run, by default the newest.
- `suite.py`, then `summarise.py target/catalogue`: the whole catalogue of default pipelines.
- `ab/`: A/B checks between two builds. `save-before.py` keeps the current release build as
  "before"; `perf.py` interleaves runs of both and prints median GPU ms and fps per pass
  (`perfsum.py` summarises them); `pixels.py` renders with both and compares the saved frames
  pixel by pixel (`cmp.py` compares two run directories' frames); `wgsl.py` dumps and
  compares every shader both builds assemble; `sweepcmp.py` compares two sweeps' records,
  matched by pipeline key.
