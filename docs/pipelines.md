# Pipelines

A renderer is a **pipeline graph**: nodes, each one a component in a role, connected from
the world to the image. No fixed list of stages exists. Any valid path from input to output
is a pipeline, and pipelines need not use the same kinds of component. `multirenderer list`
prints every shipped graph file with its label and every component by the role of the node it
fills (with the devices and APIs it is implemented for, and the files whose nodes may be it),
`multirenderer audit` tabulates, per component and device kind, the APIs it is implemented with
and the shipped files (or graphs one node-adding or -removing edit from one) that accept it there,
then lists the implementations no such graph reaches and the components implemented on one kind
only: gaps not filled yet, and those refused on a kind by their nature (`REFUSED` in
`mr-components`, with the reason, which `accept` also gives), and `multirenderer graph <renderer>` prints a renderer as its graph.

## Nodes

| Role | Output | What it is |
|---|---|---|
| `generator` | chunks | makes chunks from the world's spec: on the server (default) or on the client's CPU, which then receives only edits |
| `world` | changes | the authoritative chunk store and its change log (the server) |
| `client` | changes | what the renderer holds: `pick` (the server's LOD levels) or `derive` (coarse levels built on the client), with `radius` and `max_lod` |
| `storage` | storage | a representation answering storage queries (`bricks`, `prefix`, `tree64`, ...) |
| `geometry` | faces | faces made from chunks (`greedy`, `naive`, `cubes`, `boxes`, `points`) |
| `traversal` | rays | a ray-query service: a kernel walking a storage (`refloor`, `incremental`, `sphere`, `rowrun`, `topdown`, `rayquery`; `heightfield`, the heightmap's own march, casts the columns pipelines' shadow and bounce rays) |
| `prepass` | starts | where each primary ray may start (`beam`) |
| `visibility` | hits or faces | what is visible per pixel: `raycast`, `faces` (rasterise everything), discovery `quadtree` / `cluster`, `splat`, `slices` (plane sweeping: every integer plane rasterised, fragments kept on block faces by two storage lookups), `columns` |
| `holes` | tiles | which screen tiles discovery looks at (`coverage`: only where something may have changed) |
| `draw` | hits | the shared face drawer: `raster`, `visbuffer` (face ids) or `gbuffer` (hit records) |
| `meshes` | surfaces | how triangle meshes join the image (`composite`, `depth`) |
| `shading` | image | `direct` (the reference lighting model), `pathtrace` (diffuse path tracing with next-event estimation) or `ao` (direct light with ray-traced ambient occlusion within `reach` blocks) |
| `gate` | what it reads | a gap between two domains on an edge: `net(latency_ms, jitter_ms, bandwidth_mbps, loss, reorder, resend, mtu, seed)` ([Gates](#gates)) |
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
frame can be split over two GPUs at the hit records, with the visibility (ray casting with
deferred shading, a `gbuffer` or `visbuffer` drawer, splats) on one and the shading on the
other (`plans/two-gpus.toml`).

## Executors and presets

A graph runs on the executor of one device kind: `executor = "gpu"` (the default,
`crates/mr-exec-gpu`) or `"cpu"` (`crates/mr-exec-cpu`), where the image is made; nodes without a
`device` run there when their component can. The graph's shape names a preset, the region of
that executor that builds it: `raycast-gpu`, `raster-gpu`, `rayquery-gpu`, `discover-gpu`,
`discover-cpu` (discovery on the CPU, drawn on the GPU), `splat-gpu`, `slices-gpu`, `columns-gpu` on the GPU,
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
list; whether the edited graph is a pipeline is decided as for any graph. A knob is set on
the node that owns it: one its executor does not declare for that node's role and component
(an unknown name, another node's knob, a pipeline knob that belongs in `options`) is refused
naming node and knob, with the node's own knobs; the world and the generator have none. A renderer read
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
gpu: derive(radius=4) + bricks + refloor + raycast + direct + aces
```

Left out are the nodes that say nothing: the world, the generator on the server, a `pick`
client without knobs, a `clamp` post (gates follow the label: `| gate ...`). Options are not in the label (reports
show them beside it); two graphs that differ only in which of two like nodes an input reads
share a label (their keys differ: give one a `label`). Labels are for reading, not for
writing a pipeline: reports of older runs keep the labels they were written with
(`raycast-gpu: prefix + incremental`).

The flat form of older plans (`compose`, `sites`, a pipeline by `name`) is no longer read: a
pipeline is `graph` (and `edits`); `name` is for standalone renderers (`cpu-reference`).

The nodes every pipeline has without writing them (the client, the generator, the post) are
changed by adding them: `{ add = "client", role = "client", component = "pick" }` then
`{ node = "client", knobs = { radius = 4, max_lod = 2 } }`; a `generator` node
(`procedural`) with `device = "cpu"` to make the world on the client. The network between
server and client is a gate reading the world ([Gates](#gates)). The shipped plans (`plans/*.toml`) are written this way.

### Gates

A `gate` node sits on an edge: it reads one node (`in`) and its reader reads it instead, so
any number of gates go on any edge, several in a row on one. What crosses a gate is
serialized (`mr_core::gate::Wire`), the bytes cross a modelled network in packets of `mtu`
bytes (`latency_ms`, `jitter_ms` per packet, `bandwidth_mbps`, `loss` per packet, `reorder`:
messages overtake each other, else in order) and are deserialized on the far side; each gate
in a row is a domain of its own (serialized and deserialized again). The serialization's
cost is part of what it simulates. A lost packet's bytes arrive as zeros, or with `resend`
after a timeout (twice the latency plus the jitter, at least 10 ms); the data is whatever the
bytes say, and a message that does not decode is lost (counted). Nothing guards a type
against loss.

```toml
edits = [
  { add = "gap", role = "gate", component = "net" },
  { node = "gap", input = "in", from = "world" },
  { node = "gap", knobs = { latency_ms = 30, loss = 0.01, resend = true } },
  { node = "storage", input = "changes", from = "gap" },
]
```

The checked graph keeps gates apart from its nodes (`Checked::gates`, per edge), so every
executor builds the graph as it would without them; keys and labels name them
(`| gate bricks.changes (latency_ms=30.0, ...)`), and a graph without gates keeps its key.
A chain of gates no node reads sits behind the graph's output: the frame crosses it (it must
read the output; `| gate output.frame (...)`).
Refused, with the reason: a gate on a storage or ray-query edge (compiled into the caller,
nothing crosses), on a GPU device, two chains no node reads, one behind something other than
the output; by the executors, on an edge with timing `iterate`, or `prev` where the executor
does not hand the previous frame's value over itself (it does on the CPU discoverer's faces and
on the hit records a split's second GPU shades). Where they run:

| edge | run by | message | until it arrives |
|---|---|---|---|
| the world's changes into the client (gates reading the world, whoever reads them: the world's changes reach a renderer only through its client; the server-to-client network) | the worker, in front of the client (every such edge carries the same gates: one stream) | each change (edits split per chunk); with `reorder` chunks overtake each other, each chunk's own changes (and the lights and environment) stay in order | the client does not know it: chunks loaded into the gates and not out of them are holes, those on the way and those lost (without `resend` a load whose bytes do not decode never arrives: a hole until a later load of the chunk arrives or it is unloaded) |
| the generated chunks into the client (`client.chunks`, the generator on the client's cpu; on the server the edge is the host's, refused) | the worker, between its generator and its client (with the edits it merged) | each change | holes, as above |
| the client's changes into the renderer (gates reading the client) | the worker, between its client and its renderer (every such edge the same gates: the renderer takes one stream) | each change | the renderer does not hold it yet: holes |
| hits (`shading.hits`) | the CPU executor, a pass between visibility and shading (`placement = "deferred"`; the ray caster's records, the column marcher's and the rasteriser's) | each frame's records, with its frame id (the ray caster's with their layers when see-through blocks are drawn: eight records a pixel) | the shading reads the last records that arrived at this size (no hit before any has); it and the passes after it run with the camera of the frame they were cast in (its own frame's when the id was lost) |
| hits (`shading.hits`) on a GPU | the GPU executor, as a split at the hit records (the shading's own executor, on the same GPU or another, with its own copy of the storages its rays walk): read back, through the gates on the host, written to the shading's edge (`placement = "deferred"`, a drawer writing records, or splats; no meshes node) | each frame's records, with its frame id, size and records per pixel (the ray caster's with their layers when see-through blocks are drawn: eight a pixel, composited past the gate) | the shading reads the last records that arrived at this size (sky before any has), with the camera of the frame they were cast in (its own frame's when the id was lost) |
| faces into the CPU rasteriser's visibility (`visibility.faces`: the geometry's meshes) and drawer (`draw.faces`: the screen triangles) | the CPU executor, a pass between the two | each frame's meshes of the world (not the chunks the geometry remeshes from), or the frame's triangles with the visible chunks' quads they index, its size and frame id | the reader reads the last that arrived (none before; triangles only at this size); the meshes are the same from every camera, so the passes after them keep this frame's camera, while late triangles are drawn and shaded with the camera of their frame |
| faces into the CPU discoverer's drawer (`draw.faces`, discovery a frame ahead with timing `prev`, or within the frame) | the GPU executor, on the host between discovery and the upload to the GPU that draws them | each frame's whole face set (anchor, records, the view's tags) with its frame id | the drawer draws the last set that arrived (none before; only the records that differ from what it holds are uploaded); the faces are in world space, drawn from this frame's camera |
| faces into the GPU rasteriser's visibility (`visibility.faces`: the geometry's faces, meshed on the CPU) | the GPU executor, on the host between the geometry and the per-chunk upload into the quad buffers | each chunk remeshed or dropped: its position, slot, per-face counts and quads (one ordered stream per chunk; counts that do not add up to the quads fail to decode) | the GPU keeps the chunk's old faces (none for a new chunk) until a message arrives; a lost one keeps them until the chunk is next remeshed; with `reorder`, a slot freed and taken by another chunk can draw the older chunk's faces at the newer one's position until its drop arrives. The faces are in world space, drawn from this frame's camera |
| culled faces into the GPU rasteriser's drawer (`draw.faces`: the CPU culling's draws) | the GPU executor, on the host between the culling and the upload of the indirect draws | each frame's draws and their batches (page of the quads, first draw, draws), with its frame id (a batch past the draws fails to decode) | the drawer draws the last list that arrived (nothing before one has; batches on pages the quads no longer have are left out). The draws index the GPU's quads, so a late list draws what the quads hold now, culled with its own frame's camera and drawn from this frame's |
| points into the splats' visibility (`visibility.points`: the geometry's points, built on the CPU) | the GPU executor, on the host between the geometry's rebuild and the array the points' buffer mirrors | each chunk whose points changed: its position and points (one ordered stream per chunk; a neighbour rebuilt to the same points is not sent again) | the GPU keeps the chunk's old points (none for a new chunk) until a message arrives; a lost one keeps them until the chunk's points next change. The points are in world space, splatted from this frame's camera |
| geometry into ray queries (`kernel.geometry`: the geometry's quads or boxes, built on the CPU) | the GPU executor, on the host between the meshing and the BLAS build (the build budget applies to what arrived) | each chunk whose primitives changed: its position and its quads (`greedy`, `naive`) or occupied bricks (`boxes`), from which the BLAS input is derived on arrival (one ordered stream per chunk; a neighbour rebuilt to the same primitives is not sent again). One set of acceleration structures serves every ray query over the geometry, so every edge out of it must carry the same gates | the GPU keeps the chunk's old BLAS (none for a new chunk) until a message arrives; a lost one keeps it until the chunk is next rebuilt. `boxes` read the voxels of the current world inside their bricks, so late bricks and new voxels can disagree. Traced from this frame's camera |
| between two passes of one GPU executor: the GPU column marcher's hits (`shading.hits`, `placement = "deferred"`), the beam prepass's ray starts into the walk (`visibility.start`), hole detection's tiles into GPU discovery (`visibility.where`), GPU discovery's faces into its drawer (`draw.faces`), GPU ray casting's mesh surfaces (`visibility.meshes`: the record target the meshes' pass rasterises, each pixel's mesh hit and its distance, where the rays stop; the triangles they index stay the GPU's, placed for the current frame) | the GPU executor, before the reader's pass: what the passes before it left in the edge's resources (buffers, render targets) is read back (the frame's commands so far submitted and waited for), sent through the gates on the host, and the last that arrived written in place; after the reader's pass what the producer left is copied back, so state it keeps across frames (the face set, the tiles' history) stays its own | each frame's resources' bytes, with its frame id and size | the reader reads zeros (no hit: sky; no start, tile or face); the hits, starts, mesh surfaces and tiles (seen from a camera) are read, and the passes after them run, with the camera of the frame they were made in |
| the frame (behind the output) | the worker, for every executor (read back even when the host asked for no pixels: a GPU pays that) | each frame's pixels | the host gets the last frame that arrived at this size (black before any has) |

The CPU rasteriser's hits are its quad ids with the visible chunks' quads they index (a
message whose ids do not index its quads fails to decode). Refused by the executors so far,
naming the gate and the edge: hits
with `placement = inline` or of a forward drawer (they never leave the pass that finds them),
hits beside mesh surfaces on a GPU (the shading past the gate has not their triangles), an image into the post (the post is compiled into its shading,
no image crosses: put the gate after the post), mesh surfaces into a GPU drawer (rasterised in the
drawer's own pass beside its faces: no edge resource to carry), edges out of ray queries' geometry with different gates (one set of acceleration
structures serves them all).
Reports carry `gates`, one entry per gate node: `gate` (its id), `edge`
(`reader.port`; the world's side: `client.changes` into the client, `client.chunks`,
`changes` from the client into the renderer; `frame`) and `stats` (messages, bytes, packets, packets lost,
messages that failed to decode, `encode_ms` / `decode_ms`), so a gate's serialization time
and bytes are its own. Serialized forms: the world's changes, frames (`gate::Frame`), hit
records (the CPU executor's `Crossing`; the GPU's `transfer::Records`), what a GPU executor holds between two passes (`cross::Crossed`: each resource's bytes), the GPU rasteriser's chunk faces (`faces::mesh::ChunkFaces`) and culled draws (`faces::mesh::DrawList`), the splats' chunk points (`splat::ChunkPoints`), ray queries' chunk geometry (`accel::ChunkGeometry`), plain records
(`gate::encode_pod`).

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

A renderer's `options` set a knob on every stage it belongs to; knobs written on a node stay
that node's own.

`multirenderer graph <renderer>` prints any renderer in this form, together with its label,
its canonical form, its cache key and its schedule: the passes in frame order, with the
nodes that are fused into each pass and the transfers between devices.

The canonical form writes only what makes a pipeline differ, so keys stay comparable as the
bench grows: a knob a node sets to its default (`crates/mr-components/src/knobs.rs`, kept equal
to the executors' knobs by a test) is dropped, and so is a node that is its role's identity
(`graph::identity`: the client `pick`, the generator `procedural` on the server, the post
`clamp`) and sets nothing else. A component or a knob (with a default) added
later leaves every existing key alone (`src/contract.rs` probes it).

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

A big GPU can stand in for a small one: `MR_GPU_LIMITS=sizes` opens every device with its
buffer, binding and texture sizes clamped to WebGPU's baseline (128 MiB a binding, 256 MiB a
buffer, 8192 texels; lavapipe's), `MR_GPU_LIMITS=webgpu` clamps every limit (8 storage buffers a
stage, which no shipped GPU pipeline fits yet). A buffer over a limit fails the pipeline naming
the buffer, the device, the limit and what sets its size (a knob, the world, the resolution);
the voxel pool instead spans several bindings.

## Meshes

Scenes can contain animated triangle meshes (`scene.meshes`: box, sphere and torus with
vertex colours, scattered copies, and spin / orbit / bob animation; see `plans/meshes.toml`).
A pass mode combines lighting and meshes: `run.modes` takes `unlit`, `lit`, `unlit+meshes`
and `lit+meshes`, and the GUI has **Lit** and **Meshes** toggles.

Meshes and voxels occlude each other. A mesh pixel is shaded by the pipeline's shading
component like a voxel's, with the interpolated vertex colour as its albedo: `direct` lights
it with the same lights and shadow rays, `pathtrace` bounces off it.
The `meshes` node's `shadow` port picks the rays that shadow meshes (unbound: the shading's).
Meshes are also geometry of the lighting model: they cast shadows (on voxels and each other)
and bounces land on them, in the reference and in the GPU pipelines whose shading pass has
the triangles bound (raycast-gpu, raster-gpu with a deferred drawer: `visbuffer`,
`gbuffer`; the triangles' BVH is rebuilt on the CPU when meshes move). raster-gpu's forward
drawer (`raster`) and rayquery-gpu do not yet; a pipeline is scored against the full model. Pipelines with a `meshes` node draw them: `depth` on the
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
