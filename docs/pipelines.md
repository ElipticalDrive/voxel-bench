# Pipelines

A renderer is a **pipeline graph**: nodes, each one a component in a role, connected from
the world to the image. No fixed list of stages exists. Any valid path from input to output
is a pipeline, and pipelines need not use the same kinds of component. `multirenderer list`
prints every back end and every component (with the devices and APIs it is implemented for),
and `multirenderer graph <renderer>` prints a renderer as its graph.

## Nodes

| Role | Output | What it is |
|---|---|---|
| `generator` | chunks | makes chunks from the world's spec: on the server (default) or on the client's CPU, which then receives only edits |
| `world` | changes | the authoritative chunk store and its change log (the server) |
| `link` | changes | what the stream crosses: `free`, or `net(latency_ms, jitter_ms, bandwidth_mbps, loss, reorder, seed)`, a modelled network |
| `client` | changes | what the renderer holds: `pick` (the server's LOD levels) or `derive` (coarse levels built on the client), with `radius` and `max_lod` |
| `storage` | storage | a representation answering storage queries (`bricks`, `prefix`, `tree64`, ...) |
| `geometry` | faces | faces made from chunks (`greedy`, `naive`, `cubes`, `boxes`, `points`) |
| `traversal` | rays | a ray-query service: a kernel walking a storage (`refloor`, `incremental`, `sphere`, `rowrun`, `rayquery`) |
| `prepass` | starts | where each primary ray may start (`beam`) |
| `visibility` | hits or faces | what is visible per pixel: `raycast`, `faces` (rasterise everything), discovery `quadtree` / `cluster`, `splat`, `columns` |
| `holes` | tiles | which screen tiles discovery looks at (`coverage`: only where something may have changed) |
| `draw` | hits | the shared face drawer: `raster`, `visbuffer` (face ids) or `gbuffer` (hit records) |
| `meshes` | surfaces | how triangle meshes join the image (`composite`, `depth`) |
| `shading` | image | `direct` (the reference lighting model) or `pathtrace` (diffuse path tracing with next-event estimation) |
| `post` | image | tone mapping: `clamp` (default), `reinhard`, `aces` |

Every place where rays are cast picks its own traversal and storage: primary rays,
discovery rays, shadow rays, bounce rays and mesh shadows. These sites can share a node or
use different ones. For example, shadow rays can walk a `distance` storage with `sphere`
while primary rays walk `prefix` with `incremental`.

Each node also has:
- a **device**: a slot of the run's device configuration (`cpu`, `gpu`, `gpu1`, ...);
- an **API**: how it runs there (`compute`, `raster`, `rayquery` on a GPU; `scalar`, `avx2`,
  `avx512` on a CPU). A choice for which the component has no implementation is refused, with
  the reason;
- **timing** for each input: `now` (this frame), `prev` (the previous frame's value) or
  `iterate` (a loop within the frame). Cycles are allowed only through `prev` or `iterate`;
- **knobs**, the component's own options.

Data that is fused (a storage and the kernel walking it) runs with the node that calls it.
Any other edge may cross devices through a transfer, whose cost is known in advance. So one
frame can be split over two GPUs, for example with ray casting on one and deferred shading
on the other (`plans/two-gpus.toml`).

## Back ends

A back end (an *executor*) runs a family of graphs: `gpu-compute`, `gpu-raster`, `gpu-rt`,
`gpu-discover`, `gpu-splat`, `gpu-voxelspace` (on the GPU executor, `backends/gpu`), and `cpu`,
`cpu-raster`, `cpu-discover`, `cpu-voxelspace` (on the CPU executor, `backends/cpu`; the
discovery back end draws on the GPU). The back end is derived from the graph and never
chosen directly. Each back end has a default pipeline, and the GUI's "start from…" begins
from one. `cpu-reference`, which defines the reference images, is the one renderer that
stands alone, on purpose.

## Writing a pipeline

Three equivalent forms, from shortest to most general:

**Command line:** the back end's required slots in order, then optionally shading,
prepass and meshes; other sites after commas:

```sh
-r gpu-compute:prefix+incremental
-r gpu-raster:bricks+greedy+visbuffer
-r gpu-compute:bricks+refloor+pathtrace,bounce=tree64/sphere
-r gpu-compute:bricks+refloor,link=net(latency_ms=30,loss=0.01),client=derive(radius=4,max_lod=2)
```

**Compose form** in a plan, one field per slot; slots left out take the back end's default:

```toml
[[renderer]]
compose = { backend = "gpu-compute", storage = "prefix", traversal = "incremental", shading = "direct" }
sites = { shadow = "distance/sphere", link = "net(latency_ms=30, bandwidth_mbps=200)" }
options = { shadows = "sun" }
```

A site's value is another site (shared), a kernel (over the lead storage),
`<storage>/<kernel>`, or `<site>/<kernel>`.

**Graph form**, for anything the other two cannot express:

```toml
[[renderer]]
label = "split over two GPUs"
[renderer.graph]
executor = "gpu-compute"
nodes.world = { role = "world", component = "world" }
nodes.storage = { role = "storage", component = "bricks", inputs = { changes = "world" } }
nodes.kernel = { role = "traversal", component = "refloor", inputs = { storage = "storage" } }
nodes.visibility = { role = "visibility", component = "raycast", inputs = { primary = "kernel" } }
nodes."s.storage" = { role = "storage", component = "bricks", inputs = { changes = "world" } }
nodes."s.kernel" = { role = "traversal", component = "refloor", inputs = { storage = "s.storage" } }
nodes.shading = { role = "shading", component = "direct", device = "gpu1", knobs = { placement = "deferred" }, inputs = { hits = "visibility", shadow = "s.kernel" }, timing = { hits = "prev" } }
```

Here the shading runs on the second GPU, with a shadow walk of its own there, and it shades
the previous frame's hit records while the first GPU casts the current frame's rays.

`multirenderer graph <renderer>` prints any renderer in this form, together with its sites
form, its canonical form, its cache key and its schedule: the passes in frame order, with the
nodes that are fused into each pass and the transfers between devices.

## Capabilities and adapters

An interface has a required core and optional capabilities (`components/src/caps.rs`): a
storage always answers `s_probe` (the largest empty cube around a cell), and may also offer
`s_dist` (a distance field) or `s_row` (row bitmasks). When a component needs a capability
that its input lacks, an **adapter** provides it: compile-time code built from the core (for
example `sphere` over a storage without distances gets `s_dist` from `s_probe`). The result
is correct but slower, and the GUI notes it. A need that no adapter can meet makes the graph
invalid, and the GUI greys it out with the reason.

A feature costs only the pipelines that use it. Capabilities are compiled in where they are
chosen and absent elsewhere: there are no runtime flags in hot paths, and no wider records
or extra passes for pipelines that do not use them. On the GPU, each storage and kernel is a
WGSL module, composed into one shader per pipeline (naga_oil), so the split into components
costs nothing at run time.

## Pipelines compiled for this machine

`run.compile` (GUI: run settings → compile; a renderer's own `compile` overrides it):

- `off` (default): the generic build.
- `native`: a small crate is generated for the pipeline, with its graph and baked knobs as
  constants, and built with `-C target-cpu=native`.
- `break-in`: as `native`, then instrumented, trained on a short flight, and rebuilt with
  profile-guided optimisation and/or fat LTO; the fastest candidate is kept.

Compiled workers are cached in `~/.cache/multirenderer/pipelines` (Windows:
`%LOCALAPPDATA%\multirenderer\pipelines`, or `$MR_PIPELINE_CACHE`), keyed by the generated
source, the sources it builds against, the toolchain and the CPU. The first build fills a
shared target directory; later ones take seconds. Before a new build is used, it renders a
short terrain flight next to the generic build and must match it pixel for pixel
(`.trusted` / `.rejected` next to the worker). In the GUI the generic build runs while the
pipeline compiles, and the compiled one then hot-swaps in. Its label gains ` [native]` or
` [break-in]`. A baked knob cannot change live: changing it needs another build.

GPU pipelines also keep the driver's pipeline cache per adapter and graph
(`MR_GPU_CACHE=0` turns it off).

## Meshes

Scenes can contain animated triangle meshes (`scene.meshes`: box, sphere and torus with
vertex colours, scattered copies, and spin / orbit / bob animation; see `plans/meshes.toml`).
A pass mode combines lighting and meshes: `run.modes` takes `unlit`, `lit`, `unlit+meshes`
and `lit+meshes`, and the GUI has **Lit** and **Meshes** toggles.

Meshes and voxels occlude each other, and lit meshes are shadowed by voxels. Meshes do not
cast shadows yet. Pipelines with a `meshes` node draw them: `depth` on the rasterisers (the
same depth buffer) and `composite` on gpu-compute (meshes are rasterised first, and rays stop
at them). Pipelines without one are skipped in `+meshes` benchmark passes, and the GUI labels
them "(no meshes)".

## Devices

GPU pipelines pick the discrete GPU on hybrid laptops. `run.devices = { gpus = ["nvidia",
"amd"] }` (or a renderer's own `devices`) chooses the adapter for each slot by name;
`cpu_threads` sets the CPU's threads. With both slots on one adapter, two devices of it are
opened. `WGPU_BACKEND` chooses the graphics API. `MR_GPU_DEBUG=1` turns on wgpu debug labels
and validation (off by default: Vulkan's debug-utils path crashes intermittently when several
renderers share one process). Any wgpu validation error fails the renderer. `gpu-rt` needs
ray-query support (Vulkan `VK_KHR_ray_query`).
