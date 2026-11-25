# The GUI

`multirenderer` with no arguments (or `multirenderer gui plans/terrain.toml`) opens the GUI.

## Areas

The renderers' viewport fills the middle; every other pane is a tab in one of three docks
(below, in brackets, where it starts) or in an OS window of its own (see [Docks](#docks)).

- **Form and TOML (left).** A form for every plan setting: world generator, camera path,
  environment, lights, scripted events, renderers, run settings and benchmark outputs. The
  TOML pane edits the raw plan (it re-reads the plan whenever it comes into view). Open, save and Ctrl+S are in the toolbar (Ctrl+S saves unless the canvas has the keyboard, i.e. the last click was on it: there it flies fast backwards); 📂 Open lists the
  recently loaded or saved plans first (kept in the user's configuration directory,
  `multirenderer/recent-plans.txt`), and 📂 Open ▸ New is a
  plain start (terrain, the sun, a camera to fly, one renderer: `plans/playground.toml`), and
  🎬 Demos loads a set-up that shows something off (`plans/demo-*.toml`; hovering one says
  what). "⌖ cam" copies the
  live camera into a position or a camera-path point, which is a quick way to author a
  fly-through.
- **Viewport (centre).** Every renderer in the plan runs live, tiled side by side. Each one
  renders the latest camera as fast as it can (or up to the fps cap), independently of the
  others. Frames are read back only at the display rate, so the numbers measure the renderer
  rather than the GUI. A lane's events (started, hot swaps, refused knobs, failures) show
  under its label for a few seconds; the Metrics tab keeps the full list.
- **Metrics (right).** Live fps, frame interval, latency, and CPU, GPU and update time (mean
  and p99); renderer counters, process memory, GPU memory and streaming progress, and what
  reached the renderer's client (bytes arrived and sent over its link, holes), with plots.
  Each renderer has its pipeline and knobs here, ↻ to restart it with the editor's settings,
  and ✖ to remove it.
- **Compare (right).** Renders the exact current view on every renderer and on the reference,
  then shows PSNR, MAE and the percentage of bad pixels, with the images and diff images.
- **Benchmark (right).** Runs the whole plan as a separate `multirenderer run` process,
  streams its output, and shows the report table and an fps chart. It can also load earlier
  reports from `runs/`.
- **Keys (right)** lists every key; **Log (bottom)** is the live session's log.
- **Pipeline (its own window).** The pipeline editor (below), shown while it is open.

## Docks

Three docks: the bottom one spans the full width, the left and right ones run down to its
upper edge. Drag a tab onto another dock to move it there, or over the viewport to give it an
OS window of its own; right-clicking a tab offers the same. ↗ in a dock's tab bar opens the
shown pane in a window, ◀ / ▶ / ⏷ collapses the dock to a thin strip at the window's edge.
Resting the pointer on a strip shows the dock over the view until the pointer leaves it (📌
keeps it open); clicking the strip opens it. An empty dock disappears, and comes back as a
strip to drop on while a tab is dragged. A pane's window docks it again with ↙ Dock or by
closing it. The pipeline editor can also take the middle in place of the viewport (⛶ in its
tab bar, tab menu or window; ↙ Dock or ↗ Own window there give the view back); closing its
window closes the editor. Keys and buttons that show a pane (C, H, ⏱ Run benchmark, ✏ graph)
open its dock or focus its window. Fullscreen (F11) keeps the viewport (or the pane in the
middle) fullscreen on the display the main window is on, with torn-off panes still open. The
layout and dock sizes are remembered across sessions (`multirenderer/docks.toml` in the
user's configuration directory).

## Keys

| Key | Action |
|---|---|
| F5 | start or restart the live session |
| W A S D, Space, Shift | fly (hold Ctrl for 5× speed); the mouse wheel changes the speed |
| right-drag, left-drag, arrows | look |
| L / N | lit or unlit / meshes on or off |
| F | follow the plan's camera path, or fly freely |
| P, [ ], R | play or pause the scene clock, scrub it, reset it |
| E | edit mode: click removes a block, Ctrl+click places one |
| G | drop a light at the camera |
| C | run a comparison |
| 1–9, Tab, Esc | focus one renderer, cycle through them, show them all |
| F11 | fullscreen viewport (Esc leaves) |
| Z | one frame pixel per screen pixel, snapped to the pixel grid and cropped, not scaled |
| M | set the render resolution to the tile's size and restart (F11, 1, M: native resolution) |
| O | hide the overlays |

The ❓ Keys tab lists everything.

`multirenderer view` is a minimal viewer without the GUI. It uses the same movement keys,
plus `P` to print the camera as a plan snippet and Esc to quit. It presents through an HDR
surface where the display offers one (scRGB, extended sRGB or HDR10), so a renderer with
`post = "hdr"` shows its values above 1 as brighter than white instead of clipped: H switches
between HDR and SDR, `[` / `]` make the frames darker / brighter against the desktop's SDR
white (in quarter stops). Frames' 1.0 sits at SDR white, decoded with the curve the
compositor uses for SDR windows (gamma 2.2 on Linux, sRGB on Windows), so an SDR frame looks
the same in both modes.

In the GUI, **☀ HDR** on a renderer's Metrics panel opens that renderer in a window of its own,
presented the same way (the GUI's own window is SDR). The panel then shows how the window
presents (e.g. `scRGB, 1.0 at 1.00× SDR white`) and the **paper white**: the frames' 1.0
relative to the desktop's SDR white, shared by every such window (`[` / `]` in a window set it
too). Closing the window (or Esc in it) unticks the button. A live-session restart (F5)
reopens the windows.

## The pipeline editor

A renderer is its pipeline graph (see [pipelines.md](pipelines.md)). The plan editor and the
Metrics tab show it in compact form: its executor, then one row per node with its component and that component's knobs.

**✏ graph** opens the editor in its pane (an OS window of its own to start with; a dock or the
middle, see [Docks](#docks)). The graph is laid out from the world (left) to the image
(right). Edges are coloured by the kind of data they carry. An edge whose value comes from
the previous frame is dashed, and a node shared by several readers is drawn once. Clicking a
node shows its component, device, API, the node each input is bound to (an existing node,
which shares it, or a new one), when each input's value arrives, and the node's knobs.
Dragging from an output onto an input rebinds the input. Dragging a node moves it, dragging
the background pans and the wheel zooms about the pointer; the view fits the graph until then,
and a double click on the background fits it again (with **auto-frame** on, so does
releasing a dragged node). Undo and redo are available, and
"start from…" loads a shipped graph file's pipeline (as "+ renderer" in the plan editor does
for a new renderer).

Every choice is tried before it is offered. A change is made exactly as picked: a new node comes
unbound, nothing is filled in and nothing is dropped. If the resulting graph cannot run, the
choice is orange with ✱ and hovering over it gives the reason; picking it anyway makes a draft
(shown with the reason, while the renderer keeps its last graph) to edit on from. In the live session an edit applies in place
when it changes only nodes the lane's executor replaces while running (the worker is sent the graph's
edits), and otherwise as a hot swap. Trying the choices is cheap: every
choice the editor lists on every shipped graph file (1,110) is answered in 0.07 ms each.

## What the GUI shows

What a renderer, a pass and the registry report, and where the GUI shows it.

**A live lane** (its tile in the viewport, its panel in the Metrics tab):

| Figure | Where |
| --- | --- |
| fps, frame interval (mean / p99), latency, GPU / CPU / update ms per frame | panel, `stats` (last 2 s) |
| time per stage, with the graph nodes it runs | panel, one row per stage |
| quality at the last probe (PSNR, SSIM, MS-SSIM, bad pixels) | panel, and the probe plot |
| the settled score: the last probe taken with the view held still, nothing new sent, for the renders its pipeline needs (late edges: `compare.settle`; accumulating: `compare.converge` paths per pixel), and how long ago; before one, how many renders to hold still. Not shown for a pipeline whose every frame is settled | panel, `settled (last)` |
| memory: RSS; GPU the renderer counts, now (peak since it started); device peak, measured from the GPU allocators | panel, `memory` |
| build: start-up ms, of it shader modules and pipelines (ms, count) | panel, `build` |
| chunks sent and pending, bytes arrived and sent over its link, holes | panel, `streaming` / `arrivals` |
| what the view asks that its pipeline leaves out (meshes, LOD levels, transparency, knobs) | tile |
| events (started, edits, refusals, hot swaps) of the last seconds | tile |
| a failure: the error, a dead worker's exit status or signal, the watchdog's verdict on a hung one | tile (first line) and panel (whole) |
| the node a failure or a refused knob is about: a failure names its component (`clipmap: ...`, the convention of build, device-limit and memory errors), its role when the code serves the role whatever its component (`storage: the voxel pool is full`), or a node id in backticks; a refusal, the knob's owner | that node, red with ⚠, in the panel's pipeline rows and on the editor's canvas, the message on hover (a refusal as long as the tile shows it; what names no node stays the lane's) |
| the node a draft's reason names (`accept` names nodes in backticks, `` `draw` (draw `raster`): input `faces` ... is not bound``) | that node, red with ⚠, on the editor's canvas, the reason on hover beside the lane's own faults |
| a hot swap in progress | panel |

**A finished benchmark** (the Benchmark tab, one row per pass): fps, frame ms (mean, p99), GPU
and CPU ms, latency, PSNR, the settled PSNR, bad pixels, start-up ms and its shader part
(modules + pipelines), peak RSS, the GPU memory the renderer counts and the device peak. Then
the columns only some passes fill, shown when a pass that ran reports them: the like score
(against a reference given the pass's LOD region; its `like` on hover), the converged paths
per pixel, update ms (ns per byte arrived on hover), missed deadlines (a paced pass), and,
where a pass streamed over a link that is not free, had holes or was complete only after its
first frame, wire MB (arrived bytes and chunks on hover), holes (mean / max; frames with any
on hover) and the first complete frame (or never). Each header says on hover what it is. A
failed pass reads FAILED with its reason on hover; ⚠ beside a label lists, on hover, what the
pass left out, a fault while its worker stopped, and lanes that failed beside it.

**Choices.** The editor's components per node come from the registry and the graph model
(`role_components`, tried with `accept` before they are offered); their descriptions from the
registry (`ComponentInfo::description`) or, for the roles outside it (world, generator, link,
client, post), from `graph::host_description`. Knobs are the components' declarations; views are
the quantities the graph's nodes write. The world's generators, their descriptions and their
knobs come from `mr_world::generators::GENERATORS`. The reference renderer list is the
registry's renderers. Fixed in the GUI: camera paths, the tuner's goals, metrics and
algorithms, read-back, isolation and compile modes, input timings (`now`, `prev`, `iterate`).

## Knobs and hot swaps

Each component declares its knobs: name, type, range, and whether the knob can change live.
Each lane also has host knobs: `host.render_scale` (the render resolution relative to the
plan's, applied as a live resize) and `host.in_flight` (how many frames the host lets the
renderer queue). How far a renderer loads is its **client node's** knobs (the loader behind
the link: `pick` or `derive`): `radius` (chunks held around the camera) and `max_lod`. A
live lane shows them under its client as `host.view_radius` (applied without a restart) and
`host.max_lod`; the plan's `run.view_radius` and `run.max_lod` are only their defaults.

Wherever knobs are listed they form three columns: 🔒 (a lane's tuner lock, where the tuner
can change that knob), the value (one width for every kind of widget) and the name. A typed
number applies on Enter (or when the field loses focus; Escape cancels), not per keystroke.

Live knobs apply between frames. Knobs marked ↻ need a restart, which is done as a **hot
swap**: a replacement worker starts, receives the world, and renders three warm-up frames
that are not shown, while the old worker keeps rendering. The new worker then takes over in
one step. Starting a worker never stalls the other renderers. 💾 copies a lane's current knob
values back into the plan.

## Diagnostic views

`view` knobs show how much work is done where, like a thermal camera. Numeric views (`steps`
and `distance` on raycast-gpu and cpu, `overdraw` on discover-gpu) map each pixel's value through one shared
colour ramp (Turbo), on a scale from 0 to `view_max` (white above it). `distance` is the
primary hit's distance in blocks (linear; the sky is white): where the far field starts, how
deep a scene is, and with a LOD run where the coarse rings begin. Category views
(discover-gpu `tiles` / `rays` / `churn` / `quads`, discover-cpu `clusters` / `kinds`) use
fixed colours. The view is the post node's knob (`view`, `view_max`): its choices are the
per-pixel quantities the graph's nodes write, and the frame is still shaded underneath; a
category view tints the greyed image with its colours (sky and other pixels outside its
categories keep the image). discover-gpu's `overdraw` counts the fragments of a separate
raster of the drawn face list in its order (depth-tested, or all of them with `order =
painter`), so it works with every drawer; the list's and discover-cpu's category views are
such rasters too. discover-cpu's views need cluster discovery (`visibility = cluster`).
The renderer sends a legend with every frame. The viewport draws the legend in the lane's
corner and, over a numeric view, shows the value under the cursor.

## Quality probes

"Probe every N s" in the toolbar (or C) renders the current view on every lane and on the
reference at the same moment, without pausing anything. Each probe records PSNR, SSIM and
MS-SSIM (multi-scale structural similarity), plotted over time. Lanes that render at a lower
resolution are upscaled bilinearly before scoring, so the loss of resolution counts as a loss
of quality.

A probe scores the frame as shown. A pipeline with late edges or one that accumulates lit
frames shows less while the view changes than once it has caught up; a probe taken after the
lane rendered the same view, with nothing new sent to it (no stream, edits, knobs or mesh
updates), for the renders a benchmark's settled pass holds it (`CompareSpec::held`, from the
plan's `[output.compare]`, else its defaults) is marked settled, and the lane panel shows the
last such score beside the last probe's: the live counterpart of a benchmark's settled dB.

The choice of metric matters. These worlds have texture detail at the scale of single pixels,
so PSNR drops from about 59 dB to about 26 dB at *any* change of resolution (even 0.98×) and
then hardly moves. MS-SSIM degrades gradually, which is why it is the tuner's default.

## Auto-tune

Each lane can tune its unlocked knobs (🔒 locks one) towards a goal:

- **Goals:** *max quality @ fps*, *max fps @ quality floor*, or *max fps*. Constrained goals
  are lexicographic: meet the constraint first, then optimise the other quantity.
- **Algorithms:**
  - *Local search:* randomised hill climbing with an adaptive step for each knob. It aborts
    trials that collapse early, remembers rejected moves, and re-measures periodically
    because the scene changes.
  - *Resolution controller:* the classic dynamic-resolution feedback loop on
    `host.render_scale`, as a baseline.
  - *Bayesian:* Gaussian-process models of log-fps and quality over all unlocked knobs (wide
    integer ranges are log-encoded), with constrained expected improvement as the
    acquisition function (`P(fps ≥ target) × EI(quality)`, or the reverse for a quality
    floor). It changes several knobs per trial, keeps a sliding window of observations so it
    can follow a changing scene, and re-checks the best configuration every few trials.
- **Measurement:** fps is the throughput over a window that starts after a settle time.
  Quality comes from probes that the tuner requests. Measurement pauses during hot swaps and
  resizes. "May change ↻ knobs" allows the tuner to hot-swap, and to change a node's component (the knobs `<node>.component`: what the graph says the node may be; a graph edit, without a restart where the executor replaces that node while running).
- **Noise:** each window is split into three parts to estimate the standard error of the
  fps. A change counts as an improvement only if it beats both the goal's margin and two
  standard errors of the difference.
- **Explore, then hold:** after *patience* trials without improvement (14 by default, three
  times that while the goal is unmet), the tuner applies the best configuration and only
  re-measures it. If the goal is violated, or the score degrades three checks in a row, it
  explores again from the best known point.
- **Stopping** applies the best configuration found and records it in the lane's events.

The tuner is plain logic (`src/tune.rs`, `src/bayes.rs`), unit-tested against a synthetic
renderer. To watch it on real terrain:

```sh
LOCK=host.view_radius TARGET_FPS=900 SECONDS=60 ALGO=bayes \
  cargo test --release --bin multirenderer tuner_demo -- --ignored --nocapture
RENDERER=raycast-gpu,storage=prefix cargo test --release --bin multirenderer scale_curve -- --ignored --nocapture
```

`scale_curve` prints quality against render scale under all three metrics. `scripts/check.py gpu`
runs both briefly as tests (`SECONDS=8`).
