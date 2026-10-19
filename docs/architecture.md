# Architecture

## Processes

```
multirenderer (host: the server)            worker process (one per renderer: a client)
├─ plan.rs      plans, -s overrides          ├─ mr-serve: IPC loop; link, client, generator
├─ session.rs   frame loop, pacing, outputs  │    └─ the back end running the pipeline graph
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

The `debug` block (in the `showcase` scene) has a white marker in the `u = 0, v = 0` corner of
every face, so UV and orientation mistakes are obvious.

## Code layout

| Path | What |
|---|---|
| `src/` | the host: plans, sessions, reports, comparison, the catalogue, the tuner, the GUI (`src/gui/`; the pipeline editor in `src/gui/graph/`) |
| `crates/mr-core` | shared vocabulary: camera, chunks, faces and UVs, lighting, knobs, the `Renderer` trait, the client and the link |
| `crates/mr-world` | generators, textures, the authoritative world store, the stream, scripted scenes (renderers never depend on it) |
| `crates/mr-ipc` | the protocol: messages, framing (Unix socket or Windows named pipe), shared-memory frame slots |
| `crates/mr-serve`, `mr-worker` | the worker side: one renderer driven over the protocol; the worker with every back end |
| `crates/mr-gpu` | wgpu plumbing: adapter choice, the frame ring (readback, timestamps), shader programs composed from modules, meshes |
| `crates/mr-mesh` | the greedy mesher and quad encoding |
| `components/` | the registry (`lib.rs`: back ends, components, their implementations), the graph (`graph.rs`: model, checks, canonical form and key, schedule), capabilities and adapters (`caps.rs`), and one directory per component: `storage/prefix/{gpu.wgsl,cpu.rs}`, `traversal/sphere/...`, ... |
| `backends/gpu` | the GPU executor: one core and a lane per kind of visibility (ray casting, faces, discovery, splats, columns) |
| `backends/cpu` | the CPU executor: ray casting, the software rasteriser, columns |
| `renderers/cpu-reference` | the reference renderer (f64, exact), standing alone on purpose |
| `tests/architecture.rs` | the architecture guard (below) |
| `plans/` | example plans |
| `baselines/` | a full sweep of 2026-10-11 (observations), for comparing later sweeps with `scripts/ab/sweepcmp.py` |
| `scripts/`, `install.py` | tools (Python); the installer |

## Adding a component

New techniques go in as components, not as new renderers:

1. Add a directory `components/src/<slot>/<name>/` with an implementation for each kind of
   device it supports (`gpu.wgsl` for the GPU executor, `cpu.rs` for the CPU executor).
2. Register it in `COMPONENTS` (`components/src/lib.rs`): its slot, its implementations per
   device kind and API, what it provides and what it needs from other stages. The
   `registry_is_consistent` test checks that the registry and the implementations agree.
3. Declare its knobs (`ParamSpec::int/float/bool/choice`, owned by the component with
   `ParamSpec::of(slot, &[components])`). Knobs marked `live` are applied in
   `Renderer::set_param`. The GUI and the tuner pick them up automatically.
4. A diagnostic `view` writes its colours through `mr_core::legend` (`ramp_wgsl()` in a
   shader, `Legend::color` on the CPU) and returns the matching `Legend` in
   `FrameStats::legend`, so that the GUI can draw the scale and read values back.
5. Add or extend its entry in the encyclopedia (`docs/schemes/`, then
   `python3 scripts/schemes.py`); `cargo test` fails while a component has none.

GPU code never opens a device: a back end's `create(params, devices, waker)` receives the
host's devices, and `mr_gpu::gpu(devices, slot, &GpuNeeds { .. })` returns one with the
features asked for. Asynchronous renderers call the `Waker` when work may have finished;
`mr_gpu::FrameRing` does this for you.

## Verifying a change

- `cargo build --release` with no warnings; `cargo test -q --workspace` (unit tests,
  including: the registry matches its implementations, storages are exact, the CPU walks
  agree, every shipped plan loads).
- GPU end-to-end tests: `cargo test -q --release --bin multirenderer -- --ignored --skip
  tuner_demo --skip scale_curve --skip gui_screenshot`.
- The architecture guard, `cargo test -q --test architecture`: no WGSL function defined twice
  across back ends or redefining shared code, no bodies copied under other names, and
  lighting only through the shading component. Exceptions are listed in the test with their
  reasons.
- Speed: a composed pipeline must be as fast as a hand-written one of the same pieces, and
  existing pipelines keep their pixels and speed. The A/B scripts in `scripts/ab/` check
  both. `.cargo/config.toml` forces code alignment, because unaligned builds moved fps by
  5–8% from layout alone; A/B comparisons need both binaries built that way.
- CPU performance: pin to one CCD (`taskset -c 0-7,16-23`) and read the `cpu` back end's
  `perf = true` hardware counters (cycles, instructions and steps per pixel) rather than fps
  alone.
