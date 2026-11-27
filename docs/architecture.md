# Architecture

## Processes

```
multirenderer (host: the server)            worker process (one per renderer: a client)
├─ plan.rs      plans, -s overrides          ├─ mr-serve: IPC loop; gates, client, generator
├─ session.rs   frame loop, pacing, outputs  │    └─ an executor (cpu, gpu) running the graph
├─ worker.rs    spawn, handshake, logs       │
├─ (mr-world)   world store, one stream ─────┼─▶ local socket: Init / Stream / Render / ...
├─ compare.rs   PSNR, MAE, bad pixels        │◀─ Hello / Ready / FrameDone / Error
├─ report.rs    summaries, tables, JSON      └─ pixels: shared-memory frame slots
├─ pipelines.rs pipelines compiled on demand
└─ gui/         the GUI (egui)
```

- **Isolation.** Every renderer runs in its own process by default: the host binary runs
  itself again as `multirenderer worker`. A crash, leak or GPU hang takes down only that
  worker; the run records the error and continues. `run.isolation = "thread"` runs workers on
  host threads instead, for debugging or profiling in an IDE. `run.launcher` prefixes the
  worker command (for example `systemd-run --user --scope -p MemoryMax=8G`, `nice`,
  `taskset`, or a profiler).
- **Failure is defined behaviour.** A renderer that errors, panics, crashes or hangs ends
  its own lane: the lane's report carries the cause (`error`; a failure while stopping goes in
  `shutdown_error`, the pass's figures standing), the other lanes and the program go on.
  What each isolation survives:

  | failure (at build, first frame, mid-run, world update, shutdown) | process | thread |
  |---|---|---|
  | error, panic | lane fails, says why | lane fails, says why |
  | abort, segfault, stack overflow | lane fails, names the signal | **the program ends** |
  | hang | killed after its deadline | abandoned after its deadline (keeps its core and memory) |
  | memory blow-up | refused or killed; `run.launcher` bounds it (`prlimit --as=...`, `systemd-run -p MemoryMax=...`) | **the host's memory** |

  The thread's limits are that choice's cost (no copy of the world, no frame transfer); the
  process is the default. The *watchdog* (`run.watchdog`) decides when silence is a hang:
  building (connecting, creating the renderer, its initial load and first frame) has nothing
  of its own to go by and gets `build_s` (default 300 s); after its first frame a wait may last
  `factor` (default 20) times the longest the lane has needed for anything so far, its build
  and initial load included, never under `min_s` (default 2 s, below which the machine's own
  hiccups would be what times out). Scripted runs and the GUI apply it alike, in the GUI to a
  lane's worker and to the replacement a hot swap is warming up (a hung replacement ends the
  swap; the lane keeps its worker). Presenter windows go by it too: connecting is their
  build (`build_s`), closing may take `factor` times as long as connecting did (at least
  `min_s`), then the process is killed. The host never waits on a killed process past its
  deadline: one the kernel holds (seen on RADV: a worker whose GPU was lost, stuck in the
  driver's teardown with the kill pending) is reported "killed, but not yet ended" and
  reaped by a thread whenever it ends. A process's death frees the device memory its
  renderer held (the driver's, measured: 1247 MiB used before, 1459 MiB during, 1247 MiB
  after each of five runs whose GPU workers segfaulted, aborted or hung and were killed).
  `tests/survival.rs` drives the binary through every moment x way in both isolations (with
  `MR_FAULT_INJECTION=1` and a renderer option `fault = "<at>:<how>"`, see
  `crates/mr-serve/src/fault.rs`), and a sweep's sequential passes past failing ones (each
  recorded in `observations.jsonl` with its reason, listed as failed in the catalogue);
  `oom` runs only in a process, under `prlimit`. A worker process does not outlive its host:
  when the host's end of the socket closes without a shutdown (the host died or was killed),
  the worker exits at once, even with its renderer hung, and a log write failing on the dead
  pipe cannot stop it (`a_worker_exits_when_its_host_is_killed_under_a_hang`: the host
  SIGKILLed under a hung worker whose watchdog is out of reach). The GUI's test
  (`gui_survives_faults`) runs one live session per isolation with a lane per moment x way,
  `knob` included (a knob's error is a refusal, an event of the lane), and a hung hot swap;
  what goes wrong while stopping is written to the session log.
- **The GUI's own GPU device** (eframe's, drawing the window and its torn-off panes; renderers
  and presenter windows have devices of their own, in their own processes by default). A lost
  surface is recreated by egui-wgpu and the frame skipped. An error on the device that is not
  its loss is shown in the toolbar's message and drawing goes on (wgpu's default made it a
  panic, ending the program). A lost device (driver reset, GPU removed or hung) cannot be
  survived in place: eframe builds its painter once with no way to swap the device under it,
  and egui-wgpu panics when it draws on a lost device. So the GUI hands over: its live session
  stops (workers end as at any stop), a plan with unsaved changes is written to
  `recovered-plan.toml` in the configuration directory, a fresh GUI starts on the plan (on a
  new device) and says in its message what happened and what was recovered, and the old one
  exits. The GUI notices the loss on its next frame, or egui-wgpu's panic does first (caught
  around eframe's loop; any other panic stays one); the hand-over is the same. Its live
  session and its lanes' state (knobs changed live, tuners) do not carry over; a thread-isolated lane
  that took the driver down with it is the thread's cost above. The GUI's test
  (`gui_device_lost_hands_over`) destroys the device both ways, in a headless compositor of
  its own (`kwin_wayland --virtual`, through its Xwayland).
- **Server and clients.** The host is the server. It owns the authoritative world and sends
  every worker one stream of `WorldUpdate`s: chunk loads and unloads, block edits, lights and
  the environment. In the worker, a gate on that stream (a `gate` node reading the world)
  serializes it, delays, reorders or loses its packets as a modelled network would and
  deserializes what arrives, and its `client` node keeps what has arrived and derives what
  its renderer holds (its radius and LOD levels). A `generator` node on the client's CPU
  makes the chunks there, so that only edits cross that gate.
- **The renderer owns its data.** It receives the client's updates and builds whatever
  structures it likes. Updates always arrive before the frame request they precede, so runs
  are deterministic. Scripted runs keep a simulated clock (`run.clock_fps`), so gate delays
  repeat too.
- **Asynchronous frames.** A renderer may return from `render` before the frame is done. Up
  to `run.in_flight` frames are outstanding at once. GPU renderers wake the worker through a
  `Waker` when work completes, so completion latency does not depend on polling.
- **Streaming.** Either everything in range is delivered before each frame
  (`streaming = "complete"`, deterministic), or there is a per-frame budget
  (`streaming = { budget = N }`: realistic pop-in, which shows up as lost quality). World
  generation finishes before timing starts.
- **Measurement.** The host measures latency (from Render sent to FrameDone received) and the
  interval between frame completions. Renderers report CPU time, GPU timestamps, per-stage
  times and any counters they like; workers report the bytes that arrived and the chunks
  still missing. The report also has the init time, the initial load time, the worker's peak
  memory and the GPU memory the renderer reports. GPU buffers start small and grow on demand
  up to their `*_mb` option, so the reported memory is what is actually used.

## Conventions every renderer follows (`mr-core`)

| What | Rule |
|---|---|
| Axes | Right-handed, +Y up. `yaw = 0` looks down −Z; positive yaw turns right. |
| Primary rays | One ray through each pixel centre (`Camera::ray_dir`). Row 0 is the top row. |
| Precision | The camera position is `f64`. Work relative to `Camera::split_position()`. |
| Chunks | 32³ cells, index `x + 32*(z + 32*y)`. A chunk that is not loaded is air. At LOD `L` a cell covers 2^L blocks. |
| Face UVs | `Face::uv`: viewed from outside with +Y up, `u` points right and `v` points down. Nearest sampling via `texel_coord`. |
| Unlit | Pixel = the sRGB texel of the first face hit; a miss is the sky colour. **Strict.** |
| Lit | `albedo * (ambient + sun + Σ point lights)` with hard shadows and windowed inverse-square falloff (`lighting::attenuation`), then `encode_srgb`. **Loose:** approximations are allowed, and scored. |
| Lights | Invisible: looking straight at one shows nothing. |
| Output | RGBA8 sRGB, top row first, tightly packed. |
| Lost data | Never dropped quietly. A holder that cannot keep what the frame needs (a buffer at its limit, a table out of slots, a failed read-back) fails the renderer with what was lost and the knob that would hold it: on a GPU through `Gpu::fail` (`GrowableBuffer` does it itself), checked as every frame starts and completes. The pass reports the error instead of a frame rate and a score. A component's error starts with its name (`clipmap: ...`); code serving a role whatever its component starts with the role (`storage: the voxel pool is full`, a buffer's label `geometry: quads`); or it names its node in backticks: the GUI marks that node (gui.md, "What the GUI shows"). |

The `debug` block (in the `showcase` scene) has a white marker in the `u = 0, v = 0` corner of
every face, so UV and orientation mistakes are obvious.

## Code layout

| Path | What |
|---|---|
| `src/` | the host: plans, sessions, reports, comparison, the catalogue, the tuner, the GUI (`src/gui/`; the pipeline editor in `src/gui/graph/`) |
| `crates/mr-core` | shared vocabulary: camera, chunks, faces and UVs, lighting, knobs, the `Renderer` trait, the client and the gates |
| `crates/mr-world` | generators, textures, the authoritative world store, the stream, scripted scenes (renderers never depend on it) |
| `crates/mr-ipc` | the protocol: messages, framing (Unix socket or Windows named pipe), shared-memory frame slots |
| `crates/mr-serve`, `mr-worker` | the worker side: one renderer driven over the protocol; the worker with its three entries: the `cpu` and `gpu` executors (each runs the region its graph picks) and the reference |
| `crates/mr-gpu` | wgpu plumbing: adapter choice, the frame ring (readback, timestamps), shader programs composed from modules, meshes |
| `crates/mr-mesh` | the greedy mesher and quad encoding |
| `crates/mr-components` | the registry (`lib.rs`: components, their implementations per device kind and API), the graph (`graph/`: model, checks, canonical form, key and label, accept with the executors' declarations (the regions), edits, the shipped graph files, schedule), capabilities and adapters (`caps.rs`), and one directory per component: `storage/prefix/{gpu.wgsl,cpu.rs}`, `traversal/sphere/...`, ... |
| `crates/mr-exec-gpu` | the GPU executor: the generic node executor (`exec.rs`), one core, and a region per program (ray casting, faces, discovery, splats, columns) whose nodes it schedules |
| `crates/mr-exec-cpu` | the CPU executor: the generic node executor (`exec.rs`) and its regions: ray casting, the software rasteriser, columns (what still differs from the [target architecture](target-architecture.md) is listed there) |
| `crates/mr-reference` | the reference renderer (f64, exact), standing alone on purpose |
| `tests/architecture.rs` | the architecture guard (below) |
| `plans/` | example plans |
| `pipelines/` | the shipped graph files (built into the binary; `graph = "<name>"` in a plan) |
| `baselines/` | full sweeps (observations and their catalogue); the newest is what `scripts/ab/guard.py` compares with. The one of 2026-10-11 is kept as data: it names its pipelines in a form this build no longer reads, so `catalogue` and the scripts leave its records out |
| `scripts/`, `install.py` | tools (Python); the installer |

## Adding a component

`scripts/new.py storage NAME "description"` does step 1 and the hand-kept lists of step 2 for a CPU
storage (a working copy of `bricks` under its name; `remove storage NAME` undoes it, `try NAME [--sweep]`
runs what it must pass, `accept-sweep` updates the full-sweep size the host tests state; `sites` lists the
edits). The steps below are what it automates and what stays yours. The recipe per kind of
technique (a storage, a shading, ...), with what each touched when it was last done, is
[adding-a-technique.md](adding-a-technique.md).

New techniques go in as components, not as new renderers:

1. Add a directory `crates/mr-components/src/<slot>/<name>/` with an implementation for each kind of
   device it supports (`gpu.wgsl` for the GPU executor, `cpu.rs` for the CPU executor).
2. Register it in `COMPONENTS` (`crates/mr-components/src/lib.rs`): its slot, its implementations per
   device kind and API, what it provides and what it needs from other stages. The
   `registry_is_consistent` test checks that the registry and the implementations agree.
   The regions (`graph::Builds`) take components by what they declare, not by name: a
   storage by its interface (`voxels`, or `heights` for the column pipelines), a walk or
   march by what it needs of its storage, a ray query by needing a geometry's acceleration
   structure (`accel`), a geometry by its form (`quads`, `cubes`, `accel`, `points`), meshes
   by where they go (`ray_limit` for a ray caster, `face_depth` for a rasteriser), drawers,
   shadings, prepasses and hole detection by being implemented on the node's device
   (`graph::Pick`). So registering a component with the right declaration makes it a choice in
   `Graph::choices`, sweeps (`component = "*"`), `list`, `audit`, the catalogue, the reference
   classes and the GUI; the host's test `contract` checks this per role with a component only
   it registers (`mr_components::register_on_this_thread`). The exception is visibility, the
   shape of a region (executor code of its own): its regions name it.
3. Declare its knobs (`ParamSpec::int/float/bool/choice`, owned by the component with
   `ParamSpec::of(slot, &[components])`). Knobs marked `live` are applied in
   `Renderer::set_param`. The GUI and the tuner pick them up automatically. A node whose component
   an executor can replace while running is declared in its region (`graph::Builds.live`) and
   replaced in the region's `switch`: the host sends the graph's edits (`Renderer::edit`), never a knob.
4. A diagnostic `view`: a component declares the per-pixel quantities it can write
   (`mr_core::Quantity`, a ramp or `Quantity::categorical`; the post node's `view` knob lists
   them, `mr_components::post::view_knobs`). On the GPU, while one is viewed the core binds a
   buffer of one `u32` per pixel (group 0, `view::QUANTITY_BINDING`), cleared before each
   frame; the writer's program fills it under its define, or the region's own view passes do
   after the frame's passes (`Region::view`: the GPU sampler's tiles, rays, category rasters of
   the drawn list and overdraw count, with atomics); the executor's view pass maps it into the
   output (a ramp through `mr_core::legend`'s ramp; a category as a tint of the greyed image in
   its colour, or one per id). On the CPU the executor keeps the values (`exec::Cx::view`,
   while one is viewed), the writing node (`Node::quantities`) fills them and the executor
   maps them into the image after the passes (ramps only so far). Either way the frame is
   still shaded and the executor reports the `Legend` in `FrameStats::legend`. Views not yet
   moved to this write their colours themselves (`Legend::color` on the CPU) and return the
   legend.

GPU code never opens a device: an executor's `create(params, devices, waker)` receives the
host's devices, and `mr_gpu::gpu(devices, slot, &GpuNeeds { .. })` returns one with the
features asked for. Asynchronous renderers call the `Waker` when work may have finished;
`mr_gpu::FrameRing` does this for you.

### What a component costs the build

`scripts/buildcost.py storage` builds the release binary, adds a CPU storage (`new.py storage`,
a copy of `bricks`), builds again, restores the tree and prints both; `walk` and `shading` do
the same with a probe (a copy of `incremental` matched at run time beside the other walks, a
copy of `direct` chosen at run time beside it: the code a real one adds, not a component);
`--time` adds the rebuild and `check.py` times (an idle machine only). Sizes measured
2026-11-24 (release `.text` 44.3 MB before), times 2026-11-26 on 21ad0f2 in the idle queue
(quiet, each job's own before and after; rebuild = the release build after the edit,
`check.py` = its wall time right after; a before took 113-117 s and 424-427 s):

| one more | binary | `.text` | functions | instantiated for it | rebuild | `check.py` |
|---|---|---|---|---|---|---|
| CPU storage | +6.1 MB (+2.2 %) | +1.8 MB (+4.2 %) | +340 (+0.7 %) | 305 | +13.1 s (+12 %) | build step +16 s; total 425 s, as the befores (tests -17 s, noise) |
| CPU raycast walk | +4.6 MB (+1.7 %) | +2.9 MB (+6.5 %) | +8 | 8 (the rest inlined) | +25.6 s (+22 %) | +36.5 s (+9 %; build step +35.5 s) |
| CPU shading | +0.2 MB (+0.1 %) | +0.17 MB (+0.4 %) | +50 | 41 | -0.7 s (noise) | +1.9 s (noise) |

CI's jobs on the same commit: `check` 22.6 min, `lavapipe` 11.0 min (recorded, `buildguard.py show`).

Why: the generic CPU build monomorphises each region per storage (`by_storage!` in
`mr-exec-cpu/src/raycast.rs`), and inside that the ray caster's tile loop
(`render_rect_*`) per ISA (scalar, AVX2, AVX-512) x pixel (`[u8; 4]`, `Half4`) x two const
flags: 24 instances of about 65 KB per storage, 1.6 MB of a storage's 1.8. Each instance holds
one copy of the tile loop per packet size (1, 4, 8, 16), and the walks are inlined into every
copy and matched at run time, so one more walk adds itself to storages x 24 x 4 loops: the
walk term grows with the product storages x walks (about 0.36 MB per storage and walk today).
The shading is called, not inlined: one more costs a fixed amount, per pixel type and spec
(41 functions). Compiled pipelines (`run.compile`) instantiate one combination and are not
affected.

**Coverage is the user's choice** (2026-11-24): what the precompiled (generic) build covers
spends the user's build time and disk, so it is a build setting, not a threshold of ours:
`MR_CPU_COVERAGE`, read by `crates/mr-exec-cpu/build.rs` (set it in the shell or, to keep it,
in `.cargo/config.toml`'s `[env]`; cargo rebuilds when it changes). Unset or `all`: everything
(every storage x walk x shading, every packet size, every ISA). Else space-separated
narrowings, each `dimension=kept,values`, of the dimensions a compiled pipeline
(`run.compile`) can supply on demand:

- `packets=4,8`: the ray packet sizes in the generic tile loop (of 4, 8, 16; 1, the `packet`
  knob's default, always stays). `packets=` alone keeps 1 only. Each kept size is one copy of every storage's tile loops with the walks
  inlined, so this divides the walk term.
- `isa=avx2`: the instruction sets dispatched to at run time (of avx2, avx512; scalar always
  stays, every CPU runs it). `isa=` alone keeps scalar only.

Storages, walks and shadings always stay, so every graph runs without a compiler. A graph or a
live knob change asking for what the build left out fails to build (the lane says which value,
the build's coverage and the two ways out), never runs as something else; `isa = auto` picks
the best covered ISA. A compiled pipeline bakes its own packet size and ISA and is unaffected.
`scripts/buildcost.py coverage SETTING` measures what a narrowing saves (built in
`target/coverage`); `check.py test` also runs `mr-exec-cpu`'s tests in a narrowed build.
Measured 2026-11-24: the narrowest, `packets= isa=` (packet 1, scalar), takes the release
binary from 272.4 to 252.2 MB and `.text` from 44.3 to 32.3 MB (-27 %), 170 functions fewer,
all in `mr-exec-cpu`.

**Tracking the build's figures.** `scripts/ab/buildguard.py record` appends a commit's figures
to `baselines/build.jsonl`: release binary and `.text` bytes and functions (from
`buildcost.py figures`), the rebuild after a component edit when `buildcost.py --time` measured
it, with `check.py`'s wall time right after that rebuild (total and per step: check.py writes
`target/logs/times.json` each run; a run's own time depends on what was built before it, so only
this defined start is tracked), and CI's job minutes
(`gh`, the newest successful run of the commit). `record --from JSON` records a
`buildcost.py` JSON made elsewhere (a timed run in a clone, as the idle queue makes them) under
its own commit. `buildguard.py` alone compares the tree with the
last record and prints every delta: sizes are flagged past `--pct` (default 5 %), times past
`--time-pct` (default 10 %) only when both records were taken on a quiet machine; CI times are
shown, never flagged (one tree's jobs took 15-24 and 8-16 minutes). `buildguard.py show` lists
the records.

**GPU shaders per graph.** A GPU graph's shaders are composed and compiled when its renderer
is built: each pass's report carries `shaders`, the part of `init_ms` spent making shader
modules (`module_ms`: naga_oil composition, naga validation and translation) and pipelines
(`pipeline_ms`: the driver's compile), with their counts (`mr_core::shader_clock`, per thread;
renderers create pipelines through `mr_gpu::Pipelines`). Two caches stand between a graph and
the driver's compile: ours (`$MR_GPU_CACHE`, a file per adapter and graph key) and the
driver's own (NVIDIA: `__GL_SHADER_DISK_CACHE`). Measured 2026-11-24 (NVIDIA, one run each,
not an idle machine; default graphs):

| graph | modules | pipelines | `module_ms` | `pipeline_ms` cached | `pipeline_ms` cold | `init_ms` cold |
|---|---|---|---|---|---|---|
| raycast-gpu | 2 | 3 | 22-25 | 1 | 117 | 324-348 |
| raster-gpu | 3 | 8 | 39-44 | 4 | 273-306 | 491-527 |
| splat-gpu | 1 | 6 | 16-20 | | 244-322 | 445-563 |
| discover-gpu | 6 | 26 | 86-113 | | 1837-3099 | 2107-3435 |

Cold, the driver's compile is most of a GPU build (87 % for discover-gpu); cached, it is a
few milliseconds and composition (ours) is the larger part. Each pipeline one more component
adds costs a driver compile once per graph key, then nothing.

## Verifying a change

- `cargo build --release` with no warnings; `cargo test -q --workspace` (unit tests,
  including: the registry matches its implementations, storages are exact, the CPU walks
  agree, every shipped plan loads).
- The toolchain is pinned by `rust-toolchain.toml` (rustup installs it on first use, with
  clippy, llvm-tools and the Windows targets); CI, the installer and the compiled pipelines
  use the same one, so lints and features agree everywhere. Bump it deliberately, with
  `scripts/check.py all` on the new version.
- GPU tests, `scripts/check.py gpu`: the host's end-to-end live sessions, the tuner and
  scale-curve demos (briefly: `SECONDS=8`), and `mr-gpu`'s device pool and growable buffer, all `#[ignore]`d so
  the workspace tests run without a GPU. Only `gui_screenshot` stays manual (it saves a
  picture to look at). It also runs `scripts/limits.py`: every shipped GPU graph with
  `MR_GPU_LIMITS=sizes`, on a small world (renders, the same pixels as unclamped) and a big
  one at 4K (renders or fails naming a `max_...` limit), so a new node cannot quietly assume a
  big binding; and `tests/scale.rs` (a world past a pipeline's size limit is refused naming it).
- Size limits, `scripts/scale.py` (by hand, minutes to hours): the view radius doubled per
  scenario plan over every pipeline, the radius at which each stops and why; the table is
  `docs/limits.md`.
- GPU tests in CI, `scripts/check.py lavapipe`: the ones that fit Mesa's software Vulkan (the
  device pool, the live sessions of raycast, meshes and the two-GPU split, the headless GUI) on lavapipe whatever GPU the machine has; CI's `lavapipe` job runs it on every push. A
  real small device: 128 MiB a binding and 48 storage buffers a stage, so the voxel pool is
  paged and a shader binding more than `voxels.rs`'s `OTHER_BUFFERS` besides the pool fails
  there. Not the HDR window (it needs a display), the compiled build, nor the timing demos.
- The architecture guard, `cargo test -q --test architecture`: no WGSL function defined twice
  across executors or redefining shared code, no bodies copied under other names, and
  lighting only through the shading component. Exceptions are listed in the test with their
  reasons.
- Speed: a composed pipeline must be as fast as a hand-written one of the same pieces, and
  existing pipelines keep their pixels and speed. The A/B scripts in `scripts/ab/` check
  both. `.cargo/config.toml` forces code alignment, because unaligned builds moved fps by
  5–8% from layout alone; A/B comparisons need both binaries built that way.
- CPU performance: pin to one CCD (`taskset -c 0-7,16-23`) and read the `raycast-cpu` preset's
  `perf = true` hardware counters (cycles, instructions and steps per pixel) rather than fps
  alone.
