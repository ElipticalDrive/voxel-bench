# Size limits: where each pipeline stops

How large a world each pipeline renders before it stops, and what stops it. A pipeline past
a limit fails naming it (the limit, the knob that moves it, or what it ran out of); it never
renders with chunks quietly missing. Measured with `scripts/scale.py`, which doubles the view
radius over the scenario plans and records, per pipeline, the largest radius rendered and the
first failure (pass / fail only, no timing):

    scripts/scale.py --plans terrain,lattice,city,islands --radii 8,16,32,64,128
    scripts/scale.py --table
    scripts/scale.py --plans terrain,lattice --radii 8,16,32 --lavapipe   # Mesa's software Vulkan

The run goes inside a systemd scope capped at `--mem` (default 24G, no swap), so host memory
running out is a recorded stop, not a frozen machine. Steps are journaled
(`target/scale/results.jsonl`); the same command resumes.

## Stopping sizes

View radius in chunks (Chebyshev, 32-block chunks, `max_lod = 0`), 640x360, NVIDIA RTX
(8 GiB, shared with whatever else runs on it), host capped at 24 GiB. Re-run 2026-11-26 with
islands added (radius 128 from the first run, terrain only). A cell is the largest
radius that rendered and, in brackets, the radius at which it stopped and why; `≥ 64`: still
rendering at the largest radius tried (terrain was tried at 128 too).

| pipeline | city | islands | lattice | terrain |
|---|---|---|---|---|
| columns-cpu | ≥ 64 | ≥ 64 | ≥ 64 | 64 (r128: [1]) |
| columns-gpu | ≥ 64 | ≥ 64 | ≥ 64 | 64 (r128: [1]) |
| discover-cpu | 32 (r64: [2]) | 32 (r64: [2]) | 16 (r32: [2]) | 32 (r64: [2]) |
| discover-gpu | 16 (r32: [3]) | 32 (r64: [3]) | 16 (r32: [3]) | 32 (r64: [3]) |
| raster-cpu | ≥ 64 | ≥ 64 | ≥ 64 | 64 (r128: [1]) |
| raster-gpu | 32 (r64: [2]) | 32 (r64: [2]) | 16 (r32: [2]) | 32 (r64: [2]) |
| raycast-cpu | ≥ 64 | ≥ 64 | ≥ 64 | 64 (r128: [1]) |
| raycast-gpu | 32 (r64: [4]) | ≥ 64 | 32 (r64: [4]) | 32 (r64: [4]) |
| rayquery-gpu | 32 (r64: [5]) | ≥ 64 | 32 (r64: [5]) | 32 (r64: [6]) |
| splat-gpu | 32 (r64: [4]) | ≥ 64 | 32 (r64: [4]) | 32 (r64: [4]) |
| raycast-gpu storage=hier | 32 (r64: [4]) | ≥ 64 | 32 (r64: [4]) | 32 (r64: [4]) |
| raycast-gpu storage=prefix | 32 (r64: [4]) | ≥ 64 | 32 (r64: [4]) | 32 (r64: [4]) |
| raycast-gpu storage=distance | 32 (r64: [4]) | ≥ 64 | 32 (r64: [4]) | 32 (r64: [4]) |
| raycast-gpu storage=rows | 32 (r64: [4]) | ≥ 64 | 32 (r64: [4]) | 32 (r64: [4]) |
| raycast-gpu storage=tree64 | ≥ 64 | ≥ 64 | ≥ 64 | 64 (r128: [1]) |
| raycast-gpu storage=svo_dag | ≥ 64 | ≥ 64 | ≥ 64 | 64 (r128: [1]) |
| raycast-gpu storage=clipmap | 32 (r64: [7]) | 32 (r64: [7]) | 32 (r64: [7]) | 32 (r64: [7]) |
| raycast-cpu storage=hier | ≥ 64 | ≥ 64 | ≥ 64 | 64 (r128: [1]) |
| raycast-cpu storage=prefix | ≥ 64 | ≥ 64 | ≥ 64 | 64 (r128: [1]) |
| raycast-cpu storage=brickmajor | ≥ 64 | ≥ 64 | ≥ 64 | 64 (r128: [1]) |
| raycast-cpu storage=morton | ≥ 64 | ≥ 64 | ≥ 64 | 64 (r128: [1]) |
| raycast-cpu storage=u8 | ≥ 64 | ≥ 64 | ≥ 64 | 64 (r128: [1]) |
| raycast-cpu storage=bits | ≥ 64 | ≥ 64 | ≥ 64 | 64 (r128: [1]) |
| raycast-cpu storage=palette | ≥ 64 | ≥ 64 | ≥ 64 | 64 (r128: [1]) |
| rayquery-gpu geometry=naive | 8 (r16: [8]) | 32 (r64: [9]) | 32 (r64: [10]) | 16 (r32: [6]) |
| rayquery-gpu geometry=boxes | 32 (r64: [4]) | ≥ 64 | 32 (r64: [4]) | 32 (r64: [4]) |
| raster-gpu geometry=naive | 32 (r64: [2]) | 32 (r64: [2]) | 16 (r32: [2]) | 32 (r64: [2]) |
| raster-gpu geometry=cubes | 32 (r64: [2]) | 32 (r64: [2]) | 16 (r32: [2]) | 32 (r64: [2]) |

1. Host memory: the run (host and workers together) was killed at the 24 GiB cap in its
   first pass; terrain at radius 128 is 257² chunk columns. The host's world and the worker's
   copy grow alike (below, "Host and worker memory"): about 19 GB each at radius 128
   (extrapolated from the area), so neither alone crosses 24 GiB, both together do.
2. The voxel pool is full (24576 chunks: `pool_mb` 1536); raise `pool_mb`.
3. discover-gpu holds at most 16384 non-air chunks' face slots; lower the client's radius.
4. The voxel pool is full (32767 chunks: `pool_mb` 2048, one binding of NVIDIA's 2 GiB less a
   byte); raise `pool_mb` (the pool then spans several bindings).
5. rayquery's acceleration structure holds at most 65536 chunks (its TLAS instance slots);
   lower the client's radius.
6. Device memory: the merged geometry buffer could not grow (256 to 512 MiB). Shared with the
   other programs on the GPU, so this stop moves with what else runs.
7. Device memory: the clipmap's 3D textures grow with the cube of the radius (8452 MiB at 64:
   a volume 129x16x129 chunks). Past `max_texture_dimension_3d` (on a WebGPU-baseline device,
   2048 texels: radius 31) it is refused naming that limit.
8. Device memory: the per-chunk BLAS inputs (city's naive faces: 1530 MiB for 4660 chunks).
9. Device memory, named by the device's error handler (islands' naive faces at radius 64; the
   renderer was seen holding 4368 MiB).
10. Host memory: the run was killed at the 24 GiB cap during the pass (lattice's naive faces at
    radius 64, built on the CPU, beside the host's copy of a world with no air chunks).

The world's content decides as much as the radius: lattice (beams in every chunk, no air
chunks) fills the pools at half terrain's radius, and city's naive faces exhaust device memory
at radius 16. tree64 and svo_dag (region storages: their own structures, not the voxel pool) ran as far
as the CPU pipelines.

## Fixed by these runs

- rayquery-gpu panicked when the device refused a BLAS input buffer (`create_buffer_init`
  maps the refused buffer, a panic before the error can be named); it now fails naming the
  inputs' size and chunk count.
- The clipmap clamped its volume to `max_texture_dimension_3d` and let chunks past it evict
  each other from their slots, only counted (`dropped`): frames missed chunks with no error.
  A radius over the limit now fails naming it (test: `tests/scale.rs`, a flat world on a
  device clamped to WebGPU's sizes, radius 31 renders and 32 is refused). Its textures'
  allocation failing is named with their size.
- wgpu's own out-of-memory error names no allocation; the device's error handler now says
  the device is out of memory, with the most this renderer was seen holding and the knobs
  that size it.

## Far from the origin, frame sizes, generator sizes

Not radius but position and size parameters, each checked at the smallest size that showed
the break (tests named).

| limit | where it is refused | what broke past it (before) |
|---|---|---|
| A camera within 2^30 blocks of the origin on each axis (`WORLD_LIMIT`) | plan load: every point of the camera path (`scene.camera: ... past the world's reach`) | at `i32::MAX` less the view reach, every pipeline rendered the far side of the world, no error (`sizes_past_the_limits_are_refused`) |
| A generator size (blocks or counts) within 2^30; menger level ≤ 18 (3^18 blocks) | plan load and the editor's fields (the spec does not parse) | room overflowed `i32` from size 715827883, menger from level 20, the others near `i32::MAX`; gyroid's spawn scanned the whole period (31 s at 2^30) (`generator_sizes_hold_out_to_the_worlds_reach`) |
| Free flight (GUI, interactive window) within 2^30 blocks on each axis | the camera stops at it, each axis clamped, and says so (the GUI's message line, the window title) | flew on past the reach unchecked, so the view wrapped as a path past it did (`free_flight_stops_at_the_worlds_reach`) |
| A frame side at most the device's `max_texture_dimension_2d` (32768 on the RTX 5060, 8192 at WebGPU's baseline) | the GPU executor's build and a live resize, naming the limit | a bare `wgpu error: Validation Error` (`a_frame_past_the_texture_limit_is_refused_naming_it`) |
| A frame of at most `u32::MAX` pixels, both sides ≥ 1 | plan load, a live resize | pixel indices are `u32` in the shaders and the CPU frame buffers; byte sizes were `u32` products too, so an RGBA16F frame of 23171² sized its shared memory slot 3 MiB (`frame_sizes_are_counted_wide`) |

Inside these, pipelines render the same image anywhere: a flat world seen from the origin,
from 2^19, 2^24 and ±(2^30 - 1024) blocks out gives pixel-identical frames on every shipped
graph (the renderers work camera-relative in `i32` blocks plus an `f32` fraction; the camera
is `f64`). Two things did not, fixed:

- The CPU discovery (discover-cpu) keyed faces by 20 bits of each absolute coordinate and
  read them back as if near the origin: from 2^19 blocks out it drew nothing right (8.7 dB at
  2^24). Keys are now read back as the cell nearest the camera, which holds while a face is
  within 2^19 blocks of it (`face_keys_hold_anywhere_in_the_world`).
- Terrain (and the islands' and caves' noise) took `f32` block coordinates, whole only up to
  2^24: 8 % of terrain heights were wrong there, 63 % at 2^28 (neighbouring columns merged).
  The noise now finds its lattice cell in `f64` (`noise_resolves_blocks_out_to_the_worlds_reach`).
  Near the origin 33 of 4.2 million terrain heights moved by one block (rounding).

Frames between processes: a message is refused past 4 GiB (its `u32` length); frames go
through shared memory slots sized in `u64`.

## Big buffers, big frames, many instances (measured)

On the RTX 5060 (8 GiB), each run pixel-compared with the CPU reference:

- Voxel pool past 4 GiB: terrain at radius 64 with `pool_mb = 5000` (68331 chunks, 4.43 GB of
  pool over three bindings) renders on raycast-gpu with bricks, hier, prefix and rows at
  57.7 dB, as at radius 32 (59.4 dB; the difference is far-field detail). No offset
  overflows: the pool pages at 2 GiB a binding and slots index within a page. distance (its
  distance field beside the pool), rayquery-gpu boxes and splat-gpu stop earlier, each named:
  device memory, and the splats' points over `max_storage_buffer_binding_size` (3467 MiB for
  227 M points in view).
- Frames: 16384² renders on raycast-gpu (65.1 dB, 70 ms GPU; the host held 10.7 GB with the
  reference). 23200² and 32768² run out of device memory, named (frame-sized targets cost
  about 31 bytes a pixel on raycast-gpu: output, readback ring, hits, accumulation: 8.4 GB of
  device memory at 16384²); 32769 is past `max_texture_dimension_2d`, refused (above).
- Mesh instances: 100 000 boxes render on raster-gpu and raycast-gpu (42 dB against the
  reference, both alike: the boxes are smaller than a pixel); 4 million (48 M triangles) are
  refused naming the limit, 4194304 (2^22) triangles a hit record can name.

## Level of detail (`max_lod > 0`)

    scripts/scale.py --plans terrain --radii 8,16,32,64,128 --lod 2 --only raycast-gpu

With `max_lod = 2` the view reaches radius x 4 chunks, each level at twice the cell size of
the one inside it. Only raycast-gpu over bricks, hier, prefix, distance and rows builds levels;
every other pipeline (and tree64, svo_dag, clipmap) leaves `max_lod` out, says so in
`not_supported` and renders at level 0, so the run marks it left out, not as reaching the
radius. Terrain, same setup as above:

| radius (reach in chunks) | chunks sent, lod 2 (lod 0) | raycast-gpu: bricks, hier, prefix, distance, rows |
|---|---|---|
| 8 (32) | 3696 (1989) | renders |
| 16 (64) | 11520 (6060) | renders |
| 32 (128) | 38642 (19229) | renders |
| 64 (256) | - | the voxel pool is full (32767 chunks: `pool_mb` 2048), named |

The coarser levels cost about as much again as level 0, so the pool fills at the same radius
as at level 0 (the stop [4] above) while the view reaches four times as far. No new break.

## Host and worker memory

Terrain, raycast-cpu (bricks), 640x360, peak resident memory of the host (`host peak memory`)
and of the worker process (`peak_rss_mb`):

| radius (chunk columns) | process: host | process: worker | thread: one process |
|---|---|---|---|
| 32 (65²) | 1269 MB | 1243 MB | 2480 MB |
| 64 (129²) | 4701 MB | 4330 MB | 8701 MB |
| 96 (193²) | 10852 MB | 9252 MB | 19280 MB |

Both grow with the area (about 290 KB a chunk column for the host: terrain's 64 KiB dense
chunks), and they are about equal: the host holds the world it streams from (generated
chunks as dense 16-bit blocks), the worker its renderer's storage built from them. Thread
isolation saves nothing here: the renderer's storage is its own form, not the host's chunks.
So the 24 GiB cap stops both together at radius 128 (about 19 GB each, extrapolated from the
area; not run past the cap).

The host's world is a cache of what the generator gives (edited chunks are kept for good):
it held every chunk the camera had ever streamed, so a long flight grew it without bound
(the streaming plan, 4 km at radius 16: host 1413 MB). It now drops what leaves the streamed
region (1033 MB; the worker 573 MB both times); chunks asked for again regenerate
(`the_world_cache_follows_the_stream`). What the host keeps inside the region is the copy
above; dropping it too (regenerating for a joining client, a pick, an edit) would halve a
process-isolated run's memory at a regeneration cost, not done.

## Counts on the drawers

Mesh instances run on the two graphs that draw meshes (raster-gpu and raycast-gpu; the
others leave meshes passes out), both refusing past 4194304 triangles (above). Faces and
points never reached a limit of their own on the RTX 5060: with the pool raised (`pool_mb`
6000, a renderer option), lattice at radius 32 (25350 chunks) renders on raster-gpu (greedy
and naive faces), rayquery-gpu boxes and splat-gpu; at radius 64 each stops at device memory
growing the pool (1904 MiB), named. discover-gpu stops at its 16384 chunks' face slots
(stop [3]) whatever the pool. The CPU drawers (raster-cpu, raycast-cpu, columns-cpu) stop
only at host memory (stop [1]).

## lavapipe (Mesa's software Vulkan)

    scripts/scale.py --plans terrain,lattice --radii 8,16,32 --lavapipe

The GPU pipelines on lavapipe (`LP_NUM_THREADS=4`), whose sizes are WebGPU's baseline: 128 MiB
a storage binding, 256 MiB a buffer. Every stop names its limit:

| pipeline | lattice | terrain |
|---|---|---|
| columns-gpu | ≥ 32 | ≥ 32 |
| discover-gpu | 16 (r32: face slots, [3] above) | ≥ 32 |
| raster-gpu, geometry naive / cubes | 16 (r32: pool full, 22528 chunks) | ≥ 32 |
| raycast-gpu (bricks, hier, prefix, rows) | 16 (r32: pool full, 22528 chunks) | ≥ 32 |
| raycast-gpu storage=distance | 16 (r32: pool full, 16384 chunks) | ≥ 32 |
| raycast-gpu storage=tree64 | ≥ 32 | 16 (r32: side data past the binding) |
| raycast-gpu storage=svo_dag | ≥ 32 | ≥ 32 |
| raycast-gpu storage=clipmap | 16 (r32: memory for its textures) | 16 (r32: the same, 2146 MiB) |
| rayquery-gpu | ≥ 32 | 16 (r32: device memory) |
| rayquery-gpu geometry=naive | 16 (r32: geometry past the binding) | 16 (r32: the same) |
| rayquery-gpu geometry=boxes | 16 (r32: pool full) | ≥ 32 |
| splat-gpu | 16 (r32: pool full) | 16 (r32: the large points' list past the binding, 216 MiB) |

The voxel pool pages over several bindings (22528 chunks here, 32767 on the RTX, both
named), and so (2026-11-27) do a pool storage's side data and the splats' points: `distance`
rose from 8 / 16 to 16 / ≥ 32 and splat-gpu on terrain from 8 to 16 (re-run with `--only
distance` / `--only splat`). The side data's pages share the stage's 48 storage buffers with
the pool's, so beside `distance` the pool takes 8 pages (16384 chunks) and the side data 4.
What stops first otherwise is a single buffer that does not page (tree64's blobs, merged
geometry, splat-gpu's list of large points: 4 bytes a point, past the binding at 54 M points).
Fixed by an earlier run: a storage's side data past the binding failed with "the storage's
side data does not fit" (distance) or named `pool_mb` as the limit (tree64; its 2048 MiB asks
more than the device binds). A buffer the device bounds below what its knob asks now names
the device's binding size, `(below what `pool_mb` asks)`. Test of the paging:
`side_data_and_points_past_the_binding_size_render_over_several_bindings` (clamped and
unclamped pixel-equal, under `MR_GPU_LIMITS=sizes` and `binding:16`).

## The guard's worlds

The sweep and so the regression guard's baseline include `plans/big.toml` ("terrain-big"):
the terrain flight at radius 32, the largest radius every pipeline renders on terrain (the GPU
pools fill at 64, stop [4]), 1920x1080, unlit and lit. Lattice and city stop earlier on some
pipelines (16), so the size is terrain's; a pipeline past its stop in a sweep is a recorded
failure naming the limit, not a lost pass.
