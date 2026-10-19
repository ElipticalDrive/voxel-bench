# The GUI

`multirenderer` with no arguments (or `multirenderer gui plans/terrain.toml`) opens the GUI.

## Areas

The renderers' viewport fills the middle; every other pane is a tab in one of three docks
(below, in brackets, where it starts) or in an OS window of its own (see [Docks](#docks)).

- **Form and TOML (left).** A form for every plan setting: world generator, camera path,
  environment, lights, scripted events, renderers, run settings and benchmark outputs. The
  TOML pane edits the raw plan (it re-reads the plan whenever it comes into view). Open, save and Ctrl+S are in the toolbar; 📂 Open lists the
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
- **Schemes (right).** The encyclopedia (`docs/schemes/`, built into the binary): every
  entry by category, with a filter; an entry shows its full text and links its related
  entries. Untested ideas are marked. Hovering a node in the pipeline editor shows its
  component's entry, and the inspector links to it.
- **Benchmark (right).** Runs the whole plan as a separate `multirenderer run` process,
  streams its output, and shows the report table and an fps chart. It can also load earlier
  reports from `runs/`.
- **Keys (right)** lists every key; **Log (bottom)** is the live session's log.

## Docks

Three docks: the bottom one spans the full width, the left and right ones run down to its
upper edge. Drag a tab onto another dock to move it there, or over the viewport to give it an
OS window of its own; right-clicking a tab offers the same. ↗ in a dock's tab bar opens the
shown pane in a window, ◀ / ▶ / ▼ collapses the dock to a thin strip at the window's edge
(click the strip to open it again; hovering it names its panes). An empty dock disappears,
and comes back as a strip to drop on while a tab is dragged. A pane's window docks it again
with ↙ Dock or by closing it. Keys and buttons that show a pane (C, H, ⏱ Run benchmark)
open its dock or focus its window. Fullscreen (F11) keeps the viewport fullscreen on the
display the main window is on, with torn-off panes (and the pipeline editor's own window)
still open. The layout is remembered across sessions (`multirenderer/docks.toml` in the
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
Metrics tab show it in compact form: the back end it runs on (derived from the graph, never
chosen directly), then one row per node with its component and that component's knobs.

**✏ graph** opens the editor in an OS window of its own (↙ / ↗ move it into the main window and
back; the choice is remembered for the session). The graph is laid out from the world (left) to the image
(right). Edges are coloured by the kind of data they carry. An edge whose value comes from
the previous frame is dashed, and a node shared by several readers is drawn once. Clicking a
node shows its component, device, API, the node each input is bound to (an existing node,
which shares it, or a new one), when each input's value arrives, and the node's knobs.
Dragging from an output onto an input rebinds the input. Dragging a node moves it, dragging
the background pans and the wheel zooms about the pointer; the view fits the graph until then,
and a double click on the background fits it again (with **auto-frame** on, so does
releasing a dragged node). Undo and redo are available, and
"start from…" loads a back end's default pipeline.

Every choice is tried before it is offered. If the resulting graph cannot run, the choice is
greyed out, and hovering over it gives the reason. A choice that moves the graph to another
back end says so (`→ gpu-discover`). When a new component needs inputs, they are filled in
from a back end's default pipeline that has that component. Nodes that no longer reach the
image are dropped, so the graph always runs. In the live session an edit applies in place
when the back end can change that knob live, and otherwise as a hot swap. Trying the choices is cheap: every
choice the editor lists on every preset (1,100) is answered in 0.26 ms each (930 run, 170
refused with genuine reasons).

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
and `distance` on gpu-compute and cpu, `overdraw` on gpu-discover) map each pixel's value through one shared
colour ramp (Turbo), on a scale from 0 to `view_max` (white above it). `distance` is the
primary hit's distance in blocks (linear; the sky is white): where the far field starts, how
deep a scene is, and with a LOD run where the coarse rings begin. Category views
(gpu-discover `tiles` / `rays` / `churn` / `quads`, cpu-discover `clusters` / `kinds`) use
fixed colours. The renderer sends a legend with every frame. The viewport draws the legend in
the lane's corner and, over a numeric view, shows the value under the cursor.

## Quality probes

"Probe every N s" in the toolbar (or C) renders the current view on every lane and on the
reference at the same moment, without pausing anything. Each probe records PSNR, SSIM and
MS-SSIM (multi-scale structural similarity), plotted over time. Lanes that render at a lower
resolution are upscaled bilinearly before scoring, so the loss of resolution counts as a loss
of quality.

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
  resizes. "May change ↻ knobs" allows the tuner to hot-swap.
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
RENDERER=gpu-compute:prefix+refloor cargo test --release --bin multirenderer scale_curve -- --ignored --nocapture
```

`scale_curve` prints quality against render scale under all three metrics.
