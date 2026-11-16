# Pipelines

A renderer is a **pipeline graph**: nodes, each one a component in a role, connected from
the world to the image. No fixed list of stages exists. Any valid path from input to output
is a pipeline, and pipelines need not use the same kinds of component. `multirenderer list`
prints every shipped graph file with its label and every component by the role of the node it
fills (with the devices and APIs it is implemented for, and the files whose nodes may be it),
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
| `traversal` | rays | a ray-query service: a kernel walking a storage (`refloor`, `incremental`, `sphere`, `rowrun`, `rayquery`; `heightfield`, the heightmap's own march, casts the columns pipelines' shadow and bounce rays) |
| `prepass` | starts | where each primary ray may start (`beam`) |
| `visibility` | hits or faces | what is visible per pixel: `raycast`, `faces` (rasterise everything), discovery `quadtree` / `cluster`, `splat`, `columns` |
| `holes` | tiles | which screen tiles discovery looks at (`coverage`: only where something may have changed) |
| `draw` | hits | the shared face drawer: `raster`, `visbuffer` (face ids) or `gbuffer` (hit records) |
| `meshes` | surfaces | how triangle meshes join the image (`composite`, `depth`) |
| `shading` | image | `direct` (the reference lighting model) or `pathtrace` (diffuse path tracing with next-event estimation) |
| `post` | image | tone mapping: `clamp` (default), `reinhard`, `aces`; `hdr`: a float RGBA16F output (every executor, CPU ones included) in extended sRGB, unlimited, saved as `.hdr` beside the PNG, presented in HDR by `multirenderer view` |

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

## Executors and presets

A graph runs on the executor of one device kind: `executor = "gpu"` (the default,
`crates/mr-exec-gpu`) or `"cpu"` (`crates/mr-exec-cpu`), where the image is made; nodes without a
`device` run there when their component can. The graph's shape names a preset, the region of
that executor that builds it: `raycast-gpu`, `raster-gpu`, `rayquery-gpu`, `discover-gpu`,
`discover-cpu` (discovery on the CPU, drawn on the GPU), `splat-gpu`, `columns-gpu` on the GPU,
`raycast-cpu`, `raster-cpu`, `columns-cpu` on the CPU. A preset is named by technique and device. It is derived from the graph and never
chosen. A renderer is started from a graph file: one is shipped per region (`pipelines/*.toml`,
named alike), and the GUI's "+ renderer" and "start from…" list them. `cpu-reference`, which defines the reference images, is the one renderer that
stands alone, on purpose.

## Writing a pipeline

A pipeline is written as a graph: a graph file and the edits made to it (in a plan, or on
the command line), or the graph itself.

**A graph file and edits**: the graph of a file, changed step by step. The file is one
shipped with the bench, by name (`pipelines/*.toml`, built into the binary: one default
pipeline per executor region, e.g. `raycast-gpu`, `raster-cpu`, `discover-gpu`), or any
graph file by path (relative to the working directory). A shipped file is a graph like any
you save; nothing in the code knows it apart.

```toml
[[renderer]]
graph = "raycast-gpu"
edits = [
  { node = "storage", component = "tree64" },
  { node = "shading", component = "pathtrace" },
  { node = "shading", input = "bounce", from = { role = "traversal", component = "sphere" } },
  { node = "traversal", input = "storage", from = "storage" },
]
```

The edits are the pipeline editor's own changes, made in order, each exactly as written
(nothing filled in or pruned; a new node comes unbound):

| Edit | Does |
|---|---|
| `{ node, component }` | another component in the node; inputs the new one lacks, its knobs and API are dropped |
| `{ node, input, from = "<node>" }` | binds the input to an existing node |
| `{ node, input, from = { role, component } }` | binds it to a new node (id: the role's name, then `<role>-2`, ...) |
| `{ node, unbind = "<input>" }` | leaves the input unbound |
| `{ node, device }` | where the node runs (`cpu`, `gpu`, `gpu1`, ...; `"default"`: unset) |
| `{ node, api }` | how it runs there (`compute`, `rayquery`, `avx2`, ...; `"default"`: unset) |
| `{ node, input, when }` | when the input's value arrives: `"now"`, `"prev"`, `{ iterate = 2 }` |
| `{ node, knob, value }` | a knob on that node (no `value`: unset) |
| `{ node, knobs = { ... } }` | the node's knobs are these and no others |
| `{ add = "<id>", role, component }` | a new node under that id, unbound |
| `{ remove = "<id>" }` | the node gone, the inputs reading it unbound |

An edit naming a node or an input the graph does not have is refused with its place in the
list; whether the edited graph is a pipeline is decided as for any graph. A renderer read
this way is saved this way while its graph is still what the file and the edits give
(after a change in the editor it is saved by what it is). `edits` also follow an inline
`graph`.

**Command line** (`view -r`, `graph`): the same, `<graph file>[,<edit>,...]`, the edits
in short spellings or as the plan's inline tables:

```sh
-r raycast-gpu,storage=prefix,kernel=incremental
-r raster-gpu,geometry=cubes,draw=visbuffer
-r raycast-gpu,shading=pathtrace,shading.bounces=2
-r raycast-gpu,shading=pathtrace,shading.bounce=traversal:sphere,traversal.storage=storage
-r raycast-gpu,+client=client:derive,client.radius=4,client.max_lod=2,-meshes
-r 'raycast-gpu,{node="draw",input="faces",when="prev"}'
-r my-graphs/far-shadows.toml,shading@gpu1
```

| Short spelling | Edit |
|---|---|
| `node=component` | `{ node, component }` |
| `node.input=source` | `{ node, input, from }`: `source` an existing node, or a new one written `role:component`; empty: `{ node, unbind }` |
| `node.knob=value` | `{ node, knob, value }` (the value as TOML, else a string) |
| `node@device` | `{ node, device }` (`node@default`: unset) |
| `+id=role:component` | `{ add, role, component }` |
| `-node` | `{ remove }` |
| `{...}` | the table itself (APIs, timings, several knobs at once) |

Whether the name after a node's last dot is an input or a knob is read off the node's
component. Commas inside braces, brackets, parentheses and quotes belong to the edit. A
first part that is neither a shipped graph nor a file is a standalone renderer's name
(`-r cpu-reference`).

**Labels.** A renderer without a `label` of its own is labelled by its graph alone: the
executor's device, then its nodes, each after the nodes it reads (the main path first), as
its component, with `@device` where it runs elsewhere than on the executor's device, `:api`
where that is a choice off the default, and in parentheses its late inputs and its knobs:

```
gpu: bricks + refloor + composite + raycast + direct                 # pipelines/raycast-gpu.toml
gpu: greedy@cpu + faces + bricks + refloor + depth + raster + direct # pipelines/raster-gpu.toml
gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct
gpu: net(latency_ms=30.0) + derive(radius=4) + bricks + refloor + raycast + direct + aces
```

Left out are the nodes that say nothing: the world, the generator on the server, a `free`
link, a `pick` client without knobs, a `clamp` post. Options are not in the label (reports
show them beside it); two graphs that differ only in which of two like nodes an input reads
share a label (their keys differ: give one a `label`). Labels are for reading, not for
writing a pipeline: reports of older runs keep the labels they were written with
(`raycast-gpu: prefix + incremental`).

The flat form of older plans (`compose`, `sites`, a pipeline by `name`) is no longer read: a
pipeline is `graph` (and `edits`); `name` is for standalone renderers (`cpu-reference`).

The nodes every pipeline has without writing them (the client, the link, the generator, the
post) are changed by adding them: `{ add = "client", role = "client", component = "pick" }`
then `{ node = "client", knobs = { radius = 4, max_lod = 2 } }`; a `link` node with component
`net` and its knobs; a `generator` node (`procedural`) with `device = "cpu"` to make the world
on the client. The shipped plans (`plans/*.toml`) are written this way.

**Sweeps** are edits whose values are lists: `vary = [...]` after `graph` and `edits`
([plans.md](plans.md#sweeps)).

**Graph form**, for anything the others cannot express:

```toml
[[renderer]]
label = "split over two GPUs"
[renderer.graph]
executor = "gpu"
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

A graph the sites form can say is loaded and saved in that form, unless its stages carry
knobs: those stay on their nodes (each node keeps its own values), so it stays a graph. A
renderer's `options` set a knob on every stage it belongs to.

`multirenderer graph <renderer>` prints any renderer in this form, together with its sites
form, its canonical form, its cache key and its schedule: the passes in frame order, with the
nodes that are fused into each pass and the transfers between devices.

## Capabilities and adapters

An interface has a required core and optional capabilities (`crates/mr-components/src/caps.rs`): a
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
  constants (the knobs its components declare specialisable, read off the graph's nodes; on
  the CPU executor `exact`, `packet`, `skip*` and `shadows`), and built with `-C target-cpu=native`.
  The GPU executor's crate builds its CPU work natively and bakes no knob: its specialisable
  knobs (`max_steps`, `bounces`) are the shaders', compiled in by every build.
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

The cache stays bounded: a new build replaces older builds of the same pipeline (from
older sources, toolchain or CPU), a passed check's frames are deleted, and above 4 GB
(`MR_PIPELINE_CACHE_GB`) the least recently used workers go, then the shared target
directories. `multirenderer cache` shows what it holds; `multirenderer cache --clear`
empties it.

GPU pipelines also keep the driver's pipeline cache per adapter and graph
(`MR_GPU_CACHE=0` turns it off).

## Meshes

Scenes can contain animated triangle meshes (`scene.meshes`: box, sphere and torus with
vertex colours, scattered copies, and spin / orbit / bob animation; see `plans/meshes.toml`).
A pass mode combines lighting and meshes: `run.modes` takes `unlit`, `lit`, `unlit+meshes`
and `lit+meshes`, and the GUI has **Lit** and **Meshes** toggles.

Meshes and voxels occlude each other. A mesh pixel is shaded by the pipeline's shading
component like a voxel's, with the interpolated vertex colour as its albedo: `direct` lights
it with the same lights and shadow rays, `pathtrace` bounces off it (bounces land on voxels).
The `meshes` node's `shadow` port picks the rays that shadow meshes (unbound: the shading's).
Meshes do not cast shadows yet. Pipelines with a `meshes` node draw them: `depth` on the
rasterisers (the same depth buffer) and `composite` on raycast-gpu (meshes are rasterised
first, and rays stop at them). Pipelines without one are skipped in `+meshes` benchmark
passes, and the GUI labels them "(no meshes)".

## Devices

GPU pipelines pick the discrete GPU on hybrid laptops. `run.devices = { gpus = ["nvidia",
"amd"] }` (or a renderer's own `devices`) chooses the adapter for each slot by name;
`cpu_threads` sets the CPU's threads. With both slots on one adapter, two devices of it are
opened. `WGPU_BACKEND` chooses the graphics API. `MR_GPU_DEBUG=1` turns on wgpu debug labels
and validation (off by default: Vulkan's debug-utils path crashes intermittently when several
renderers share one process). Any wgpu validation error fails the renderer. `rayquery-gpu` needs
ray-query support (Vulkan `VK_KHR_ray_query`).
