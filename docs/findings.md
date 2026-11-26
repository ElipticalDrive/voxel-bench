# Findings

Measured on a Ryzen 9 9955HX and an RTX 5060 Laptop GPU, at 1080p on the terrain flight
unless noted. These numbers are indicative: GPU time for the same configuration has varied
between 1.7 and 3.0 ms across separate runs (boost clocks, power state), while interleaved
repeats (`run.repeats`) keep the spread within ±1%. The techniques themselves are
described in the encyclopedia [Onkographos](https://github.com/ElipticalDrive/onkographos).

The baseline (`baselines/sweep-20261125/`, its README lists what it holds) has measured the
`showcase`, `terrain-flight` and `far` plans in the graph architecture (4269 passes); the other plans
run as background work and are folded in as they finish. Figures below from other plans come
from earlier, separate runs (dated where known).

## Storages and walks (GPU)

- `prefix` + `refloor` is the fastest GPU ray walk on open scenes (−13% on terrain, −23% under
  edits, compared with `bricks` + `refloor`). With LOD it is 22% faster.
- `distance` + `sphere` has the cheapest traversal (0.9 ms against 1.3 ms for `bricks`), but is
  twice as slow under heavy edits: edited chunks are walked cell by cell until their distance
  field is recomputed. It streams in about 1–1.5 ms of world update per frame on the `city`
  and `islands` plans (mean; p95 6–8 ms) since its field is built by bitmask dilation
  (2026-11-16; was 7–10 ms mean, p95 40–68 ms, from a transform quadratic per line).
- The stress worlds (2026-10-19, 100 frames each): on `lattice` (beams every 24 blocks) ray
  walks cost 2–4.5 ms of GPU time, while `raster-gpu` takes 0.15 ms; on `checker` (96³, every
  block six faces) rays stop at once (0.06–0.3 ms) while rasterisers pay for hidden faces
  (`raster-gpu` 1.1 ms, `raster-cpu` 72 ms); `city` costs `raster-gpu` 3.6 ms of CPU meshing
  per frame while streaming.
- `rows` + `rowrun` is 5–15% slower than `bricks` + `refloor`.
- `tree64` runs 5–6% behind `bricks` with about 15% of the VRAM (re-measured 2026-11-23; the
  same gap as the former tree64 renderer had to its own `bricks`). `svo_dag` deduplicates about 3×
  on terrain and has the smallest footprint. `clipmap` reads textures only (no hash table), and
  its slot wrap happens once per chunk.
- `incremental` is 2.0× (unlit) / 2.4× (lit) slower than `refloor` over `bricks` on NVIDIA and
  1.5× over `tree64` (2026-11-23; earlier 3.5–8×, and 1.3× on AMD). Where the block read sits in
  the loop swings it per storage, so it is the compiler's code for the loop, not extra work.
- LOD at the same far view distance (768 blocks): 136 MB of VRAM against 1037 MB at full
  resolution, and faster. The quality loss is concentrated in pixel-scale foliage in the
  distance. Against the full-resolution reference at the same reach (radius × 2^max_lod) it
  scores 13–25 dB, 7–25% bad pixels: the coarse cells' loss, which per-pixel PSNR counts in
  full; on `lattice` thin beams alias or vanish at coarse levels. Reports also score LOD against
  a reference at the same LOD (`quality_like`), which is what the catalogue's floor uses.
- The `beam` prepass is exact but still a net loss: chunk-level skipping already makes empty
  space cheap.
- `dispatch = "fragment"` (the walk in a fragment shader) runs at the same speed as compute.

## CPU

- The `raycast-cpu` preset is about 1.5× the reference renderer; avoiding libm `floor` calls was
  the big win.
- SIMD packets do not help, and **not** because of memory: hardware counters show ~80 cycles
  and ~220 instructions per DDA step at IPC 2.6, with L2 misses costing at most 3% of the
  time. Shrinking the world data 7–16× (`bits`, `palette`) leaves cycles per pixel unchanged.
  `brickmajor` and `morton` halve L1 misses, but cycles do not drop, so they run 2.5–3.3%
  behind `bricks` (2026-11-23: the index math on every probe).
- What helps is fewer steps: `hier` skips empty 8³ regions (20–34% fewer steps, up to +29%
  fps). `prefix` takes the fewest steps of all (−31 to −51%) but is 3–9% behind `hier` per
  pixel, because big steps cost more than unit steps in this walk. `incremental` with `exact`
  brings `prefix` within 2% of `hier` on terrain, but loses on short rays.
- `schedule = tiles` (screen tiles along a space-filling curve, 2D packets) is 4% faster than
  rows. Software prefetch only added instructions.
- Pipelines compiled for the machine (`compile = "native"`) are 2–13% faster than the generic
  build (`prefix` + `refloor` 11–13%): the generic x86-64 target lacks `popcnt` and wide
  vectors outside the hand-written AVX paths.
- `raster-cpu` is the closest match to the reference (64–99 dB), but its cost scales with the
  number of faces (Menger sponge: 8 fps). `columns-cpu` gives the same image as
  `columns-gpu` at about half its speed.

## Rasterisers and hardware ray tracing

- `naive` makes 2.6–18× more faces than `greedy`. `greedy` with `merge = exposure` (merging
  across block types) has 16% fewer quads and 13% less GPU time on menger, and is slightly
  slower on terrain.
- `visbuffer` is 1.5–2.5× faster than `raster` when lit with lots of overdraw, and halves lit
  time for `cubes`. `gbuffer` is a little slower than `visbuffer`.
- Lit-mode shadow rays of the rasterisers walking the composed storage are 7–18% faster than
  their former fixed walk.
- `rayquery-gpu` traces fastest on the RT cores, but its BLAS rebuilds make it CPU-bound when chunks
  stream in or change.

## Discovery

- `discover-gpu` (`quadtree`): 0.05–0.34 rays per pixel, audit misses ≈0–1 pixel per frame.
  It is not yet faster than rasterising everything: the levels run one after another, and late
  rays are incoherent. `holes = coverage` (only re-discover where something may have changed)
  uses 83% fewer rays and 33% less GPU time at walking speed with equal or better quality, but
  is 13–19% slower in fast flight. A still camera skips discovery entirely (17× less GPU time).
  `visbuffer` / `gbuffer` drawing is 2–4× faster than `raster` when lit on terrain, and a
  little slower unlit.
- `discover-cpu`: `cluster` is more complete than `quadtree` on every scene at about the same
  number of rays (terrain 0.24 rays per pixel, 51.0 against 43.8 dB; menger 51.5 against
  26.4 dB), at 2.5× its CPU time. Pipelined (discovering frame N+1 while the GPU draws frame N),
  one frame of latency costs 0.1–0.3% of the pixels while moving. CPU discovery (25–60 ms) is
  the bottleneck. (Those `quadtree` figures predate three of its settings, 2026-11-21: `strict`
  footprints split a coherent-looking tile whose plane has a hole (a window: city 21 -> 36 dB);
  `jitter` shifts the tile grid every frame, so small floating blocks between one frame's corner
  rays are hit on another; `dilate = 2` adds the camera-facing faces around fine-level hits
  (city lit 33 -> 45 dB) for ~45% more discovery time. `dilate = 0` is the fast setting.
  2026-11-23: `strict` reads only the cell in front of each footprint face, so a block floating
  a cell or more above a floor, between the corners' rays, passed it; `occluders = 4` also walks
  4 cells from each face toward the eye and splits the tile on a solid one: showcase lit settled
  28.6 -> 36.7 dB, unlit 30.5 -> 36.8 at 480x270. The GPU `quadtree` has the same walk; it
  changed neither showcase (36.7 dB) nor city lit (35.7) at that size. Its cost (idle, 300
  frames, against `occluders = 0`): GPU discovery+draw +6% lit / +11% unlit on showcase, +2% /
  +5% on terrain; CPU quadtree -12% fps on showcase, -3% on terrain.)
  `cluster` missed small distant blocks (islands lit 34.0 dB settled at 480x270): a lattice
  cell up to `sky_tile` (then 16 px) whose four corner seeds missed was sky, and an island
  between seeds was never cast. `sky_tile` is now 8 for both, and `dilate` applies to seeds
  cast at spacing <= `dilate_below`: islands lit settled 34.0 -> 43.7, unlit 37.8 -> 45.5.
  City does not follow the "more complete" rule above any more: at 480x270, settled, `cluster`
  gives 40.7 dB unlit and 34.9 lit with `pathtrace` (dilate brought it up from 33.7; `sky_tile`
  makes no difference there), while the current `quadtree` gives 41.7 and 36.9, and a full GPU
  raycast with `pathtrace` gives 39.0 lit. Cluster discovery took about 80 ms per frame against
  quadtree's 11 ms. Neither `dilate_below` 4 or 8 (35.0 lit) nor `seed_step` 8 closes the gap.
  While moving, both miss the same amount (about 60 faces, 0.34% of pixels: the one frame
  that `faces~prev` lags behind).
  What the two changes cost (480x270, idle machine, `direct`, medians of 3 interleaved runs):
  islands 18.7 -> 12.2 fps lit (-35%), city 12.0 -> 8.5 (-30%); the GPU side is unchanged
  (+2..3% lit). On islands, `dilate = 0` gives back 15.5 fps, so seed dilation costs ~21% and
  the smaller sky tile the rest. Both stay the defaults for completeness; `dilate = 0,
  sky_tile = 16` is the fast setting.
- Drawing discovered faces one quad each disagrees with a ray about which face owns a pixel
  whose centre lies on, or within float error of, the shared edge of faces smaller than a
  pixel. In a distant forest (`far`, faces ~0.6 px at 480x270, ~460 blocks away) that is ~0.13%
  of pixels: `discover-gpu` 33.4 dB lit / 35.1 unlit, while its audit finds no visible face
  missing from the set and no `quadtree` setting changes the score. The error falls
  quickly as resolution rises (960x540: 38.9 dB, 1920x1080: 46.0). Rasterising greedy meshes has
  fewer edges and the same trend (44.3 / 46.6 / 49.3). This is sub-pixel aliasing, not missing
  faces; neither answer is wrong and the reference's ray decides. It stays below the floor at
  the sweep's resolution.

## Approximate techniques

- `splat-gpu`: each pixel of a point's footprint takes the depth where its ray enters the
  block, or nothing where the ray misses it, so it shows what a ray cast sees: within
  0.1–0.8 dB of `raycast-gpu` lit and unlit (2026-11-21). Filling a sub-2 px point's whole
  footprint had grown every silhouette by a pixel (islands lit 21 dB); a square footprint at
  the block centre's depth, capped at 8 px, had left near surfaces full of holes (10–26 dB).
- `columns-gpu`: no overhangs, caves or trunks (PSNR ~15–36 dB on terrain-like plans; 4–21 dB
  on the stress worlds, where little is a heightmap), 27 MB of GPU memory. Against a
  reference given the world as a heightmap (its declared lack of volume) it still scores
  ~17–20 dB (480x270: islands 19.1 unlit, 17.4 lit; terrain 20.4 / 17.9, where the heightmap
  changes nothing): Voxel Space's own approximation. Each screen column marches the
  horizontal direction of its centre ray and fills row spans with the cells it crosses, so
  under a pitched camera vertical edges stay vertical on screen instead of converging, and
  rows take a neighbouring cell near every edge; on detailed ground (foliage) that is a
  pixel's shift everywhere. Exact only for a level camera; making it exact otherwise is a
  per-pixel heightmap ray cast, another technique (2026-11-22).

## Shading

- `pathtrace` with `bounces = 0` reproduces `direct`.
- Running deferred shading on a second GPU, a frame behind, is possible (`plans/two-gpus.toml`)
  for ray casting, the face drawers and the splats: the hit records cross between the GPUs.

## Metrics

PSNR treats any resampling as a ~30 dB loss on this content; MS-SSIM is the better tuning
target across resolutions. Tuning for MS-SSIM keeps shadows and lowers the resolution; tuning
for PSNR does the opposite.

## Open directions

- Hardware ray tracing with ray-tracing pipelines (closest-hit shaders) instead of ray queries.
- A brick-level beam prepass.
- Discovery seeded from the previous frame's face set; one persistent kernel for all levels.
- A denoiser and jittered primary rays for `pathtrace`; emissive blocks.
- Mesh shaders, GPU-driven occlusion culling (Hi-Z), and GPU meshing.
- LOD for the rasterisers and the trees, and LOD that reflects edits.
- Baked or flood-filled lighting against ray-traced shadows on the quality / speed curve.
