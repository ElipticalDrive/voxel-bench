# Target architecture

What the bench's architecture is meant to be. It is **not** fully what the code does today:
[design.md](design.md), [pipelines.md](pipelines.md) and [architecture.md](architecture.md)
describe what exists. The gap is stated in [Where the code differs](#where-the-code-differs).

## Principles

1. **Renderers are constructed from components, and components are reused.** A renderer is
   nothing but a choice and wiring of components.
2. **The exact pipeline runs at native speed.** A composed pipeline runs nearly, if not
   exactly, as fast as the same pipeline written by hand. A pipeline that is slow because of
   what was asked for (a poor storage for its kernel, a costly shading model) is accepted
   as slow; slowness that comes from the composition itself is a bug.
3. **Interfaces between components are typed and at least somewhat generic.** Any output may
   feed any input of the same type.
4. **Everything else is the user's creativity.** No fixed list of stages, no set of blessed
   pipeline shapes. Any valid graph from the world to the image is a pipeline.

Priority when they conflict: *any edge, any pipeline* outranks tidy composition rules; native
speed bounds how the composition is done, not what may be composed.

## Vocabulary

| Term | Meaning |
|---|---|
| **Component** | A reusable unit with typed input ports, one typed output, knobs, and one implementation per (device kind, API) it supports. |
| **Node** | An instance of a component in a graph, with its inputs bound, its knobs set, a device and an API. |
| **Edge** | A binding of one node's output to another node's input, with a timing (`now`, `prev`, `iterate = n`). |
| **Graph** | Nodes and edges. The *whole* description of a renderer. |
| **Device** | A slot of the run's device configuration (`cpu`, `gpu`, `gpu1`, ...). The host maps slots to hardware; components hold no hardware opinion. |
| **API** | The software the program uses to speak to the device: compute, raster, ray queries on a GPU; scalar, AVX2, AVX-512 on a CPU. |
| **Back end** | A (device, API) pairing, and nothing more. It is not a structure of execution and does not constrain the graph's shape. |
| **Fused region** | Nodes compiled together into one program on one device, where per-step cost lives (a storage, the kernel walking it, inline shading). |
| **Pass** | One unit the executor schedules: a fused region, or a node that stands alone. Data crosses pass boundaries only through edge resources. |

## The graph

- **Nodes are typed by their ports.** Port types are the data products: world changes, chunks,
  storages, ray services, faces, starts, hits (the common hit record), tiles, images. The
  typing is the single source of truth for what may connect to what.
  **Port types are added as needed**: as many as the techniques require, including ones only a
  single technique uses. The set is open and is not kept small for its own sake.
  Where a port carries several kinds of data at once (today's storage-and-rays port, for
  example), splitting it into separate ports can let other components share the pieces and
  removes duplicated code; that is done where it pays and is not always applicable.
- **Edges are free.** Any output of type *T* may be bound to any input accepting *T*. The editor
  lets the user add a node and wire it however they like. Nothing is dropped, rewritten or
  re-filled behind their back.
- **Validity is a diagnosis, not a gate on editing.** A graph is runnable when:
  1. every input is bound to an output of a compatible type (or is optional);
  2. cycles pass only through `prev` or `iterate` edges;
  3. every node has an implementation for its device and API;
  4. cross-device edges have a transfer (inserted automatically; its cost is known);

  A graph failing any of these is still a graph: it is shown with the reason attached to the
  node or edge responsible, and does not run. Nothing in validity refers to which back end,
  lane or preset the graph resembles.
- **Fusion is attempted, not pre-judged.** Whether the fused regions a graph asks for can be
  composed is not a validity rule: running the graph tries it, and a failure is reported to
  the user with its reason (a compile error, a missing capability). A fusion that succeeds but
  gives a wrong image is the user's pipeline to debug; the exact reference and the debug views
  are the tools for that.
- **Ray-casting sites are inputs like any other.** Every place rays are cast (primary,
  discovery, shadow, bounce, mesh shadows) is an input port taking a ray service, bound to
  whichever traversal-and-storage the user wants there.
- **State flows both ways.** `prev` and `iterate` edges make history, feedback and loops part
  of the graph, not special cases.
- **Not every graph can be verified by enumeration.** Correctness rests on per-port contracts
  and the exact reference renderer; graphs are sampled, not enumerated.

## Components

A component declares, in data:
- its input ports and output port (types, optionality, whether an input casts rays);
- its knobs (type, range, baked or live);
- for each (device kind, API) it supports: an implementation, and the capabilities it
  provides (for storages: which queries it answers fast);
- adapters: code that gives a method a feature it lacks, even slowly, so good pairings are
  fast and bad ones work but are slow.

The registry is the only place that knows what components exist. Neither the executor nor the
editor names a component.

**New techniques work with the rest of the graph.** A technique is implemented so that it
accepts the ports and data the other components produce. Where that can only be done in part,
that is acceptable: the unsupported combinations are declared by the component and make those
edges invalid, with the reason. Full support can be front-loaded when it would take too long
at first, and widened later without changing any graph that already works.

## Execution

The graph is executed by **one generic executor per device**, driven by the graph's schedule.
There are no per-strategy executors.

- **Schedule.** From the graph: an ordered list of passes and loops (`iterate`), the transfers
  between devices, and what is kept for the next frame (`prev`). Computed once per graph, not
  per frame.
- **Node interface.** Each implementation, for its (device, API), declares the resources it
  needs and provides two operations: *update* (apply the world changes it receives) and
  *encode/run* (do its work for this frame, reading its input edges' resources and writing
  its output's). It owns its internal resources; edge resources belong to the executor.
- **Edge resources.** The executor allocates each edge's resource, sized and formatted to
  exactly what the graph needs (see [Native speed](#native-speed)). Same-device edges stay on
  the device; cross-device edges go through generated transfer passes (readback, upload).
- **The common hit record** stays the boundary after visibility, so one shading
  implementation per kind of device serves every visibility strategy, and surface sources
  (meshes) write into the same record.
- **Isolation is unchanged.** One worker process per renderer; the host streams the world to
  it; the worker's gates and client node shape that stream. These are graph nodes already.

## Native speed

Composition cost happens before the pipeline runs, never while it runs.

**Fused regions.** Inside a region, composition is by source, and the platform's compiler
optimises across the boundaries:
- GPU: shader modules are composed into one shader per region before compilation
  (`naga_oil`), so the driver sees one program. Choices that vary per pipeline (record
  format, lighting on or off, LOD, step limits) are compile-time constants of that program,
  never uniforms in hot paths.
- CPU: components are generics monomorphised per combination. A region is generated Rust,
  built with `-C target-cpu=native`; inside it, no dynamic dispatch and no runtime-sized
  records.

**Region boundaries.** A region's boundary is where a hand-written version of the same
pipeline would also have a pass boundary: where the hit record crosses between visibility and
shading when shading is deferred, where history crosses frames, where data crosses devices.
The cost of a boundary is therefore the cost the hand-written pipeline pays.

**Where generic dispatch is allowed.** Only at pass level: tens of dispatches per frame on a
GPU, whole tile-passes on a CPU. Never per ray, per step or per pixel.

**The graph-to-program generator.** The new tool the target probably requires (CPU pipelines
are already generated per graph; the glue for the rest is unproven until the first spike). From
a graph it:
1. decides the fused regions and their boundaries;
2. emits the glue: the shader composition for GPU regions, the Rust crate instantiating the
   generic components for CPU regions, and the executor's schedule;
3. leaves all optimisation to the existing compilers (the driver through wgpu/naga, `rustc`).

It is not a compiler or a JIT of our own. A generic precompiled build covers the common
graphs and runs while the specialised build compiles; the new build must match the generic
one pixel for pixel before it is trusted, then hot-swaps in. A user-triggered break-in mode
spends longer on the best build (fat LTO, profile-guided). Builds are keyed by the canonical
graph and cached.

**Acceptance.** For every pipeline that exists as a hand-written equivalent, the composed
version must give identical pixels and run within noise of it (about 1–2%) on the same
devices. A miss is a bug in the generator or a component, not an accepted cost. The A/B
tooling (`scripts/ab/`) is the test. Pipelines without a hand-written equivalent are not
compared against anything; their slowness is their own.

## The editor

- The graph is the document. Adding a node, rewiring, removing: always allowed, one level of
  undo per action.
- Ports show their types; a connection that does not type-check is refused with the reason
  (the diagnostic names the port and the types).
- Diagnostics live on the node or edge at fault, with the reason, live as the graph changes.
- Knobs, device and API are on the node. A choice that has no implementation says so there.
- Live edits apply in place where the knob is live, otherwise as a hot swap of the pipeline.
- A concise text form (a small scripting language) is a second way to write the same graph,
  for users who prefer it to the canvas: same graph, same diagnostics, convertible both ways.
  Not designed yet.
- "Start from…" loads an example graph, as any saved graph can be loaded. Presets are
  examples, not the legal shapes.

## Where the code differs

The migration below is done (2026-11-04 to 2026-11-09): every pipeline runs as nodes on the
generic executor of its device, "back end" means device + API, and the editor edits any graph
exactly. Edge resources are the executor's (`exec::Edges`, `exec::Form`, and `exec::Split` for a
pipeline over two GPUs, whose hit records cross between them in each of the hits edge's forms;
on the CPU the passes read only what their graph edges reach, `exec::reads`); the regions keep
their programs, group 0 and internal buffers, and the generator specialises CPU regions only,
both by design (below). What still differs from the target:

| Target | Today |
|---|---|
| Fusion is attempted, not pre-judged (required: every graph meeting the validity rules executes) | Each executor declares the nodes it builds (`graph::Builds`: per role the components, device kind and count), and `Graph::accept` refuses, with the reason, any valid graph outside the declarations at creation instead of trying it. The declarations are the regions' shapes, so a graph is accepted exactly when a region builds it. The ten presets named by technique and device (`raycast-gpu`, `raster-cpu`, ...) are those regions' names: derived from the graph, offered as starting points, never chosen. |
| Diagnostics on the node or edge at fault | A graph that does not run is a draft; the nodes its reason names, and the node a failure or refused knob is about, are marked red on the canvas and in the pipeline rows, the reason on hover (`docs/gui.md`). An edge at fault is not marked, a reason naming no node stays the lane's, draft steps are not in the undo stack, and nothing suggests a fix (the graph editing work). |

Decided for the GPU rows (2026-11-23):

- **A region owns its program, group 0 and its nodes' internal buffers, by design.** A fused
  region *is* one program, so its compiled pipelines and its bindings are its own. The rule:
  anything another node reads is an edge (a port, its resource the executor's); anything only
  one program uses (the drawers' depths, the splats' depth buffer) stays the region's. A
  buffer a second node comes to need becomes an output port then, not before. So the face
  drawer's per-pixel block lookup of merged faces reads the storage bound to its face
  source's input (the `faces` visibility's `storage`, the quadtree's `exposure`), never the
  shadow walk's by accident of linking: unbound (the shipped files), nothing merges across
  block types and nothing is looked up.
- **Every form of the hits edge has a transfer between GPUs.** The records cross as a buffer
  (rays) or a render target (the `gbuffer` drawer: the first GPU draws without the shading,
  the target is copied out with padded rows); the split's near end is the node the shading's
  hits come from. Ids (visbuffer, splats) decode only with their source, so a decode pass on
  the first GPU turns them into records first (the Ids -> Records adapter); meshes drawn with
  the faces or the rays stay on one GPU.
- **The generator specialises CPU regions only, by design.** A GPU program is specialised by
  composing its shader per graph and baking the knobs as constants (compiled per key, cached),
  which is the GPU's monomorphisation; a generated Rust crate adds nothing there. The GPU
  executor's CPU work (meshing, uploads) is per chunk or per frame, not per step. A CPU node
  of the GPU executor that becomes per-pixel hot is specialised like a CPU region.

What already matches: typed ports, free edges, per-node device and API, `prev`/`iterate`
timings, the schedule, shader-module composition, the shared hit record, isolation and the
stream/gate/client nodes, the canonical graph key, on-demand compiled builds with pixel
verification, the A/B tooling, one executor per device, executors reading their nodes' knobs
and plans and labels read off the graph.

## Every valid graph executes

A graph meeting the validity rules (`Graph::check`) must run; today `Graph::accept` refuses
those outside every region's declarations. The refusals are measured, not guessed:
`graph::refused()` takes every valid graph one edit from the shipped files (an input rebound
or unbound, a new node, a component swapped: `graph::edits`; a node moved to another device,
an edge timed `prev`) and keeps those `accept` refuses, grouped by the reason its message
names (`multirenderer audit --refused` prints the table; the test
`valid_graphs_refused_only_go_down` pins each count, which may only fall).

| gap | reason | graphs (2026-11-27) | what is missing |
|---|---|---|---|
| no region of that shape | no region has the shape | 1 (33 before the unfused passes) | GPU discovery drawn by the CPU rasteriser |
| no region of that shape | the shape is the other executor's | 9 | the visibility on one device kind, the drawer on the other (cross-device graphs) |
| node outside the declarations | role not built | 5 | mesh records beside ray queries or GPU discovery, holes beside CPU discovery |
| node outside the declarations | role on the other device kind | 11 | a kernel, a culling or a shading on the other device than its region's |
| node outside the declarations | component not built | 10 (19) | a culling or splatting over GPU discovery, faces beside ray queries or splatting, a heightmap in the meshing region |
| node outside the declarations | number of nodes of a role | 6 (37) | no kernel or no storage in the GPU regions, a second heightmap for the GPU column march |
| node outside the declarations | what a port reads | 7 (10) | an unbound shadow site in the GPU regions, faces splatted as points, ray queries into GPU discovery's set |
| edge without a resource form | edge timing | 6 (41 before the CPU's `prev` edges) | `prev` on the GPU's per-chunk streams uploaded from the host (splat points, ray-query geometry, a drawer's meshes); hits late behind a forward drawer (no records) |
| edge without a resource form | device boundary | 6 | a second GPU anywhere but at the split's hit records |

**Edges timed `prev` on the GPU** (2026-11-27). An edge one GPU executor holds between two
passes (its region's `Builds::crossings`, and on one GPU the shading's hits) may be timed
`prev` without a gate: `cross::EdgeHold` copies the producer's value aside after its pass
and swaps it into the edge's resources around the reader's pass (copies on the device, no
read-back, no wait; the passes reordered so the producer runs first where no cycle forbids).
Hits into the shading are shaded with the camera and camera block they were cast with;
rays write records whatever the placement once their hits are late. Gap: a ray query's
acceleration structure (and the meshes) stay placed for this frame's camera block, so lit
shadow rays from last frame's records are off by the block the camera crossed. The world's
changes timed `prev` are held back a frame per copy of the world the GPU executor keeps
(`Checked::late_world` -> `WorldTiming`): the GPU's (voxel pool, storages' data, lights), the
geometry's, the CPU storages' (the CPU discoverer's world) and the meshes'. The readers of
one copy read alike (else refused, naming them); a geometry timed apart from the GPU's
storages gets a CPU-side copy of the chunks of its own (`Core::geometry`, the world's memory
once more), so the storage and the geometry may each take the changes this frame or the last.

**Design: unfused passes with edge resources.** The executor runs the graph's schedule
(`Checked::schedule`) pass by pass; every non-fused edge is a resource the executor owns
(the CPU's `exec::Res`, the GPU's `exec::Edges`), so a node needs only a *standalone*
implementation (its pass reading its inputs' resources, writing its output) to run in any
graph. A region is then what it is in the target: an optimisation the executor attempts over
a run of passes it knows how to fuse (one program, internal buffers, an edge kept in
registers), with a failed attempt reported and the unfused passes run instead. Concretely,
per gap:

- *Shape and node gaps*: the executor builds the passes it has standalone implementations
  for, whatever region they would belong to; a region claims the passes its shape covers and
  the rest stay single passes. Each role's standalone pass is the region's node code minus
  the region's private bindings: culling (faces) reads faces and writes faces, a drawer reads
  faces and writes hits, a storage pass keeps its storage and hands it to every reader
  (fused data is compiled into each reader, so two storages are two passes, not one region
  slot), a shading reads hits and its sites.
- *Edge gaps*: each edge resource gets the three things an edge needs: a copy a frame late
  (`prev`), a transfer between devices and a serialized form (gates). The serialized form
  exists for every edge a gate runs on; a copy is its `sent` half without the bytes.
- *Speed*: an unfused graph is not a slow fallback. CPU passes are monomorphised per graph as
  the fused regions are; GPU passes are composed into as few programs as the edges allow, edge
  resources stay on the device; every pass and edge boundary an unfused graph adds is measured
  against the hand-fused equivalent of the same pieces.

**Done: `prev` on any CPU edge.** The CPU executor's regions declare `Builds::late`: any edge
that is not fused data may be timed `prev`. A hold pass on the edge keeps a copy of last
frame's value (the edge's serialized form's `sent`, through no gates; the stage `hold`; a
blank value before the first frame and after a resize): just before a reader scheduled before
its producer (it copies the producer's output as it stands, last frame's), just after the
producer otherwise (it outputs a frame later what it took). The copy costs one pass over the
value a frame; reading the producer's output in place where it fits is left to do. A value seen from a camera (hits, screen triangles) is read with the camera it
was made with, so a frame of hits a frame late is exactly the previous frame (the world's
meshes are read with this frame's camera); a shading reading its hits late runs in a pass of
its own whatever its `placement`; the world's changes into a node timed `prev` reach it a frame
late (`tests/late.rs`).

**Done: the CPU's first unfused shapes** (157 -> 138 refused). The CPU rasteriser and column
march build each node as a pass of its own where their declarations used to stop
(`graph::CPU_RASTER`, `CPU_COLUMNS`; `tests/unfused.rs` runs each and compares with the
shipped graph where the image is the same):

- a drawer reading the geometry's faces straight (no culling node) transforms and clips them
  in its own pass (a `Faces` of its own; pixel-equal to the graph with the culling node);
- a culling may read another culling's visible set (culled again for the frame's camera);
- a face source's lookup storage is kept as a pass of its own, any CPU storage (voxels or a
  heightmap), never read: the CPU geometries merge only faces of one block (the shadow rays'
  storage bound there is the one pass);
- cubes (`Cap::Cubes`) are drawn as their six unit faces (the depth test leaves the exposed
  ones: pixel-equal to greedy's quads);
- a shading without a shadow site lights without shadow rays (raster and columns);
- the column march's shadow rays walk a heightmap of their own (the march's storage and
  the shadow site's are a pass each; the deferred shading decodes its records against the
  march's).

Each pass is the region's own node code, monomorphised as before; the shipped graphs build the
same passes in the same order. One cost added to the shipped graphs, not measured yet: the
column march's shadow heightmap is an `Option` (no shadow site: none), checked per shadow ray.

**Done: the CPU's other single-device shapes** (138 -> 129 refused; no graph on the CPU alone
is refused now, the rest of the table is the GPU's and cross-device graphs):

- CPU discovery (`quadtree`, `cluster`) drawn by the CPU rasteriser, straight or through a
  culling node, its faces `now` or `prev`: a discovery pass (`raster::Discover`) runs the
  components' discovery over its own copy of the world in the storage its rays walk (as
  discover-cpu's sampler; the node's knobs shared, `visibility::discover::{Knobs, params}`)
  and hands the face set on as each chunk's unit quads, drawn from the frame's camera. Its
  frames score within a few dB of raster-cpu's against the reference (discovery misses what
  slips between its rays); its region knobs (`sky_tile`, `keep_frames`, `margin`) keep
  discover-cpu's defaults;
- boxes and points drawn by the CPU rasteriser, as the exposed block faces each stands for
  (pixel-equal to `naive`);
- the ray caster over a heightmap's march (`heightfield` bound to its `primary` port), run by
  the column-march region: one ray per pixel through `Heightmap::trace`, records column-major
  as the march writes them, shaded by the same deferred pass (inline: in the same pass).

**Done: the GPU rasteriser's first unfused shapes** (129 -> 95 refused; `graph::GPU_MESHING`,
`tests/unfused.rs`, `tests/late.rs`, all pixel-equal to the shipped graph):

- the drawer reading the geometry's faces straight (no culling node): every chunk's faces in
  one draw each, the hardware's back-face culling and the depth test leaving what the culling
  would;
- a culling over another's culled set (both for the frame's camera: the same set);
- no storage at all, the shadow rays ray-queried into the drawn geometry's acceleration
  structure (the same image as walked: both find the same blocks);
- the faces `prev`: the geometry's chunks uploaded a frame late, or last frame's culled draws
  drawn with this frame's camera;
- a hits gate between GPU passes restores the block the records were cast from with their
  camera (lights and shadow lookups were a block off on a crossing frame), slices-gpu's hits
  through a late gate shaded deferred (the plane sweep has no `placement`).

**Done: CPU discovery's exposure storage; boxes and points on the GPU rasteriser** (95 -> 68
refused). CPU discovery's exposure tests read the storage bound to its `exposure` port, which
may differ from the one its rays walk (`grid::Paired`: a runtime pair reads the exposure
storage dynamically, a compiled pipeline pairs both statically). Boxes and points are drawn by
the GPU rasteriser as the exposed block faces each stands for, as the CPU rasteriser draws them
(`geometry::drawn`: naive's culled quads, the same image).

**Done: the GPU ray caster over a heightmap's march** (62 -> 61 refused). As on the CPU, a
GPU ray caster whose primary rays are the `heightfield` kernel is the column-march region's
(`graph::GPU_COLUMNS`): its program is the march's with a second entry
(`crates/mr-exec-gpu/src/columns_cast.wgsl`, `cast_rays`) casting one ray per pixel through
the heightmap's `hm_trace`, the surface the march gives the same column, shaded inline or
written as records for the common deferred pass. Its frames score as the CPU caster's against
the reference (`tests/unfused.rs`).

## Migration

Done, one lane at a time, the old path kept until its replacement matched (pixels identical,
speed within noise; `scripts/ab/`):
1. The node interface and the generic executor: the CPU first (`crates/mr-exec-cpu/src/exec.rs`,
   each pass a node with an output resource the executor owns, dispatch per pass only), then the
   GPU (`crates/mr-exec-gpu/src/exec.rs`, each node recording into the frame's command encoder,
   nodes compiled into one program sharing a region).
2. Ray casting as storage → visibility → shading nodes on both, the hit records the edge
   between them, `placement = inline` fusing visibility and shading. This was the spike for the
   native-speed claim; it needed no generator, so none was built.
3. The other lanes the same way: columns, rasterisation (geometry → visibility → draw →
   shading), point splatting, meshing's faces, CPU and GPU discovery. The lane interfaces are
   gone; the executors own the renderer interface (world changes, knobs, resizes, memory, frames).
4. `accept` = the validity rules plus the executors' declarations, for every graph; the editor
   edits exactly, with drafts.
5. The ten named back ends retired: a graph names its executor by device kind (`executor =
   "cpu"` or `"gpu"`), executors read their nodes' knobs from the graph (a live slot switch is a
   graph edit), plans and labels are read off the graph (written as a graph file and edits, or the graph),
   `RendererSpec` holds the graph, and the compiled pipelines are keyed by the graph and its
   executor alone (the knobs each component declares specialisable).

## Non-goals

- A compiler or JIT of our own.
- Making every graph fast. Slow graphs are the user's.
- Hardware opinions inside components, or silent substitution of a different pipeline from
  the one requested.
- Verifying all graphs by enumeration.

## Open questions

- **Region boundaries:** today one region per program, as the executor's declarations say.
  Whether they are decided automatically by the generator, declared by components, or chosen
  on the node by the user is still open.
- **Resource formats on edges:** how a component declares the layouts it can read and
  write, so the generator can pick one without extra conversion passes.
- **Which graphs the CPU generator can specialise:** how far monomorphisation reaches when
  the graph, not a fixed region, decides the generic parameters.
