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
  it; the worker's link and client nodes shape that stream. These are graph nodes already.

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
exactly. What still differs from the target:

| Target | Today |
|---|---|
| Edge resources owned by the executor | On the GPU, nodes compiled into one program share a *region* (ray casting, faces, splats, columns, discovery) that owns the frame's state and bindings; the hit records are a binding of the region's group 0, not an output resource the executor owns, and the second GPU's shading is the region's remote part rather than a node of an executor of its own. On the CPU, one edge the graph leaves implicit is read by hand: raster-cpu's shading (inline: in the draw pass) casts its shadow rays in the geometry node's chunk replica, the region's own shadow storage, since its graph has no storage node (raster-gpu's has: `bricks` + `refloor`). |
| Fusion is attempted, not pre-judged | Each executor declares the nodes it builds (`graph::Builds`: per role the components, device kind and count), and `Graph::accept` refuses, with the reason, any valid graph outside the declarations at creation instead of trying it. The declarations are the regions' shapes, so a graph is accepted exactly when a region builds it. The ten presets named by technique and device (`raycast-gpu`, `raster-cpu`, ...) are those regions' names: derived from the graph, offered as starting points, never chosen. |
| Graph-driven program generator | `src/pipelines.rs` is keyed by the graph and its executor alone (the knobs declared specialisable valued on their nodes) and generates one crate per executor, but only the CPU regions are specialised: the GPU executor's crate builds its CPU work natively and bakes no knob (the shaders compile their own constants in). |
| Diagnostics on the node or edge at fault | A graph that does not run is a draft with its reason as text; the canvas does not mark the node at fault, draft steps are not in the undo stack, and nothing suggests a fix. |

What already matches: typed ports, free edges, per-node device and API, `prev`/`iterate`
timings, the schedule, shader-module composition, the shared hit record, isolation and the
stream/link/client nodes, the canonical graph key, on-demand compiled builds with pixel
verification, the A/B tooling, one executor per device, executors reading their nodes' knobs
and plans and labels read off the graph.

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
   graph edit), plans and labels are read off the graph (`compose` + `sites` an input shorthand),
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
