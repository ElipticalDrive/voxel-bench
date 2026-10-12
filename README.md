# MultiRenderer

A test bench for voxel renderers. Many renderer implementations (CPU, GPU compute, raster,
hybrid, anything) render the same scenes, each sandboxed in its own worker process, and are
benchmarked and scored against reference images.

The world is the simplest possible volume: an axis-aligned unit-cube lattice. Every cell is
either air (fully transparent) or an opaque block with a 16x16 texture per face. Point
lights occupy no volume and are invisible. Worlds are unbounded and streamed in 32³ chunks.

## Quick start

```sh
cargo build --release
./target/release/multirenderer                              # GUI (same as `gui [plan.toml]`)
./target/release/multirenderer list                         # available renderers
./target/release/multirenderer run plans/terrain.toml       # benchmark + compare + report
./target/release/multirenderer run plans/showcase.toml -s output.window=true -s run.frames=600
./target/release/multirenderer view plans/terrain.toml -r gpu-compute:bricks+refloor -r gpu-raster:bricks+greedy+raster   # fly around
./target/release/multirenderer diff a.png b.png --out diff.png
```

### GUI

`multirenderer` with no arguments (or `multirenderer gui plans/terrain.toml`) opens the
GUI. It has five areas:

- **Workflow (left).** A form for every plan setting: world generator, camera path,
  environment, lights, scripted events, renderers (each as a stage grid, see below), run
  settings and benchmark outputs. A TOML tab lets you edit the raw plan. Open, save and Ctrl+S are in
  the toolbar. "⌖ cam" buttons copy the live camera into positions and camera-path
  points, which is a quick way to author fly-throughs.
- **Viewport (centre).** Every renderer in the plan runs live, tiled side by side. Each
  renders the latest camera as fast as it can (or up to the fps cap), independently of the
  others. Frames are read back only at the display rate, so the numbers reflect the
  renderer rather than the GUI.
- **Metrics (right).** Live fps, frame interval, latency and CPU/GPU/update time (mean
  and p99), renderer counters, process RSS, GPU memory and streaming progress, with
  frame-time and GPU/CPU plots. Each renderer has ↻ to restart it with the options from
  the editor, and ✖ to remove it.
- **Compare (right).** Renders the exact current view on every renderer and the reference
  renderer, then shows PSNR, MAE and bad-pixel % plus images and diff images.
- **Benchmark (right).** Runs the full plan as a separate `multirenderer run` process,
  streams its output, and shows the report table and an fps chart. It can also load
  previous reports from `runs/`.

Keys: **F5** starts or restarts the live session. **WASD**, **Space** and **Shift** fly
(**Ctrl** for 5× speed). Right-drag, left-drag or the arrow keys look, and the mouse
wheel changes speed. **L** toggles lit/unlit, **N** toggles meshes. **F** switches between following the plan's
camera path and free flight. **P** plays or pauses the scene clock, **[ ]** scrubs it, and
**R** resets it. **E** enters edit mode: click to remove a block, Ctrl+click to place one.
**G** drops a light at the camera. **C** runs a comparison. **1–9**, **Tab** and **Esc**
focus a renderer, cycle through them or show them all. **F11** switches to a fullscreen viewport (Esc
leaves). **Z** draws exactly one frame pixel per screen pixel, snapped to the physical
pixel grid and cropped rather than scaled. **M** sets the render resolution to the
tile's exact pixel size and restarts the session, so F11, 1 and M together give native
display resolution. **O** hides the overlays. The ❓ Keys tab lists everything.

### Assembling renderers from components

A renderer is assembled from components rather than picked from a list (IDEAS §12):
**back end** × **storage** × **geometry** × **prepass** × **visibility** × **traversal** ×
**shading** × **meshes**. The pieces live in `components/`, one directory each
(`storage/prefix/gpu.wgsl`, ...); back ends in `backends/`. Not every back end has every
slot: `geometry` exists only on the rasterising and RT back ends, `prepass` only on
gpu-compute, `meshes` only on gpu-compute and gpu-raster.

**Visibility and traversal** (IDEAS §18): the *visibility strategy* organises primary
visibility (`raycast`: a ray per pixel; `raster` / `visbuffer` / `gbuffer`; discovery
`quadtree` / `cluster`; `splat`; `columns`). The *traversal kernel* answers ray queries
against the storage (`refloor`, `incremental`, `sphere`, `rowrun` on the storages,
`rayquery` on gpu-rt's BLAS): the primary rays of ray casting, discovery's rays, and every
shadow and bounce ray. So a rasteriser's shadow rays can walk with `sphere` like
gpu-compute's primary rays. Kernels live in `components/src/traversal/`. Labels leave out
a stage that is implied at its default (`raycast` on gpu-compute, the shadow-ray kernel
of a rasteriser): `gpu-compute: prefix + incremental`, `gpu-raster: bricks + greedy +
raster`, and `gpu-raster: distance + greedy + raster + sphere` once changed. Plans and
records from before the split (`visibility = "refloor"`) load with the kernel moved to
`traversal`.

Lighting and meshes are stages *around* the walk, so storages and walks know nothing of
them. The pass mode (lit / unlit, meshes on / off) is what the scene asks for. The shading
stage renders lit and unlit and owns the lighting knobs (`shadows`). The meshes stage
decides how meshes are drawn (`composite` on gpu-compute, `depth` on gpu-raster), or that
they are not (`none`).

**The stage grid** (plan editor and live Metrics tab) edits a renderer in one compact
block: the back end with its own knobs, then one row per stage with the chosen component
and *that component's* knobs next to it (e.g. `exact` / `skip2…16` beside traversal
`incremental`, `bounces` / `spp` beside shading `pathtrace`, `cull` beside geometry
`naive`). Every component of a stage is listed; ones that cannot run with this back end
are greyed out, and hovering them says what they conflict with. In the live session a
component switch applies in place where the back end can (or hot-swaps), and a back-end
switch hot-swaps the renderer. Back ends declare each knob's owner with
`ParamSpec::of(slot, &[components])`.

```toml
[[renderer]]
compose = { backend = "gpu-compute", storage = "prefix", traversal = "incremental", shading = "direct" }
```

```toml
[[renderer]]
compose = { backend = "gpu-raster", storage = "bricks", geometry = "cubes", visibility = "visbuffer" }
```

- *Command line:* the back end's required slots in order, then optionally shading,
  prepass, meshes and the slots labels leave out (visibility on gpu-compute, traversal on
  rasterisers): `-r gpu-compute:prefix+incremental`,
  `-r gpu-raster:distance+greedy+raster+direct+depth+sphere`,
  `-r gpu-raster:bricks+greedy+visbuffer`, `-r gpu-compute:bricks+refloor+direct+beam`,
  `-r gpu-compute:bricks+refloor+direct+none+none` (no prepass, no meshes).
  `multirenderer list` shows back ends (with their slots and defaults) and components.
  Slots left out of a `compose` table take the back end's default.
- *GUI:* each renderer has a back-end picker and one picker per slot. Combinations that
  cannot run are disabled, with the reason on hover.
- *Capabilities and adapters* (`components/src/caps.rs`): an interface has a required core
  and optional capabilities; components declare what they provide and what they need from
  other stages. A need the chosen component lacks is met by an adapter (compile-time code,
  e.g. `sphere` without `distance` gets `s_dist` from `s_probe`): correct, but slower, and
  noted in the GUI. A need with no adapter makes the combination invalid (greyed, with the
  reason), e.g. LOD on gpu-compute needs a pool storage, `refloor` and no beam prepass.
- *Today:* three back ends.
  - `gpu-compute`: storages bricks, hier, prefix, distance, rows, tree64, svo_dag, clipmap;
    visibility raycast; traversal refloor, incremental, sphere, rowrun;
  - `cpu`: storages bricks, hier, prefix, u8, bits, palette; visibility raycast; traversal
    refloor, incremental;
  - `gpu-raster`: geometry greedy, naive, cubes; visibility raster, visbuffer, gbuffer; storage
    and traversal (what lit-mode shadow rays walk) as on gpu-compute;
  - all: shading `direct`; gpu-compute, gpu-rt, gpu-raster, gpu-discover and gpu-splat
    also `pathtrace` (was gpu-pathtrace: diffuse path tracing with NEE for sun and point
    lights, `bounces` cosine-weighted bounces through the composed walk, `spp`,
    progressive accumulation while the view is still; `bounces = 0` reproduces
    `direct`; 6–7% faster than the former renderer). Every back end lights through its
    kind's one shading implementation: `shade_surface` on the GPU (a `Surface`: point,
    normal, block-local point, block, face), `direct::cpu::shade` on the CPU, except
    cpu-raster, which keeps the reference's f64 model on purpose (higher PSNR).
    Fragment-shaded `pathtrace` (gpu-raster `raster`, gpu-discover) needs the lit depth
    prepass (and depth order) so one fragment per pixel updates the accumulation;
  - gpu-compute also: prepass `none` or `beam` (was gpu-beam: one conservative cone per
    tile marches top cells, rays start where it is provably empty; exact, knob `tile`;
    matches the former renderer, and is still a net loss: chunk-level skipping already
    makes empty space cheap).

  - `gpu-rt`: geometry greedy, naive, boxes; visibility raycast; traversal rayquery; shading direct,
    pathtrace (hardware ray queries; needs a GPU with ray-query support);
  - `cpu-raster`: geometry greedy, naive; visibility raster; shading direct (the CPU
    software rasteriser);
  - `gpu-discover` and `cpu-discover` (discovery, the user's idea: cast few rays to find
    the visible faces, rasterise only those): storage (any GPU / CPU storage), visibility
    `quadtree` (both) or `cluster` (CPU), traversal (gpu-discover: any GPU kernel;
    cpu-discover: refloor), shading direct (gpu-discover also pathtrace);
    both draw the face set with the shared face drawer (`visibility/faces.wgsl`,
    `visibility/drawer.rs`) and choose how with `draw` = `raster` (shaded per fragment),
    `visbuffer` or `gbuffer` (shaded once per pixel in a compute pass: lit 2–4× faster on
    terrain, unlit a little slower);
  - `gpu-splat`: storage and traversal (for shadow rays), geometry points, visibility splat, shading
    direct, pathtrace; `gpu-voxelspace` / `cpu-voxelspace`: storage heightmap, visibility
    columns (Voxel Space, 2.5D), shading direct (heightmap-slope normals; shadow rays
    walk the heightmap, knob `shadows`; gpu-voxelspace also pathtrace). cpu-discover's
    GPU shadow and bounce rays walk the GPU storage chosen by `shadow_storage`.

  Only `cpu-reference`, which defines the reference images, stays a standalone renderer
  (on purpose). The former cpu-simd-u16/u8/bits/hier/prefix/palette renderers
  are now `cpu` compositions; gpu-raster, gpu-raster-naive, gpu-cubes and gpu-visbuffer
  are now `gpu-raster` compositions.

### Mesh mode

Scenes can contain animated triangle meshes (`scene.meshes`: box, sphere and torus with
vertex colours, scattered copies, and spin / orbit / bob animation; see
`plans/meshes.toml`). A pass mode is lighting plus meshes: `run.modes` takes `unlit`,
`lit`, `unlit+meshes` and `lit+meshes`, and the GUI has latching **Lit** and **Meshes**
toggles.
- *Who draws meshes:* compositions whose `meshes` stage draws them: gpu-raster `depth`
  (the default; same depth buffer) and gpu-compute `composite` (the default; meshes are
  rasterised first, rays stop at the mesh), plus the standalone `cpu-reference`.
  `meshes = none`, and back ends without a meshes stage, are skipped in `+meshes`
  benchmark passes and labelled "(no meshes)" in the GUI. The auto-tuner never changes
  `meshes` (nor the diagnostic `view` knobs): they change what is shown, not its speed.
- *Semantics* (see `mr_core::mesh`): meshes and voxels occlude each other, lit meshes
  are shadowed by voxels, and meshes cast no shadows yet.
- *For renderer authors:* GPU rasterisers can use `mr_gpu::meshes::GpuMeshes`, GPU ray
  casters `MeshComposite`, and CPU renderers `cpu_reference::mesh`.

### Knobs, hot swaps, quality probes and auto-tuning

Each renderer declares its **knobs** (`RendererInfo::params`): name, type, range, and
whether it can change *live*. Every lane also has host knobs:

- `host.render_scale`: render resolution relative to the plan's, as a live resize.
- `host.view_radius`: streamed chunks.
- `host.in_flight`: frames the host lets this renderer queue.

The **Metrics → stages & knobs** section shows them in the stage grid, each next to its
component, as sliders, checkboxes and dropdowns.
Live knobs apply between frames. Knobs marked ↻ need a restart, which is done as a
**hot swap**: a replacement worker spawns, streams the world and renders three unseen
warm-up frames, while the old one keeps rendering. Then it takes over in one step.
Spawning is asynchronous, so a starting or restarting renderer never stalls the others.

**Diagnostic views** (`view` knobs) show how much work is done where, like a thermal
camera. Numeric views (`steps` on gpu-compute and cpu, `overdraw` on gpu-discover) map each
pixel's value through one shared colour ramp (Turbo), on a scale from 0 to `view_max`
(white above it). Category views (gpu-discover `tiles` / `rays` / `churn` / `quads`,
cpu-discover `clusters` / `kinds`) use fixed colours. The renderer reports what the colours mean
with every frame (`FrameStats::legend`); the viewport draws that legend in the lane's
corner and, over a numeric view, the value under the cursor.
💾 copies a lane's current knob values back into the plan.

**Quality probes** ("probe every N s" in the toolbar, or C) render the current view on
every lane and on the reference at the same moment, without pausing anything. Each probe
records PSNR, SSIM and **MS-SSIM** (multi-scale structural similarity), plotted over time.
Lanes rendering at reduced scale are bilinearly upscaled (centre-aligned) before scoring,
so resolution loss counts as quality loss.

Which metric you tune against matters. These worlds have pixel-scale texture detail, so
PSNR drops from about 59 dB to about 26 dB at *any* resolution change (even 0.98×) and
then hardly moves. MS-SSIM degrades gracefully, and it is the tuner's default.

**Auto-tune** (per lane) turns that lane's unlocked knobs (🔒 locks one) towards a goal:

- **Goals:** *max quality @ fps*, *max fps @ quality floor*, or *max fps*. Constrained
  goals are lexicographic: meet the constraint first, then optimise the other quantity.
- **Algorithms:**
  - *local search*: randomised hill climbing with per-knob adaptive steps, early abort of
    collapsing trials, memory of rejected moves, and periodic re-measurement because the
    scene drifts;
  - *resolution controller*: the classic dynamic-resolution feedback loop on
    `host.render_scale`, as a baseline;
  - *bayesian*: Gaussian-process models of log-fps and quality over all unlocked knobs
    (wide integer ranges are log-encoded). The acquisition is constrained expected
    improvement: `P(fps ≥ target) × EI(quality)`, or the reverse for a quality floor. It
    changes several knobs per trial, keeps a sliding window of observations so it can
    follow a drifting scene, starts close to the current settings, and re-checks the best
    configuration every few trials (`src/bayes.rs`).
- **Quality metric:** MS-SSIM (default) or PSNR, in dB.
- **Measurement:** fps is throughput over a window that starts after a settle time.
  Quality comes from probes the tuner requests itself. Measurement pauses during hot
  swaps and resizes. "May change ↻ knobs" lets it hot-swap.
- **Noise:** each window is split into three sub-windows to estimate the fps standard
  error. A change counts as an improvement only if it beats both the goal's margin and
  two standard errors of the difference, so the tuner doesn't chase jitter.
- **Explore → hold:** after *patience* trials without improvement (default 14; three
  times that while the goal is still unmet), it applies the best configuration and only
  re-measures it. If the goal is violated or the score degrades for three checks in a
  row (the scene changed), it explores again from the best known point.
- **Stopping** applies the best configuration found and logs what it restored in the
  lane's events. The panel shows the best result, how it differs from the starting
  knobs, and a plot of every knob's value over time.

The tuner is pure logic in `src/tune.rs`, unit-tested against a synthetic renderer. Watch
it on real terrain with:

```sh
LOCK=host.view_radius TARGET_FPS=900 SECONDS=60 ALGO=bayes \
  cargo test --release --bin multirenderer tuner_demo -- --ignored --nocapture
RENDERER=gpu-compute:prefix+refloor cargo test --release --bin multirenderer scale_curve -- --ignored --nocapture
```

`scale_curve` prints quality against render scale for a renderer under all three metrics.

`view` (the minimal CLI viewer) controls: WASD/Space/Shift to move, Ctrl to boost, left-drag to look, mouse wheel
to change speed, `L` to toggle lit/unlit, `P` to print the current camera as a plan
snippet, and Esc to quit.

Each `run` writes `runs/<timestamp>-<name>/` containing `plan.toml` (with the overrides
used), `report.json` (every metric, including per-frame records), `summary.txt`, `logs/`
(one per worker) and `frames/` (saved frames and diff images).

## Architecture

```
multirenderer (host)                         worker process (one per renderer)
├─ plan.rs     TOML run plans + -s overrides ├─ mr-worker: IPC loop, owns one Renderer
├─ session.rs  frame loop, in-flight frames, │    └─ Box<dyn mr_core::Renderer>
│              pacing, outputs               │
├─ stream.rs   per-renderer chunk streaming ─┼─▶ Unix socket: Init / Updates / Render / Flush
├─ worker.rs   spawn / handshake / logs      │◀─ Hello / Ready / FrameDone / Error
├─ compare.rs  PSNR / MAE / bad-pixel %      │
├─ report.rs   summaries, tables, JSON       └─ pixels: shared-memory slots (/dev/shm)
└─ window.rs   live view (winit+softbuffer)
crates/mr-core   shared vocabulary: camera, chunks, faces/UVs, lighting, Renderer trait
crates/mr-world  generators, textures, authoritative world store, scripted scenes
crates/mr-ipc    protocol messages, framing, shared-memory frame slots
crates/mr-gpu    wgpu helpers: adapter choice, frame ring (readback/timestamps), GPU voxel world,
                 lighting WGSL with a pluggable `occluded()` (DDA, ray query, tree, ...)
crates/mr-mesh   greedy mesher and quad decoding shared by the raster and triangle-RT renderers
components/      mr-components: the registry, compatibility rules and `Composition`, plus one
                 directory per component: storage/<name>/{gpu.wgsl,cpu.rs},
                 geometry/<name>/..., visibility/<name>/..., traversal/<name>/...,
                 shading/<name>/...
backends/*       back ends that assemble compositions: gpu-compute, cpu, gpu-raster, gpu-rt,
                 cpu-raster, gpu-discover, cpu-discover, gpu-splat, gpu-voxelspace, cpu-voxelspace
renderers/*      cpu-reference only: the standalone reference renderer
```

- **Isolation.** By default every renderer runs in its own process: the host binary
  re-executes itself as `multirenderer worker`. A crash, leak or GPU hang only takes down
  that worker, and the run records the error and continues. `run.isolation = "thread"`
  runs workers on host threads instead, for debugging or profiling in the IDE.
  `run.launcher` prefixes the worker command (for example `systemd-run --user --scope -p
  MemoryMax=8G`, `nice`, `taskset`, or a profiler).
- **The renderer owns its data.** It receives an ordered stream of `WorldUpdate`s
  (chunk loads and unloads, block edits, lights, environment) and builds whatever
  structures it likes. Updates always arrive before the `FrameRequest` they precede, so
  runs are deterministic.
- **Asynchronous frames.** `render` may return before the frame is finished. Up to
  `run.in_flight` frames are outstanding at once. GPU renderers wake the worker through a
  `Waker` when work completes, so completion latency doesn't depend on polling.
- **Streaming.** The host tracks what each renderer holds and streams chunks within
  `run.view_radius` of the camera, nearest first. Either everything is delivered before
  each frame (`streaming = "complete"`, deterministic) or there's a per-frame budget
  (`streaming = { budget = N }`, realistic pop-in that shows up as lost quality).
  World generation is done before timing starts.
- **Measurement.** The host measures latency (Render sent to FrameDone received) and the
  interval between frame completions. Renderers report CPU time, GPU timestamp time and
  any counters they like. The report also includes init time, initial streaming load
  time, peak RSS of the worker and the GPU memory the renderer reports.

## Conventions every renderer must follow (`mr-core`)

| What | Rule |
|---|---|
| Axes | Right-handed with +Y up. `yaw = 0` looks down −Z, positive yaw turns right. |
| Primary rays | One ray through each pixel centre (`Camera::ray_dir`). Row 0 is the top row. |
| Precision | Camera position is `f64`. Work relative to `Camera::split_position()`. |
| Chunks | 32³ cells, index `x + 32*(z + 32*y)`. A chunk that isn't loaded is air. LOD `L` cells cover 2^L blocks. |
| Face UVs | `Face::uv`: viewed from outside with +Y up, `u` points right and `v` points down. Nearest sampling via `texel_coord`. |
| Unlit | Pixel = sRGB texel of the first face hit. Misses use the sky colour. **This is strict.** |
| Lit | `albedo * (ambient + sun + Σ point lights)` with hard shadows and windowed inverse-square falloff (`lighting::attenuation`), then `encode_srgb`. **This is loose:** approximations are allowed and scored. |
| Lights | Invisible. Looking straight at one shows nothing. |
| Output | RGBA8 sRGB, top row first, tightly packed. |

The `debug` block (in the `showcase` scene) has a white marker in the `u = 0, v = 0` corner
of every face, so UV or orientation mistakes are obvious.

## Run plans

See `plans/` for complete examples. All keys are listed below.

```toml
name = "my-test"

[scene]
fov_deg = 70
generator = { type = "terrain", seed = 7 }   # terrain|flat|menger|room|random|showcase|empty
camera = { type = "flythrough", points = [[0,110,0],[260,140,120]] }
                                   # static|orbit|line|flythrough|spin (see mr-world/scenario.rs)
environment = { no_sun = true, ambient = [0.02,0.02,0.03], sky = [0,0,0], exposure = 1.0 }
lights = [{ type = "scatter", count = 400, extent = [600,80,300], on_surface = true }]
events = [{ type = "random_edits", count = 2000, radius = 40, block = "toggle" }]
                                   # sphere|box|random_edits|animate_lights|sun_cycle

[run]
frames = 600            # measured frames; the camera path spans these
warmup = 30
resolution = [1920, 1080]
modes = ["unlit", "lit"]
view_radius = 16        # chunks
max_lod = 0            # LOD levels streamed (each covers view_radius chunks at 2x the cell size)
in_flight = 2
pacing = "max"          # or { fps = 60 }
streaming = "complete"  # or { budget = 400 }
parallel = false        # true: all renderers at once (side by side in the window)
isolation = "process"   # or "thread"
launcher = []
repeats = 1             # >1: passes repeat round-robin; a median/spread table is added

[[renderer]]
compose = { backend = "gpu-raster", geometry = "greedy", visibility = "raster" }  # or name = "cpu-reference"
label = "raster-sunshadow"         # optional; lets you run one renderer with several option sets
options = { shadows = "sun" }      # renderer-specific
view_radius = 8                    # optional per-renderer override of run.view_radius
max_lod = 2                        # optional per-renderer override of run.max_lod

[output]
window = false
readback = "auto"                  # auto|always|never
save = { every = 100, diffs = true }
compare = { reference = "cpu-reference", every = 100 }
dir = "runs"
```

Override any key with `-s dotted.key=value`, and remove one with `-u dotted.key` (e.g.
`-u output.save` for no saved frames).

### Sweeps, observations and the catalogue

A `compose` field may be `"*"` (every component the back end has for that stage; for
`backend`, every back end) or a list. The renderer then expands into one renderer per
valid combination:

```toml
[[renderer]]
compose = { backend = "*", storage = "*", geometry = "*", prepass = "*", visibility = "*", traversal = "*", shading = "*", meshes = "*" }
```

- Stages left out keep their default. `"*"` on a stage a back end lacks is ignored, and a
  *named* component there skips that back end (`backend = "*", storage = "palette"` runs
  only the back ends with a `palette` storage).
- The entry's `options` go to each combination whose back end declares them.
- In each pass, combinations the pass mode cannot tell apart run once
  (`Composition::effective`): shading in unlit passes (every shading component shows the
  texel), the meshes stage without meshes, and gpu-raster's and gpu-splat's storage (only
  their shadow rays walk it) in unlit passes. The full sweep expands to 458
  compositions, of which 121 are distinct in unlit passes and 258 in lit ones.

Every `run` writes `observations.jsonl` to its run directory: one record per renderer and
pass, with the machine (CPU, threads, memory, GPU, driver), the scenario (the plan's name
and scene, resolution, view radius, frames, streaming, mode), the scenario's **measured
features** (`src/features.rs`, renderer-independent, measured once per run in well under
a second: camera speed and turn rate; chunks at start and loads per frame; non-empty
chunk, solid and surface fractions in range; sky fraction and hit distances from a ray
grid; edits per frame; lights; mesh triangles), the composition and knobs,
and the results (fps, frame-interval mean / p50 / p99, GPU / CPU ms, latency, init and
load time, memory, PSNR mean / min, bad pixels, the renderer's counters averaged). They are
the raw material for the catalogue, and later for a planner that learns this machine
(IDEAS §14).

`multirenderer catalogue <dirs…> [--out DIR] [--min-psnr 35]` reads any number of runs and
writes `catalogue.md` and `catalogue.csv` (the CSV carries the scene features as columns).
Per scenario (plan × mode) it ranks the renderers
by fps; renderers below the PSNR floor count as incorrect (ranked, never best). Per renderer
it lists:
- the geometric mean of its speed relative to each scenario's best;
- wins, and where it is within 10% of the best (*good at*);
- where it is comparatively strong (*stands out*: at least 1.25× its own average relative
  speed);
- where it is weak (below 0.25), incorrect, or failed.

Per back end and stage, it shows the paired effect of each component: fps against the
best of otherwise equal compositions.

`scripts/sweep.sh [plans…]` runs the full sweep over every scenario plan, one night's
work at each plan's own length (`FRAMES=120` for shorter, faster camera paths), into
`target/sweep/<time>/`, then writes the catalogue there.

Every value can be overridden from the command line, for example
`-s run.resolution=[3840,2160] -s 'renderer=[{compose={backend="gpu-compute",storage="prefix"}}]'`.

## Renderers

| Name | Category | Technique |
|---|---|---|
| `cpu-reference` | CPU ray casting | Multi-threaded f64 two-level DDA (chunk, then voxel), exact lighting with a shadow ray per light. Defines ground truth. |
| `cpu-raster: greedy/naive + raster` *(back end; was the cpu-raster renderer)* | CPU software raster | The geometry component's quads (`greedy` or `naive`), then homogeneous clipping, tiled multithreaded edge-function rasterisation into a depth + visibility-ID buffer, then deferred shading from an exact per-pixel ray–plane hit, with CPU ray-traced shadows. Options: `tile_width`, `band_height`. |
| `cpu: prefix` *(composition; was `cpu-simd-prefix`)* | CPU ray casting | The user's hierarchy: Morton-ordered 4³ occupancy masks (bits Morton-ordered too) plus a popcount prefix array, so any aligned 2–16 cube's emptiness is one compare. The prefix doubles as a rank into compressed, solid-only block ids. Fewest steps of any variant (−31 to −51% vs u16) and 20% less world memory, but 3–9% behind `hier` per pixel: big steps cost more than unit steps in this traversal. `visibility = "incremental"` (with `exact`, default on) is the incremental-level hierarchical DDA; it brings prefix within 2% of hier on terrain (+13% for prefix) but loses on short rays. `skip2`…`skip16` choose the levels. |
| `cpu: bricks/u8/bits/hier/palette/brickmajor/morton` *(compositions; were cpu-simd and `cpu-simd-u16/u8/bits/hier/palette/brickmajor/morton`)* | CPU ray casting | The `cpu` back end with each storage component: `bricks` (u16 ids, cpu-simd's layout), u8 ids, 1-bit occupancy (one u64 per 4³ brick) with ids fetched on hit, `hier` (bits + 8³ region skips), and per-chunk palette compression (1–16-bit indices, 7× smaller on terrain). Knob `perf` adds hardware counters (cycles, instructions, cache and TLB misses per pixel). `brickmajor` / `morton` order the u16 ids so each 4³ brick is contiguous: L1 misses halve, but cycles do not drop (CPU traversal is not memory bound), so they run 3–4% behind `bricks`. Knob `schedule = tiles` (with `tile_order` morton/hilbert/linear and `tile`) hands out screen tiles along a space-filling curve with 2D packets (was cpu-simd-tiles; +4% over rows). Software prefetch (cpu-simd-loc's `prefetch`) was not carried over: it only added instructions. |
| `cpu-voxelspace: heightmap + columns` *(back end; was the cpu-voxelspace renderer)* | CPU, 2.5D | Rayon port of gpu-voxelspace (one task per screen column); identical image, about half its speed. |
| `gpu-raster: … + greedy/naive/cubes + raster/visbuffer` *(back end; were gpu-raster, gpu-raster-naive, gpu-cubes, gpu-visbuffer)* | GPU raster plus hybrid | The `geometry` component meshes chunks on the CPU (parallel, incremental) into 8-byte instances: `greedy` quads (knob `merge` = `block`, or `exposure`: merge across block types with a per-pixel block lookup; −16% quads and −13% GPU time on menger, slightly slower on terrain), `naive` one quad per exposed face (2.6–18× more faces than greedy; `cull = false` emits every face), or `cubes` one 36-vertex instance per surface block. Vertex pulling, one `multi_draw_indirect`, per-chunk frustum and per-face-direction culling, analytic per-pixel UVs. `visibility` = `raster` (forward shading, `depth_prepass` in lit mode), `visbuffer` (ids + depth, then one compute shading pass per pixel: 1.5–2.5× faster lit with lots of overdraw; halves lit time for cubes) or `gbuffer` (was gpu-hybrid: the raster pass writes hit point, face and block, then one compute shading pass; 23% faster lit than the former renderer, a little slower than visbuffer). Lit-mode shadow rays walk the composed `storage` with refloor (7–18% faster than the former fixed walk). Options: `shadows`, `max_steps`, `mesh_budget`, `quad_buffer_mb`, `pool_mb`. |
| `gpu-rt: greedy/naive/boxes + rayquery` *(back end; were gpu-rtx and gpu-rtx-bricks)* | GPU hardware ray tracing | wgpu ray queries on the RT cores. One BLAS per chunk from the `geometry` component, a TLAS of chunks rebuilt each frame relative to the camera block. `greedy` / `naive`: quads as triangles, traversal and intersection entirely in hardware (knob `cull_faces` for primary rays); `boxes`: one AABB per occupied 4³ brick with a small software DDA through the voxel pool inside each candidate. Shading `direct` or `pathtrace` (bounce rays are ray queries too). Same images and speed as the former renderers. Options: `fast_build`, `build_budget`, `shadows`. |
| `gpu-compute` *(back end; replaces gpu-dda, gpu-dda-lod, gpu-sdf, gpu-bitmask, gpu-tree64, gpu-svo-dag, gpu-clipmap, gpu-beam, gpu-pathtrace and gpu-compose)* | GPU compute, modular | Storage and traversal as separate WGSL modules behind one interface, fused into one shader per combination (so the split costs nothing at run time). Knobs: `storage` = `bricks` (the shared pool and 4³ brick bits), `hier` (bricks + an 8³ region summary), `prefix` (the user's Morton masks + popcount prefix), `distance` (per-voxel Chebyshev distance to the nearest solid, so `sphere` leaps whole empty cubes; was gpu-sdf) or `rows` (one u32 occupancy bitmask per 32-voxel row, so `rowrun` bit-scans empty runs; was gpu-bitmask, 5–15% slower than bricks + refloor) or `tree64` (was gpu-tree64: a sparse 64-tree per 64³ region, 4×4×4 child masks, popcount-packed children, compressed leaves, uniform subtrees collapsed; it owns its data instead of the voxel pool, so ~15% of the VRAM; within 5% of the former renderer's speed) or `svo_dag` (was gpu-svo-dag: per-chunk binary octrees with identical subtrees hash-consed into one global DAG, ~3× dedup on terrain, the smallest footprint; as fast as the former renderer) or `clipmap` (was gpu-clipmap: camera-centred toroidal 3D textures read with `textureLoad` only, no hash table; 36–39% faster than the former renderer because the slot wrap happens once per chunk instead of per access); every storage works with every traversal (`refloor`, `incremental`, `sphere`, `rowrun`) through adapters for missing fast paths; `visibility` (alias `traversal`) = `refloor` (gpu-dda's walk) or `incremental` (the per-level DDA from step 1), with `exact` and `skip2`…`skip16`; `view = "steps"` shows loop iterations per pixel on the colour scale (`view_max`); `dispatch = "fragment"` runs the pixels in a fragment shader over a full-screen triangle instead of compute (was gpu-frag-dda; same speed as compute). `bricks` + `refloor` is gpu-dda's algorithm and runs 7–9% faster (no LOD code); `prefix` + `refloor` is the fastest GPU ray walk on open scenes (−13% terrain, −23% edits). `incremental` is 3.5–8× slower on NVIDIA for reasons still unknown (1.3× on AMD). **LOD:** with a per-renderer or run `max_lod`, pool storages with `refloor` walk level-of-detail chunks (cells 2× coarser per level away from the camera, textures still tiled per block; was gpu-dda-lod, same image and speed; `prefix` with LOD is 22% faster). Other storages, walks and the beam prepass run without LOD (with a warning). |
| `gpu-discover: <storage> + quadtree` *(back end; was the gpu-discover renderer)* | GPU compute discovery + raster | Discovery rays, exposure tests and shadow rays read the composed `storage` (any GPU storage; refloor walk); identical rays, faces and images to the former renderer over `bricks`, 8–10% faster over `prefix`. Self-directed visibility discovery: adaptive screen quadtree of corner rays (coherent tiles accept all exposed faces on the hit plane, otherwise split, down to 1 px), a GPU hash set of discovered faces kept for `keep_frames`, then only those faces are rasterised. 0.05–0.34 rays/px, audit misses ≈0–1 px/frame. Not yet faster than rasterising everything: levels serialise and late rays are incoherent. Knobs: `tile`, `criterion` (plane/face), `sky_tile`, `keep_frames`, `jitter`, `audit`. `incremental = true` rasterises last frame's faces first and only re-discovers tiles that are uncovered (with a reprojected sky memory), near occlusion edges whose parallax may reveal something, in changed chunks, or due for refresh: −83% rays and −33% GPU time at walking speed with equal or better quality, but 13–19% slower for fast flight (see IDEAS.md §3). `events = true` (default) classifies frames: a still camera skips discovery entirely (17× less GPU time), rotation-only frames skip seen-marking and parallax (`still_holes` now/late), and edits mark only their own blocks. `view = tiles | rays | churn | overdraw | quads` shows why tiles were discovered, where rays went, which faces are new, fragments per pixel (on the colour scale, `view_max`), and drawn quads. Also: `order = painter` (exact back-to-front without depth test), `merge = rows` (half the quads), `inline_fp`, `morton`. |
| `cpu-discover: <storage> + cluster` *(was cpu-cluster)* | CPU discovery + GPU raster | The user's face-cluster discovery (IDEAS §10): seed rays on a coarse-to-fine pixel lattice; from each hit, a flood fill over coplanar faces confirmed by rays to their corners (each corner cast once); confirmed faces cover their pixels. More complete than the quadtree on every scene at about the same rays (terrain 0.24 rays/px, 51.0 vs 43.8 dB; menger 51.5 vs 26.4 dB), 2.5× its CPU time. Knobs: `seed_step`, `sky` (corners / sparse), `sky_tile`; views `clusters` and `kinds`. |
| `cpu-discover: <storage> + quadtree` *(back end; was the cpu-discover renderer)* | CPU discovery + GPU raster | The same discovery idea with the quadtree on the CPU (any CPU storage, refloor walk, rayon; `visibility` was the `algorithm` knob, still accepted), pipelined: frame N+1 is discovered while the GPU draws frame N, optionally for the extrapolated camera (`predict`). Synchronous (`pipelined = false`): 0.25 rays/px finds all but ~15 faces on terrain (PSNR 49.8). Pipelined: one frame of latency costs 0.1–0.3% of pixels when moving. CPU discovery (25–60 ms) is the bottleneck. |
| `gpu-splat: <storage> + points + splat` *(back end; was the gpu-splat renderer; shadow rays walk the composed storage)* | GPU compute, approximate | Software point splatting: every exposed block is a point, projected with 64-bit-free `atomicMin` depth then id passes, resolved by ray–cube intersection. Edges fatten (PSNR ~20–27 dB). |
| `gpu-voxelspace: heightmap + columns` *(back end; was the gpu-voxelspace renderer)* | GPU compute, 2.5D | The 1992 Comanche "Voxel Space" technique: one thread per screen column marches a heightmap front to back with a y-buffer. No overhangs, caves or trunks (PSNR ~15–36 dB), 27 MB of GPU memory. |

GPU buffers (voxel pools, quad and geometry buffers, tree nodes) start small and grow on
demand up to their `*_mb` option, so the reported GPU memory is what a renderer actually
uses and several GPU renderers fit side by side in the live view. Set `MR_GPU_DEBUG=1`
to enable wgpu debug labels and validation. They're off by default because Vulkan's
debug-utils path crashes intermittently when several renderers share one process
(`isolation = "thread"`).

`gpu-rtx*` need a GPU with ray-query support (Vulkan `VK_KHR_ray_query`) and use
wgpu's experimental ray-tracing API. Any wgpu validation error now fails a renderer
instead of letting it report a frame rate for frames it never drew.

GPU renderers pick the discrete GPU on hybrid laptops. Override this with
`run.devices = { gpus = ["nvidia"] }` (or a renderer's own `devices = { ... }`; `cpu_threads` sets
CPU stages' threads), or set `WGPU_BACKEND` to choose the backend.

### Adding a renderer

**New techniques go in as components, not new renderers.** Add a directory under
`components/src/<slot>/<name>/` with the implementation for each back end that supports
it (`gpu.wgsl` for gpu-compute, `cpu.rs` for cpu), and register it in `COMPONENTS` in
`components/src/lib.rs` (slot, back ends, the optional queries it `provides` / `uses`).
The `registry_is_consistent` test checks that the registry and the implementations agree.
The steps below are for the legacy, monolithic kind.

Declare knobs in `RendererInfo::params` (`ParamSpec::int/float/bool/choice`, with the same
names as your `options`). For each knob marked `live`, implement `Renderer::set_param`.
The GUI and the tuner then pick them up automatically.

A diagnostic `view` writes its colours through `mr_core::legend` (`ramp_wgsl()` in a shader,
`Legend::color` on the CPU) and returns the matching `Legend` in `FrameStats::legend`, so
the GUI can draw the scale and read values back.


1. Create the crate with `cargo new --lib renderers/my-renderer`, add it to the workspace
   `members` and `[workspace.dependencies]`, and depend on `mr-core` (plus `mr-gpu` for
   wgpu plumbing).
2. Implement `mr_core::Renderer` and export `pub fn entry() -> RendererEntry`.
3. Add it to `registry()` in `crates/mr-worker/src/lib.rs`.
4. Run it with `multirenderer run plans/showcase.toml -s 'renderer=[{name="my-renderer"}]'`.

Renderers that complete frames asynchronously must call the `Waker` they receive when
work may have finished. `mr_gpu::FrameRing` does this for you. GPU renderers never open a
device: `create(params, devices, waker)` receives the host's devices, and
`mr_gpu::gpu(devices, slot, &GpuNeeds { .. })` returns one that has the features asked for
(the slot's adapter comes from the plan's `devices` configuration).

## Findings so far

Measured on a Ryzen 9 9955HX and an RTX 5060 Laptop GPU. Numbers are 1080p on the
terrain flight unless noted. They're indicative; see `run.repeats`.

- **GPU time:**
  - gpu-sdf has the cheapest traversal (0.9 ms, against 1.3 ms for gpu-dda), but is 2×
    slower under heavy edits, because edited chunks fall back to plain stepping until
    their field is recomputed.
  - gpu-rtx traces fastest on the RT cores, but its BLAS rebuilds make it CPU-bound when
    chunks stream or change.
  - gpu-tree64 matches gpu-dda's speed with about 15% of its VRAM.
- **LOD:** at the same far view distance (768 blocks), `gpu-dda-lod r6+2` uses 136 MB
  of VRAM against 1037 MB at full resolution, and is faster. The quality loss is
  concentrated in pixel-scale foliage at distance.
- **CPU:**
  - cpu-raster is the closest match to the reference (64–99 dB), but its cost scales
    with face count (Menger sponge: 8 fps).
  - cpu-simd is about 1.5× the reference; avoiding libm `floor` calls was the big win.
    SIMD packets don't help, but **not** because of memory: hardware counters
    (the `cpu` back end with `perf = true`) show ~80 cycles and ~220 instructions per DDA step at
    IPC 2.6, with L2 misses costing ≤3% of the time. Shrinking the world data 7–16×
    (bits, palette) leaves cycles per pixel unchanged. What helps is fewer steps:
    `cpu: hier` (formerly `cpu-simd-hier`) skips empty 8³ regions (20–34% fewer steps, up to +29% fps). The
    next wins are fewer instructions per step (register spills, saturating float→int
    casts, bounds checks in the hot loop).
- **Metrics:** PSNR treats any resampling as a ~30 dB loss on this content; MS-SSIM is the
  better tuning target across resolutions. With MS-SSIM the tuner keeps shadows and
  lowers resolution; with PSNR it does the opposite.
- **Benchmark noise:** GPU time for the same configuration has varied 1.7–3.0 ms between
  separate runs (boost clocks and power state). Interleaved repeats keep the spread
  within ±1%.

## Ideas to explore next

- Hardware RT with ray-tracing pipelines (closest-hit shaders) instead of ray queries.
- A hierarchical (brick-level) beam pre-pass: the chunk-level one in `gpu-beam` saves nothing.
- `gpu-discover`: seed refinement from the previous frame's face set (rasterise it to an ID buffer and only send rays where it is inconsistent), one persistent kernel for all levels.
- A denoiser and jittered primary rays for `gpu-pathtrace`; emissive blocks.
- Power-of-two slots and a uniform-chunk flag for `gpu-clipmap`.
- Mesh shaders, GPU-driven culling (Hi-Z occlusion), and asynchronous or GPU meshing.
- LOD for the rasteriser and the tree, and LOD that reflects edits.
- Baked or propagated (flood-fill) lighting compared against ray-traced shadows on the
  quality/speed curve.
- An HDR output pipeline: renderers emitting linear float images.
