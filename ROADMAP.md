# Roadmap

Ideas for the bench itself, none built or tried yet. Ideas about rendering techniques are
entries of the [encyclopedia](docs/schemes/README.md); the design already built is in
[docs/design.md](docs/design.md).

## A planner that learns the machine

Like the auto-tuner, but with memory across sessions about the user's machine: it uses all the
information it can to pick a decent render plan (pipeline and knobs) very cheaply, and keeps
learning, the whole time the user interacts with it, how best to render things on that
machine. It replaces or extends the tuner; sweeps over every combination give it data about
every pipeline on the machine. Precedent: FFTW's *wisdom* (plans measured on the machine and
kept; M. Frigo, S. G. Johnson, The design and implementation of FFTW3, Proc. IEEE 2005).

A sketch:
- **Features, not scenario names:** a cheap feature vector of the scene and request
  (resolution, view radius, solid and surface fractions, camera speed and turn rate, edits per
  frame, lights, mesh triangles, mode), so it generalises from the sweep's scenarios. The
  observations already record these features.
- **Constraints, then the objective:** mode support, a quality floor and memory budgets
  filter; frame time (mean, p99), latency and later power are the objective.
- **Two levels:** a pipeline change costs a hot swap (seconds), a knob change is live; so a
  learned model picks the pipeline rarely (at scene load, or on large drift) and the Bayesian
  tuner tunes its knobs continuously.
- **Model:** small and honest about uncertainty (per-pipeline regression of log frame time on
  the features, or nearest neighbours); exploration on a hidden standby lane or in idle time,
  so the user sees no stutter. Learning during use is a contextual bandit.
- **Noise and drift:** observations keyed by machine and driver, recent ones weighted more,
  the incumbent re-measured now and then.
- Per-stage timings let it predict a pipeline as a sum of stages instead of measuring every
  combination.

What exists for it: observations with machine, scenario features, pipeline key and per-stage
times (`observations.jsonl`), sweeps, the catalogue, the tuner.

## The bench as a renderer library

A simpler, library-style interface so the project can be used as a renderer: the caller's
scene, edits and meshes are the input, and the planner picks how to render them. The
`Renderer` trait with `WorldUpdate`, mesh updates and `FrameRequest` is close to the input side
already. A library also needs: renderers in-process (`isolation = "thread"` exists); one
canonical world copy on the library's side, so a pipeline switch can re-stream the world into
the new renderer (the live session does this for hot swaps); and output into the caller's
surface: frames are read back to host memory today, so GPU back ends would render into a
caller-provided device and texture (devices are already handed in, `GpuPool::fixed`).

## A cached inventory of every device and API

Enumerate every available graphics API at start-up and cache the result, so that a node's API
becomes a real choice per device, offered from what the machine has, instead of fixed to
Vulkan. (Raised after a Windows guest without Vulkan found no adapter.)

`wgpu::Instance` with `Backends::all()` and `enumerate_adapters` lists every adapter on every
API (Vulkan, DX12 and GL on Windows; Vulkan and GL on Linux; Metal on macOS); one GPU appears
once per API. Adapters are live driver handles and cannot be cached, but the *inventory* can,
per machine, on disk: per adapter its name, API, device type, driver version, features and
limits. The GUI, the planner and plan validation read it without touching a driver; a worker
opens only the API it was assigned. Invalidation: driver versions plus one cheap API's adapter
list. Today each worker creates its own instance and picks Vulkan unless `WGPU_BACKEND` says
otherwise.

Open: the cost of bringing up every API (GL may create a hidden context; likely tens of
milliseconds); several Vulkan instances on threads of one process are not safe with debug
labels; measurements must record the API used (they record the adapter's backend already);
DX12 needs its shader compiler (DXC), linked in on Windows builds.

## More pipeline compilation and device splits

- Which further pipelines to precompile, chosen by the planner's run-time feedback.
- A possible extra back end: the WGSL components on the CPU through a software Vulkan driver
  (lavapipe).
- Other device splits: other record writers, peer copies, splitting a node by screen tiles or
  ray batches.

## Docking panes

As in common GUI tools: every pane (workflow form, TOML, metrics, compare, benchmark, keys,
log, the pipeline editor, ...) can be torn off into its own OS window or rearranged in the main
one. Fullscreen stays on the display the main window is on, also with panes torn off.

Three docks: left, right and bottom; the bottom spans the full width, left and right run down
to its upper edge; the middle is the main view (the renderers' viewport, or the canvas). An
empty dock collapses; a full one may be collapsed by the user; collapsed, a dock is a thin
border at the screen's edge (click or hover to open). Candidates: `egui_dock` (tabs, splits,
tear-off into viewports) if it fits the three-dock layout, else a thin dock layer over egui's
multi-viewport support.

## Game controller support

Flying the camera with a gamepad (sticks for move and look, triggers for speed, buttons for the
mode and lane toggles) in the GUI's viewport and the view and presenter windows, beside the
keyboard and mouse. A gamepad library (e.g. `gilrs`, cross-platform) polled once per frame
into the same camera control the keys drive, with a dead zone and a response curve; the
bindings shown in the keys pane. Analog input makes recorded flights smoother, so a recorded
controller path could also serve as a camera path for benchmarks.
