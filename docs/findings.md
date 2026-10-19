# Findings

Measured on a Ryzen 9 9955HX and an RTX 5060 Laptop GPU, at 1080p on the terrain flight
unless noted. These numbers are indicative: GPU time for the same configuration has varied
between 1.7 and 3.0 ms across separate runs (boost clocks, power state), while interleaved
repeats (`run.repeats`) keep the spread within ±1%. Each scheme's entry in the
[encyclopedia](schemes/README.md) has the details.

## Storages and walks (GPU)

- `prefix` + `refloor` is the fastest GPU ray walk on open scenes (−13% on terrain, −23% under
  edits, compared with `bricks` + `refloor`). With LOD it is 22% faster.
- `distance` + `sphere` has the cheapest traversal (0.9 ms against 1.3 ms for `bricks`), but is
  twice as slow under heavy edits: edited chunks are walked cell by cell until their distance
  field is recomputed.
- `rows` + `rowrun` is 5–15% slower than `bricks` + `refloor`.
- `tree64` matches `bricks` in speed with about 15% of the VRAM. `svo_dag` deduplicates about 3×
  on terrain and has the smallest footprint. `clipmap` reads textures only (no hash table), and
  its slot wrap happens once per chunk.
- `incremental` is 3.5–8× slower than `refloor` on NVIDIA, for reasons not yet known (1.3× on
  AMD).
- LOD at the same far view distance (768 blocks): 136 MB of VRAM against 1037 MB at full
  resolution, and faster. The quality loss is concentrated in pixel-scale foliage in the
  distance.
- The `beam` prepass is exact but still a net loss: chunk-level skipping already makes empty
  space cheap.
- `dispatch = "fragment"` (the walk in a fragment shader) runs at the same speed as compute.

## CPU

- The `cpu` back end is about 1.5× the reference renderer; avoiding libm `floor` calls was
  the big win.
- SIMD packets do not help, and **not** because of memory: hardware counters show ~80 cycles
  and ~220 instructions per DDA step at IPC 2.6, with L2 misses costing at most 3% of the
  time. Shrinking the world data 7–16× (`bits`, `palette`) leaves cycles per pixel unchanged.
  `brickmajor` and `morton` halve L1 misses, but cycles do not drop, so they run 3–4% behind
  `bricks`.
- What helps is fewer steps: `hier` skips empty 8³ regions (20–34% fewer steps, up to +29%
  fps). `prefix` takes the fewest steps of all (−31 to −51%) but is 3–9% behind `hier` per
  pixel, because big steps cost more than unit steps in this walk. `incremental` with `exact`
  brings `prefix` within 2% of `hier` on terrain, but loses on short rays.
- `schedule = tiles` (screen tiles along a space-filling curve, 2D packets) is 4% faster than
  rows. Software prefetch only added instructions.
- Pipelines compiled for the machine (`compile = "native"`) are 2–13% faster than the generic
  build (`prefix` + `refloor` 11–13%): the generic x86-64 target lacks `popcnt` and wide
  vectors outside the hand-written AVX paths.
- `cpu-raster` is the closest match to the reference (64–99 dB), but its cost scales with the
  number of faces (Menger sponge: 8 fps). `cpu-voxelspace` gives the same image as
  `gpu-voxelspace` at about half its speed.

## Rasterisers and hardware ray tracing

- `naive` makes 2.6–18× more faces than `greedy`. `greedy` with `merge = exposure` (merging
  across block types) has 16% fewer quads and 13% less GPU time on menger, and is slightly
  slower on terrain.
- `visbuffer` is 1.5–2.5× faster than `raster` when lit with lots of overdraw, and halves lit
  time for `cubes`. `gbuffer` is a little slower than `visbuffer`.
- Lit-mode shadow rays of the rasterisers walking the composed storage are 7–18% faster than
  their former fixed walk.
- `gpu-rt` traces fastest on the RT cores, but its BLAS rebuilds make it CPU-bound when chunks
  stream in or change.

## Discovery

- `gpu-discover` (`quadtree`): 0.05–0.34 rays per pixel, audit misses ≈0–1 pixel per frame.
  It is not yet faster than rasterising everything: the levels run one after another, and late
  rays are incoherent. `holes = coverage` (only re-discover where something may have changed)
  uses 83% fewer rays and 33% less GPU time at walking speed with equal or better quality, but
  is 13–19% slower in fast flight. A still camera skips discovery entirely (17× less GPU time).
  `visbuffer` / `gbuffer` drawing is 2–4× faster than `raster` when lit on terrain, and a
  little slower unlit.
- `cpu-discover`: `cluster` is more complete than `quadtree` on every scene at about the same
  number of rays (terrain 0.24 rays per pixel, 51.0 against 43.8 dB; menger 51.5 against
  26.4 dB), at 2.5× its CPU time. Pipelined (discovering frame N+1 while the GPU draws frame N),
  one frame of latency costs 0.1–0.3% of the pixels while moving. CPU discovery (25–60 ms) is
  the bottleneck.

## Approximate techniques

- `gpu-splat`: edges fatten (PSNR ~20–27 dB).
- `gpu-voxelspace`: no overhangs, caves or trunks (PSNR ~15–36 dB), 27 MB of GPU memory.

## Shading

- `pathtrace` with `bounces = 0` reproduces `direct`.
- Running deferred shading on a second GPU, a frame behind, is possible (`plans/two-gpus.toml`).

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
- An HDR output pipeline.
