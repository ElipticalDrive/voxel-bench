# Run plans

A plan is a TOML file describing a scene, the renderers and how to run them. `plans/` has
complete examples (benchmark workloads; `demo-*.toml` are set-ups for the GUI's 🎬 Demos). `multirenderer run <plan>` runs one; the GUI edits and runs them live.

```toml
name = "my-test"

[scene]
fov_deg = 70
generator = { type = "terrain", seed = 7 }   # terrain | flat | menger | room | random | showcase | empty
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
isolation = "process"   # or "thread"
launcher = []           # a prefix for worker commands, e.g. ["nice", "-n", "5"]
repeats = 1             # >1: passes repeat round-robin; medians and spread are reported
devices = { gpus = ["nvidia"] }   # GPU adapter per slot by name; cpu_threads
compile = "off"         # or "native", "break-in" (see pipelines.md)
stage_times = true      # time every stage (GPU: +0.1–0.6%)
clock_fps = 60          # the simulated clock of scripted runs

[[renderer]]
compose = { backend = "gpu-raster", geometry = "greedy", draw = "raster" }  # or graph, or name = "cpu-reference"
label = "raster-sunshadow"    # optional; lets one pipeline run with several option sets
options = { shadows = "sun" } # the components' knobs
sites = { shadow = "distance/sphere", client = "pick(radius=8, max_lod=2)" }

[output]
window = false
readback = "auto"             # auto | always | never
save = { every = 100, diffs = true }
compare = { reference = "cpu-reference", every = 100 }
dir = "runs"
```

The three ways of writing a renderer (`compose`, `graph`, the command line's `-r`) are in
[pipelines.md](pipelines.md).

Any key can be overridden from the command line with `-s dotted.key=value`, and removed with
`-u dotted.key`:

```sh
multirenderer run plans/terrain.toml -s run.resolution=[3840,2160] \
  -s 'renderer=[{compose={backend="gpu-compute",storage="prefix"}}]' -u output.save
```

## Run output

Each run writes `runs/<timestamp>-<name>/`: `plan.toml` (with the overrides applied),
`report.json` (every metric, including per-frame records), `summary.txt`, `observations.jsonl`,
`logs/` (one per worker) and `frames/` (saved frames and diff images).

## Sweeps

A `compose` field may be `"*"` (every component the back end has for that slot; for
`backend`, every back end) or a list. The renderer then expands into one renderer per valid
combination:

```toml
[[renderer]]
compose = { backend = "*", storage = "*", geometry = "*", visibility = "*", draw = "*", traversal = "*", shading = "*" }
```

- Slots left out keep their default. `"*"` on a slot that a back end lacks is ignored. A
  *named* component on such a slot skips that back end: `backend = "*", storage = "palette"`
  runs only the back ends that have a `palette` storage.
- `sites` values may be swept the same way (`link`, `client`, `generator`, ...).
- The entry's `options` go to every combination whose components declare them.
- In each pass, combinations that the pass mode cannot tell apart run only once: shading in
  unlit passes, the meshes stage without meshes, and a rasteriser's storage (which only its
  shadow rays walk) in unlit passes.

## Observations and the catalogue

`observations.jsonl` has one record per renderer and pass:
- the machine (CPU, threads, memory, GPU, driver);
- the scenario: the plan's name and scene, the resolution, the view radius, the frames, the
  streaming and the mode;
- the scenario's **measured features**, independent of any renderer and measured once per run
  in well under a second (`src/features.rs`): camera speed and turn rate, chunks at start and
  loads per frame, the fractions of non-empty chunks, solid cells and surface in range, sky
  fraction and hit distances from a grid of rays, edits per frame, lights and mesh triangles;
- the pipeline (graph, its key) and its knobs;
- the results: fps, frame interval (mean, p50, p99), GPU and CPU ms, latency, init and load
  time, memory, PSNR (mean and minimum), bad pixels, stage times, stream counters and the
  renderer's own counters.

`multirenderer catalogue <dirs…> [--out DIR] [--min-psnr 35]` reads any number of runs and
writes `catalogue.md` and `catalogue.csv` (the CSV has the scene features as columns). Per
scenario (plan × mode) it ranks the renderers by fps. Renderers below the PSNR floor count as
incorrect: they are ranked, but never best. For each renderer it lists:
- the geometric mean of its speed relative to each scenario's best;
- its wins, and where it is within 10% of the best (*good at*);
- where it is comparatively strong (*stands out*: at least 1.25× its own average relative
  speed);
- where it is weak (below 0.25 of the best), incorrect, or failed.

For each back end and slot, it also shows the paired effect of each component: its fps
against the best of otherwise identical pipelines.

## Scripts

Python, for Linux and Windows (`scripts/`):

- `sweep.py [plans…]`: the full sweep over every scenario plan (one night's work at each
  plan's own length; `FRAMES=120` for shorter, faster camera paths), into
  `target/sweep/<time>/`, then the catalogue there.
- `check.py [build|test|gpu|windows|wine|all]`: quiet verification (builds without warnings,
  tests, the GPU end-to-end tests, Windows cross-checks); prints only problems and totals, full
  logs in `target/logs/`.
- `run1.py RENDERER_TOML [plan] [-s …]`: one quiet run of one renderer (default plan terrain)
  into `target/run1/`, then its summary; `report.py [dir|report.json]…` prints that summary
  (one line per pass, then its stages) for any run, by default the newest.
- `schemes.py`: checks the encyclopedia's entries and rewrites its index
  (`docs/schemes/README.md`).
- `suite.py`, then `summarise.py target/catalogue`: the whole catalogue of default pipelines.
- `ab/`: A/B checks between two builds. `save-before.py` keeps the current release build as
  "before"; `perf.py` interleaves runs of both and prints median GPU ms and fps per pass
  (`perfsum.py` summarises them); `pixels.py` renders with both and compares the saved frames
  pixel by pixel (`cmp.py` compares two run directories' frames); `wgsl.py` dumps and
  compares every shader both builds assemble; `sweepcmp.py` compares two sweeps' records,
  matched by pipeline key.
