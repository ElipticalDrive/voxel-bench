# Architecture

## Processes

```
multirenderer (host: the server)            worker process (one per renderer: a client)
├─ plan.rs      plans, -s overrides          ├─ mr-serve: IPC loop; link, client, generator
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
- **Server and clients.** The host is the server. It owns the authoritative world and sends
  every worker one stream of `WorldUpdate`s: chunk loads and unloads, block edits, lights and
  the environment. In the worker, the graph's `link` node delays, reorders or drops that
  stream as a modelled network would, and its `client` node keeps what has arrived and
  derives what its renderer holds (its radius and LOD levels). A `generator` node on the
  client's CPU makes the chunks there, so that only edits cross the link.
- **The renderer owns its data.** It receives the client's updates and builds whatever
  structures it likes. Updates always arrive before the frame request they precede, so runs
  are deterministic. Scripted runs keep a simulated clock (`run.clock_fps`), so link delays
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
| Lost data | Never dropped quietly. A holder that cannot keep what the frame needs (a buffer at its limit, a table out of slots, a failed read-back) fails the renderer with what was lost and the knob that would hold it: on a GPU through `Gpu::fail` (`GrowableBuffer` does it itself), checked as every frame starts and completes. The pass reports the error instead of a frame rate and a score. |

The `debug` block (in the `showcase` scene) has a white marker in the `u = 0, v = 0` corner of
every face, so UV and orientation mistakes are obvious.

## Code layout

| Path | What |
|---|---|
| `src/` | the host: plans, sessions, reports, comparison, the catalogue, the tuner, the GUI (`src/gui/`; the pipeline editor in `src/gui/graph/`) |
| `crates/mr-core` | shared vocabulary: camera, chunks, faces and UVs, lighting, knobs, the `Renderer` trait, the client and the link |
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

New techniques go in as components, not as new renderers:

1. Add a directory `crates/mr-components/src/<slot>/<name>/` with an implementation for each kind of
   device it supports (`gpu.wgsl` for the GPU executor, `cpu.rs` for the CPU executor).
2. Register it in `COMPONENTS` (`crates/mr-components/src/lib.rs`): its slot, its implementations per
   device kind and API, what it provides and what it needs from other stages. The
   `registry_is_consistent` test checks that the registry and the implementations agree.
   A storage needs nothing more: every region takes any storage with a voxel implementation
   on the device kind it runs the node on (`graph::Pick::Providing`), so registering it makes
   it a choice in `Graph::choices`, sweeps (`component = "*"`), `list`, `audit`, the
   catalogue, the reference classes and the GUI; the host's test `contract` checks this
   with a storage only it registers (`mr_components::register_on_this_thread`). The other
   roles' regions still name their components (`graph::Builds`): add it there too.
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
  big binding.
- GPU tests in CI, `scripts/check.py lavapipe`: the ones that fit Mesa's software Vulkan (the
  device pool, the live sessions of raycast, meshes, HDR and the two-GPU split, the headless
  GUI) on lavapipe whatever GPU the machine has; CI's `lavapipe` job runs it on every push. A
  real small device: 128 MiB a binding and 48 storage buffers a stage, so the voxel pool is
  paged and a shader binding more than `voxels.rs`'s `OTHER_BUFFERS` besides the pool fails
  there. Not the compiled build, nor the timing demos.
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
