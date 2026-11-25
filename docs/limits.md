# Size limits: where each pipeline stops

How large a world each pipeline renders before it stops, and what stops it. A pipeline past
a limit fails naming it (the limit, the knob that moves it, or what it ran out of); it never
renders with chunks quietly missing. Measured with `scripts/scale.py`, which doubles the view
radius over the scenario plans and records, per pipeline, the largest radius rendered and the
first failure (pass / fail only, no timing):

    scripts/scale.py --plans terrain,lattice,city --radii 8,16,32,64,128
    scripts/scale.py --table

The run goes inside a systemd scope capped at `--mem` (default 24G, no swap), so host memory
running out is a recorded stop, not a frozen machine. Steps are journaled
(`target/scale/results.jsonl`); the same command resumes.

## Stopping sizes

View radius in chunks (Chebyshev, 32-block chunks, `max_lod = 0`), 640x360, NVIDIA RTX
(8 GiB, shared with whatever else runs on it), host capped at 24 GiB. A cell is the largest
radius that rendered and, in brackets, the radius at which it stopped and why; `≥ 64`: still
rendering at the largest radius tried (terrain was tried at 128 too).

| pipeline | city | lattice | terrain |
|---|---|---|---|
| columns-cpu | ≥ 64 | ≥ 64 | 64 (r128: [1]) |
| columns-gpu | ≥ 64 | ≥ 64 | 64 (r128: [1]) |
| discover-cpu | 32 (r64: [2]) | 16 (r32: [2]) | 32 (r64: [2]) |
| discover-gpu | 16 (r32: [3]) | 16 (r32: [3]) | 32 (r64: [3]) |
| raster-cpu | ≥ 64 | ≥ 64 | 64 (r128: [1]) |
| raster-gpu | 32 (r64: [2]) | 16 (r32: [2]) | 32 (r64: [2]) |
| raycast-cpu (every storage) | ≥ 64 | ≥ 64 | 64 (r128: [1]) |
| raycast-gpu (bricks, hier, prefix, distance, rows) | 32 (r64: [4]) | 32 (r64: [4]) | 32 (r64: [4]) |
| raycast-gpu storage=tree64 | ≥ 64 | ≥ 64 | 64 (r128: [1]) |
| raycast-gpu storage=svo_dag | ≥ 64 | ≥ 64 | 64 (r128: [1]) |
| raycast-gpu storage=clipmap | 32 (r64: [7]) | 32 (r64: [7]) | 32 (r64: [7]) |
| rayquery-gpu | 32 (r64: [5]) | 32 (r64: [5]) | 32 (r64: [6]) |
| rayquery-gpu geometry=naive | 8 (r16: [8]) | 32 (r64: [6]) | 16 (r32: [6]) |
| rayquery-gpu geometry=boxes | 32 (r64: [4]) | 32 (r64: [4]) | 32 (r64: [4]) |
| raster-gpu geometry=naive | 32 (r64: [2]) | 16 (r32: [2]) | 32 (r64: [2]) |
| raster-gpu geometry=cubes | 32 (r64: [2]) | 16 (r32: [2]) | 32 (r64: [2]) |
| splat-gpu | 32 (r64: [4]) | 32 (r64: [4]) | 32 (r64: [4]) |

1. Host memory: the run (host and workers together) was killed at the 24 GiB cap in its
   first pass; terrain at radius 128 is 257² chunk columns, and the host holds the world it
   streams from. Whether the host's world or the worker's copy crossed the cap first is not
   yet separated.
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

## Not yet covered

`max_lod > 0` (the view reach at LOD is radius x 2^max_lod chunks; checked only through the
reach above), lavapipe's limits, host and worker world at the memory cap, and points, faces
and instances on the other drawers (counts above are raycast-gpu, raster-gpu and splat-gpu).
