# Ideas backlog

The user's ideas on volumetric rendering, each with analysis and a verdict. Nothing here is
implemented unless it says so; ask before implementing.

Context from measurements (October 2026): CPU traversal is instruction-bound, not
memory-bound. A DDA step costs about 80 cycles and 220 instructions, and cache misses are at
most ~3% of the time, so **fewer steps** and **fewer instructions per step** are what pay.
On the GPU, rasterising greedy meshes is very cheap (0.26 ms terrain unlit). Discovery has
to beat that to pay off.

---

## 1. Morton-ordered occupancy with a popcount prefix array

**Idea.** Store occupancy as one `u64` per 4³ region, with the regions in Morton order. A
parallel array holds, for each mask, the total popcount of all masks before it. An aligned
cube of 2^k regions is a contiguous Morton range `[a, b)`, so it is empty exactly when
`P[a] == P[b]`. That is two small loads and one compare, at every level of the hierarchy,
from one flat structure.

**Analysis.**
- **Correctness.** It is exact. Aligned power-of-two cubes, not only powers of 4, are
  contiguous in Morton order, so every level from 4 blocks to a whole chunk comes from the
  same array.
- **More levels for free.** If the bits inside each `u64` are themselves Morton-ordered, a
  2³ sub-cube is one byte, so `(m >> 8j) & 0xff == 0` tests it. The levels are then:
  voxel (bit), 2 (byte), 4 (`m == 0`), 8, 16, 32 (prefix compare).
- **It is also a rank structure.** `P[i] + popcount(m_i & below(bit))` is the rank of a
  solid voxel. Block ids can therefore be stored compressed, solid voxels only, in Morton
  order. Footprint shrinks with no extra index, and the id lookup on a hit is O(1).
  gpu-tree64 does this per node; this does it flat.
- **Width.** A per-chunk prefix fits in `u16`: a 32³ chunk has 512 masks and at most 32768
  solid voxels. Cross-chunk emptiness stays in the chunk table, so the whole structure is
  1 KiB of prefixes plus 4 KiB of masks per chunk. Loads for the coarse levels hit a few
  hot entries.
- **Cost of edits.** An edit rewrites the prefixes after it (≤512 per chunk) and shifts the
  compressed id array. Both are cheap per chunk.
- **Fit with the measurements.** It goes straight at the measured bottleneck, steps per
  pixel. `cpu-simd-hier`, with just one extra level (8³), took 20–34% fewer steps and ran
  up to 29% faster. This gives every level, and each test is a compare rather than a branchy
  descent. BMI2 `pdep`/`pext` make Morton encoding a few instructions on Zen 5.

**Verdict.** The strongest traversal idea on the list. Prototype it as a cpu-simd variant
first, where the hardware counters can confirm both steps per pixel and instructions per
step. Then do a GPU version.

**Result (implemented: `cpu-simd-prefix`, `Prefix` in `renderers/cpu-simd-mem/src/repr.rs`).**
The structure is as described: Morton-ordered bricks, Morton-ordered bits inside each
`u64`, a `u16 prefix[513]` per chunk in one cache-aligned allocation with the masks, and
compressed solid-only block ids addressed by rank. The probe goes bottom-up (bit, byte =
2³, word = 4³, prefix compare for 8³ and 16³) and stops at the first occupied level. An
exhaustive test checks that every claimed empty cube is empty, that the probe never
under-skips, that ids read via rank are correct, and that decoding round-trips.

Medians of 3 interleaved repeats, CCD-pinned, 1080p unlit, hardware counters:

| scene | variant | fps | cycles/px | instructions/px | steps/px |
|---|---|---|---|---|---|
| terrain | u16 (cpu-simd's layout) | 17.4 | 1986 | 4299 | 19.4 |
| terrain | hier (one 8³ level) | **19.6** | **1728** | 3772 | 15.6 |
| terrain | prefix | 18.3 | 1894 | **3649** | **13.3** |
| edits | u16 | 26.6 | 2718 | 6238 | 28.6 |
| edits | hier | **36.4** | **1972** | 4627 | 19.2 |
| edits | prefix | 35.4 | 2043 | **4029** | **14.0** |
| menger | u16 / hier / prefix | 48.6 / **58.5** / 53.6 | 692 / **570** / 636 | 1411 / **1203** / 1279 | 5.6 / 4.2 / **3.8** |

- **Steps:** fewest of any variant, 31–51% below u16 and 15–27% below hier.
- **Instructions:** fewest of any variant on terrain and edits.
- **Speed:** faster than cpu-simd's layout everywhere, but 3–9% behind `hier` on cycles. A
  non-unit step goes down the traversal's general path (a floor on all three axes), so
  bigger skips do not cost the same as unit steps. Prefix replaces many cheap steps with
  fewer expensive ones.
- **Memory:** world data 20% smaller on terrain (200.6 vs 251.8 MB), because ids are
  stored for solid voxels only. Peak RSS was higher (411 vs 336 MB), probably transient
  build allocations; not investigated.
- **Image:** 2-cell steps round differently at grazing edges: ±0.1 dB PSNR, in both
  directions, so not a loss of accuracy. Without the 2-cell level the image is identical
  to u16 and hier.

Fixes that mattered along the way:
- Building in one Morton-order pass (rank order equals Morton order, so masks, prefixes
  and ids come out together) cut world-update time from 7.2 to 2.0 ms per frame on
  terrain.
- A matching Morton-walk decode for edits took edits from 29.4 to 35.4 fps.

Tried and reverted:
- A branch-free probe (sum of the monotone level tests) was slower: it always pays every
  load.
- Putting masks and prefixes in one allocation made no measurable difference, but was kept.

**What would unlock it:** make large steps as cheap as unit steps. That means a
hierarchical DDA that keeps per-level `tMax` incrementally instead of re-flooring on every
coarse step. Then the step reduction should turn into speed. A GPU version is the other
open test.


**User's clarification (traversal).** The intended traversal was incremental from the
start: a hierarchical DDA that keeps per-level state (tMax per level) and moves between
levels incrementally, rather than re-deriving position on every coarse step.
`cpu-simd-prefix` therefore tested the data structure under an unsuitable traversal; the
real test of the design is still to come (see PLAN.md).

**Result 2: the incremental-level traversal (`trace_incremental`, knob `traversal`).**
The ray keeps a cell at the current level with per-axis exit times `tmax` and cell sizes
in t (`tdelta`). Every move is incremental; nothing is re-floored:
- *step:* a classic DDA step at the current level;
- *ascend:* on axes where the cell is the first child in the ray's direction, `tmax` gains
  `tdelta`; then `tdelta` doubles. Ascents are tried only after a step crosses the next
  level's boundary, because otherwise the parent is the one already known to be occupied;
- *descend:* `tdelta` halves, the midplane exit is `tmax − tdelta`, and the child is the
  second one along an axis exactly when the entry time has passed the midplane.

It is written against one storage query, `Repr::occupied(level, cell)`, with `Repr::LEVELS`
saying which levels a representation answers directly (u16/bits/palette: 1³, 4³; hier: 1³,
4³, 8³; prefix: 1³ through 16³). Chunk-level emptiness comes from the chunk grid. So every
`cpu-simd-*` variant can run it: a first piece of the storage/traversal split of §12. Knobs:
- `traversal`: `refloor` (the old one), `incremental` (exit times by adds), or
  `incremental-exact` (exit times recomputed from the integer cell: one multiply, no drift);
- `skip2`/`skip4`/`skip8`/`skip16`: which levels to use.

Tests:
- `occupied` is checked exactly at every claimed level;
- both incremental variants agree with the refloor traversal on 12,000 random rays per
  representation over a multi-chunk world (negative coordinates, a missing chunk, a uniform
  chunk, axis-aligned and diagonal rays), including restricted level sets. Zero rays
  differ.

Medians of 3 interleaved repeats, CCD-pinned, 1080p unlit, 200 frames. The counters split
the work into DDA steps, ascent tests and descents per pixel:

| scene | variant | fps | cycles/px | instr/px | DDA steps | ascent tests | descents | PSNR |
|---|---|---|---|---|---|---|---|---|
| terrain | hier refloor | 19.5 | 1691 | 3859 | 15.4 | – | – | 63.4 |
| terrain | hier incremental | **21.5** | **1434** | 3692 | 14.9 | 4.1 | 1.5 | 60.3 |
| terrain | hier exact | 20.5 | 1533 | 3703 | 15.0 | 4.2 | 1.5 | 63.4 |
| terrain | prefix refloor | 17.9 | 1914 | 3843 | 13.1 | – | – | 63.3 |
| terrain | prefix incremental | 21.0 | 1483 | 3871 | **12.7** | 6.0 | 2.4 | 61.3 |
| terrain | prefix exact | 20.3 | 1552 | 4017 | 12.7 | 6.0 | 2.4 | 63.2 |
| edits | hier refloor | 35.9 | 1977 | 4681 | 18.8 | – | – | 64.4 |
| edits | hier incremental | **36.8** | **1835** | 5005 | 18.6 | 6.5 | 2.0 | 62.4 |
| edits | hier exact | 34.7 | 1956 | 4997 | 18.6 | 6.6 | 2.0 | 64.4 |
| edits | prefix refloor | 35.1 | 2046 | **4188** | 13.6 | – | – | 64.4 |
| edits | prefix incremental | 34.1 | 1974 | 5473 | **13.4** | 9.5 | 3.7 | 62.9 |
| edits | prefix exact | 32.8 | 2075 | 5628 | 13.4 | 9.5 | 3.7 | 64.4 |
| menger | hier refloor | **61.3** | **527** | **1186** | 3.8 | – | – | 59.2 |
| menger | hier incremental | 59.6 | 520 | 1313 | 3.8 | 1.9 | 0.6 | 57.8 |
| menger | hier exact | 56.6 | 550 | 1315 | 3.8 | 1.9 | 0.6 | 59.2 |
| menger | prefix refloor | 54.8 | 605 | 1281 | 3.5 | – | – | 59.2 |
| menger | prefix incremental | 50.3 | 628 | 1653 | 3.4 | 2.7 | 1.0 | 58.3 |
| menger | prefix exact | 48.0 | 661 | 1695 | **3.4** | 2.8 | 1.0 | 59.2 |

(The refloor "steps" are its probe-and-step iterations, which are comparable to DDA steps.)

Terrain **lit** (shadow rays use the same traversal; the step counters cover primary rays
only): hier 15.1 → 16.4 (incremental) / 15.7 (exact) fps; prefix 14.2 → 16.1 / 15.4 fps.
PSNR: 58.2 refloor, 57.5–57.7 incremental, 58.1–58.2 exact.

What it shows:
- **Each move is much cheaper,** as intended: about 170–210 instructions per occupancy test
  against 240–370 per refloor step. Prefix gains most, because its big skips no longer pay
  for re-flooring.
- **But there are more tests.** Every boundary crossing tries an ascent, and near surfaces
  most fail; every descent re-tests. With a level at every power of two, prefix crosses a
  parent boundary every couple of steps: on edits it makes 9.5 ascent tests and 3.7
  descents on top of 13.4 steps.
- **Net:**
  - terrain (long rays over open space): wins for both, prefix +13–17%, hier +5–10%;
  - edits: about even for hier, prefix loses 3–7%;
  - menger (short rays, 3–4 steps): loses 3–12%, because the per-ray overhead
    (initialisation, first ascents) is not amortised.
- **Prefix vs hier:** under this traversal prefix still does not beat hier; it comes within
  2% on terrain. With exactly hier's levels ({1³, 4³, 8³}), prefix is 7% slower than hier.
  Its per-level test (Morton index, two prefix loads) costs more than hier's single summary
  bit, and its extra levels only just make up the difference. Every level helps: removing
  any one of 2³, 8³ or 16³ makes prefix slower (terrain: 20.1 fps with all levels, 19.5
  without 2³, 19.1 with only {4³, 8³}, 16.6 with only 8³).
- **Float drift is real.** Accumulating exit times by adds costs 1.5–3 dB PSNR (about
  0.002–0.004% more bad pixels: grazing edges). Recomputing from the integer cell removes
  it exactly, at 3–7% speed. The reference traverses in f64.

Tried and reverted: delaying ascents for N steps after a descent (`ascent_delay`). It cut
prefix's ascent tests by up to 30% at N = 8, but the extra steps ate most of that, and the
bookkeeping cost about 10 instructions per step even at N = 0 (hier exact: 20.5 → 19.6 fps
on terrain).

**What would help next** (not done):
- a cheaper prefix test: keep the Morton index incrementally as the cell moves instead of
  re-encoding it per test;
- testing the next coarse level only when the current level has been empty for a while,
  without per-step bookkeeping, for example by unrolling the step loop;
- a GPU version (PLAN 1b / 6′), where ALU-heavy re-flooring costs differently, and where a
  per-pixel ascent test is a memory latency rather than an instruction count. *(Done: see
  Result 3.)*

**Result 3: on the GPU (`gpu-compose`, PLAN 6′).** The prefix structure is a GPU storage
module: per pool slot, the 512 Morton masks and the 513 prefixes (5.1 KiB), with block ids
still read from the shared pool. Rank-compressed ids are not done on the GPU yet. GPU ms
per frame, 1080p, RTX 5060 Laptop, medians of 3 interleaved repeats (hier: 2):

| scene | mode | gpu-dda | bricks + refloor | hier + refloor | prefix + refloor | bricks + incr | prefix + incr |
|---|---|---|---|---|---|---|---|
| terrain | unlit | 0.942 | 0.857 | 0.764 | **0.748** | 4.51 | 4.02 |
| terrain | lit | 1.317 | 1.203 | 1.103 | **1.070** | 6.13 | 5.77 |
| edits | unlit | 0.714 | 0.653 | 0.528 | **0.506** | 2.27 | 1.90 |
| edits | lit | 1.004 | 0.927 | 0.784 | **0.746** | 3.32 | 2.95 |
| menger | unlit | 0.355 | **0.323** | – | 0.331 | 1.70 | 2.16 |
| menger | lit | 0.631 | **0.578** | – | 0.602 | 3.54 | 4.60 |

All of them render the reference image equally well (same PSNR as gpu-dda in every row).

- **Under refloor, prefix is the fastest GPU walk on open scenes:** −13% vs bricks on
  terrain, −23% on edits, and ahead of hier too. The GPU pays memory latency per step
  rather than instructions, so prefix's bigger skips pay off there, unlike on the CPU. On
  menger (short rays) it is even with bricks.
- **The incremental traversal is 3.5–8× slower on NVIDIA.** This is not the algorithm: on
  the AMD iGPU (RADV) the same shader is only 1.27× slower than refloor, which is about
  the CPU's ratio. And on NVIDIA itself, `view = "steps"` (which writes only the
  iteration count) runs it in 1.18 ms against refloor's 0.85, with nearly the same
  iterations per pixel (29.9 vs 28.8). The ~3.3 ms extra appears only when the ray's
  result is consumed; even consuming just the hit flag triggers it.
  - Ruled out, one at a time: dynamic vector indexing, the chunk cache in private
    globals, writing the result struct inside the loop, returning from inside the loop,
    unbounded inner loops, `continue`, a flattened one-test-per-iteration loop, runaway
    rays (`max_steps` 256 vs 2048 changes nothing), and extra live state.
  - Still open. A register-allocation or reconvergence decision in NVIDIA's compiler is
    the most likely cause; seeing it needs the pipeline statistics (registers, spills),
    which wgpu doesn't expose.
- The CPU-side cost of the side data matters: building prefix data for edited chunks costs
  +0.7 ms per frame on the edits plan after a one-pass Morton-order build (it was +2.0 ms).
---

## 2. Greedy meshing across block types with a per-cell block-id lookup

**Idea.** Merge coplanar exposed faces regardless of block type. The fragment shader finds
the cell under the fragment and looks up that cell's block id (from a tiny per-quad id
texture), then samples that block's texture.

**Analysis.**
- **What changes.** Today `mr-mesh` stores one block id per quad, so it only merges faces
  of the same type. Merging on exposure alone means merging a binary mask per plane. That
  is the "binary greedy meshing" case: much faster meshing using bit operations, and fewer
  quads.
- **A simpler variant than a per-quad texture.** The fragment already knows its world
  position, so it can look up the id in the voxel data the GPU renderers already hold for
  shadows. That needs no per-quad texture allocation. The per-quad id texture is still the
  better choice when the voxel data is not resident.
- **Cost.** One extra, coherent memory lookup per fragment. Quad count drops most on
  mixed-material surfaces like terrain. Per-cell data such as AO or light could share the
  same lookup.
- **Caveats.**
  - Bigger quads make culling coarser.
  - Translucent or non-cubic blocks would need their own mesh path. None exist today.

**Verdict.** Cheap, exact and likely a clear win for the raster renderers and the discovery
back-ends. A good first experiment, as a `merge = "exposure"` knob on gpu-raster.

**Result (implemented: `mr_mesh::Merge::Exposure`, gpu-raster knob `merge`).** The
fragment looks the block up in the voxel world. The cell is clamped to the quad, so
silhouette fragments never read a neighbour. Image quality is identical, or up to +0.4 dB
on menger and random (fewer T-junction cracks). Same run, 3 interleaved repeats:

| scene | quads block → exposure | unlit GPU ms | lit GPU ms |
|---|---|---|---|
| terrain | 527k → 523k (−0.7%) | 0.269 → 0.297 | 1.340 → 1.356 |
| menger | 1.93M → 1.62M (−16%) | 0.930 → 0.812 | 2.273 → 2.008 |
| random | 2.11M → 1.73M (−18%) | 1.239 → 1.188 | – |
| edits | 443k → 439k (−1%) | 0.302 → 0.406 | 0.976 → 0.999 |

On terrain the surfaces are already mostly one block type per region, so there is little
left to merge, and the lookup (a hash probe plus a pool read per fragment) costs ~10%
unlit. It pays off on mixed-material geometry. The default stays `block`. A cheaper lookup
would pass the voxel slot per chunk instead of hashing per fragment. Binary
(bitwise) greedy meshing, which exposure merging makes possible, would mainly cut CPU
meshing time.

---

## 3. Temporally refined visible-face buffer

**Idea.** Keep a buffer of visible faces across frames. Each frame:
- rasterise the buffer;
- cast rays into the uncovered ("black") areas;
- during raster, record which faces covered no pixels, and prune them before the next
  discovery.

**Analysis.**
- **Pruning is cheap and exact.** Render the set into a visibility buffer of face ids, and a
  compute pass marks the faces that appear. Keep unseen faces for a few frames (hysteresis)
  so sub-pixel faces don't flicker in and out.
- **The gap: black areas are not the only place new faces appear.** A disoccluded face can
  appear *in front of* a face already in the set, for example a pillar emerging from behind
  a wall as you strafe. Those pixels are covered, just by the wrong, farther face.
- **Where new faces can appear.** Visibility changes continuously with camera motion, so a
  newly visible face can only show up in these places:
  - next to an occlusion boundary (a depth discontinuity) of the current set, within the
    parallax that the camera motion can cause;
  - at the screen border;
  - in edited regions;
  - in uncovered pixels.

  So cast rays in a band around depth discontinuities, sized by parallax, plus the other
  three. That turns "rays in black areas" into a rule with a real argument behind it.

**Clarification from the user.** "Black spaces" means the regions a frame reveals that
were not visible the previous frame (disocclusions), not literally black pixels. They are
usually small but can exceed half the screen, so whether the method pays off depends on
measuring real use.

**Unresolved case (user).** New geometry that should appear in front of faces already in
the set: those faces are visible *through* where it should be, so the area is never
treated as revealed and the new geometry is never discovered. Part of this is
event-driven and solvable: block edits and chunk streaming are known to the host, so the
screen projection of an edited block or newly loaded chunk can be added to the revealed
area. Geometry that appears for other reasons has no such event, which is the open part.

**Verdict.** The right next step for gpu-discover and cpu-discover. It is also what both
discovery reports recommended. A still camera costs almost nothing, and motion costs rays in
proportion to newly revealed area.


**Result (implemented in `gpu-discover`, knob `incremental`, off by default).** Per
frame:
1. **ID raster.** Last frame's face list is rasterised at the current camera into a
   face-id texture plus depth.
2. **`mark`.** Every face covering a pixel is stamped as seen, and seen faces survive.
   This is the user's prune-unseen rule. An unseen face *inside* the view is kept for
   `keep_unseen` frames (64); one *outside* it expires after `keep_frames` (8).
3. **`need_raw`.** For each level-0 tile (with a 1-px border), the tile needs discovery
   if it has an uncovered pixel that was not sky last frame. The sky mask is reprojected
   by rotation only, since sky is at infinity. It also needs discovery if an occlusion
   edge can reveal more than `parallax_px`. The estimate is reveal ≈ f·T/near ·
   (d_max − d_min) on reverse-Z depth, accumulated over the frames since the tile was
   last discovered, so sub-pixel reveals add up.
4. **`need_final`.** Reveals are dilated by their radius in tiles. Changed chunks'
   screen rectangles (edits, streaming) and a 1-in-`refresh` round-robin are added.
5. **Discovery.** Level 0 casts and classifies only the flagged tiles.
6. **`sky_update`.** After the final raster, uncovered pixels become next frame's sky
   mask. Every uncovered pixel that was not already known sky forced its tile through
   discovery, so uncovered after discovery means confirmed sky.

Measured against full re-discovery (same renderer, unlit, 1080p, audit on):

| scene (camera speed) | full GPU ms | incremental GPU ms | rays/px | missed px | PSNR |
|---|---|---|---|---|---|
| terrain, walking (~0.09 blocks/frame) | 2.03 | **1.35** | 0.235 → 0.040 | 0.002% | 53.76 → **54.14** |
| terrain, 1/5 flight speed | 2.03 | **1.68** | 0.235 → 0.088 | 0.001% | 54.08 → **54.27** |
| terrain, flight (~2 blocks/frame) | 2.03 | 2.30 | 0.235 → 0.176 | 0.0003% | 53.60 → 53.64 |
| streaming (fast line) | 2.05 | 2.42 | 0.244 → 0.216 | 0.0001% | 58.43 → 58.61 |
| menger (orbit) | 1.68 | 2.00 | 0.206 → 0.153 | 0.0001% | 51.69 → 51.47 |
| room (static camera) | 0.60 | 0.60 | 0.052 → **0.005** | 0 | identical |
| terrain walking, lit | 4.27 | **3.61** | 0.235 → 0.040 | 0.002% | 50.92 → 51.05 |

**What the numbers say.**
- Walking or slower: incremental wins clearly, with up to 83% fewer rays and 33% less
  GPU time, at equal or better quality. Per-frame motion shrinks as the frame rate rises,
  so at high fps even fast movement looks like this regime.
- Fast flight or orbits: it loses 13–19%. The fixed cost (ID raster, mark and need
  passes, about 0.4 ms at 1080p) is not recouped. The tiles it skips are the cheap ones
  (flat footprints, sky); the tiles it must redo are the depth edges, where refinement is
  deepest.

**Lessons from building it.**
- Sky must be remembered (by direction), or every sky tile is re-cast each frame. This
  took the static room from 51% to 6% of tiles (the refresh rate).
- A per-frame parallax threshold misses slow motion: sub-pixel reveals accumulate.
  Accumulating by tile age fixed it.
- Pruning by "unseen" must spare sub-pixel faces still in view (keep 64), but expire
  out-of-view faces fast, or the set balloons (streaming went 298k → 718k faces).

**Still open.**
- The user's unresolved case (new geometry in front of already-visible faces) is covered
  for edits and streaming by the chunk rectangles. Other causes rely on the refresh.
- The ~0.4 ms of fixed overhead could shrink: fuse `mark` into `need_raw`, read depth at
  quarter resolution for the edge test.
- The edge test is two-sided. Revealed area lies on the background side, along the
  parallax direction, so it could flag about half as much.

**User's follow-ups.**
- *Small accumulated reveals* are fine in practice; nobody would notice them.
- *Pruning:* agreed it must spare sub-pixel faces still in view and expire out-of-view
  ones fast.
- *Integration:* a strength of the method is that its output is an ordinary face or
  triangle list, so it slots into a traditional raster pipeline.
- *Discover only as needed.* Even incremental, the face count changes every frame, which
  is unnecessary. Discovery should run only on events:
  - a camera pan, where only the strip entering at the screen edge is new (pure rotation
    changes no visibility);
  - a visible chunk loading in;
  - camera movement that brings loaded geometry into view.
- *Greedy meshing on top* of the discovered set could cut the triangle count.

**My notes.** The churn comes from my design: the ID raster, the 1-in-16 rolling
refresh, and sub-pixel faces dropping out and being rediscovered. An event-driven version
would work like this:
- with no camera change and no world updates, skip discovery and redraw the list;
- for a pan, discover only the edge strip, sized from the rotation angle, with no ID
  raster;
- for translation or chunk loads, use the existing parallax and chunk-rectangle rules;
- keep the set stable with hysteresis.

A stable set is also what makes an incremental greedy merge worthwhile, re-merging only
chunks whose discovered faces changed.

**Result 2: event-driven frames (implemented: `gpu-discover` knobs `events`,
`still_holes`, `view`).** With `incremental` on, the host classifies each frame:

| kind | when | work |
|---|---|---|
| static | camera pose identical, no world changes | no discovery; the last face list is redrawn |
| still | camera only rotated, and/or the world changed | survive counts faces in view as seen; no seen-marking, no parallax test |
| moving | camera translated | the incremental passes |
| full | first frame, teleport (> 64 blocks), setting change | everything |

Other changes made along the way:
- *Change areas are exact.* Edits mark the screen tiles of the edited blocks (not their
  32³ chunk), loads mark their chunk, and the areas go to the GPU as a per-tile mask
  instead of a list capped at 64 rectangles.
- *Fewer full frames.* Boxes behind or beside the camera used to force a full frame
  whenever any corner was behind the camera. This also helps plain `incremental`.
- *Holes are not sky.* After the raster, an uncovered pixel that was neither sky last
  frame nor in a tile discovered this frame is a hole, not sky. Its tile is discovered
  next frame, instead of the hole being remembered as sky forever.

GPU ms, 1080p unlit, 200 frames (spin: 300), medians of 3 interleaved repeats, audit on:

| scene | full re-discovery | incremental | events (`now`) | events (`late`) |
|---|---|---|---|---|
| room (static camera) | 0.599 | 0.599 | **0.035** | **0.035** |
| room, lit, animated lights | – | 0.920 | **0.314** | – |
| terrain spin (rotation only) | 2.196 | 1.229 | 1.200 | **0.915** (PSNR 42.6 vs 50.4) |
| terrain flythrough | **2.075** | 2.444 | 2.442 | 2.450 |
| streaming | **2.082** | 2.748 | 2.501 | 2.750 |
| edits (orbit + edits) | **1.717** | 1.902 | 1.898 | 1.896 |

Images were identical to plain incremental in every row except `late` on spin (PSNR and
missed pixels in the table). The streaming difference between `now` and `late` is run
noise: both run the same passes there.

What it shows:
- **A still camera is nearly free:** 17× less GPU time (0.6 → 0.035 ms), 3× with lit
  shading and moving lights. In the GUI, where the camera often stops, this is the
  common case.
- **Pure rotation does not leave visibility alone on a pixel grid.** Geometric
  visibility is unchanged, but pixel centres move, so a face too small to have been
  sampled can land on one. Skipping hole detection (`late`) leaves 1-frame holes:
  0.054% missed pixels and −8 dB. The face-id raster (`now`) is needed for exact
  quality, and then a still frame costs about what a moving one does. Faces moving onto
  pixel centres *in front of* known faces are caught only by the refresh, which
  therefore stays on in still frames.
- **Exact change areas cut work but not time.** Full frames went from 70% to 0%
  (edits), 83% to 0% (streaming) and 23% to 0% (terrain). Tiles needing discovery went
  from 98% to 71%, 92% to 40% and 57% to 41%. Rays per pixel fell 5–14%. But GPU time on
  moving scenes barely changed: it is dominated by the fixed passes (face-id raster,
  need passes, the final raster), not by rays. Full re-discovery stays 10–24% faster on
  moving scenes.
- *What the `tiles` view showed immediately:* in forward flight nearly every ground
  tile is flagged by the parallax test, which is why incremental cannot win there. Also,
  the round-robin refresh forms vertical stripes, because the tiles per row are a
  multiple of 16.

Diagnostic views (`view` knob, live):
- `tiles`: why each tile was discovered (white full, blue edited block or changed
  chunk, cyan entering strip, red uncovered or hole, yellow parallax, green refresh,
  dark skipped);
- `rays`: every ray cast this frame;
- `churn`: faces new to the set this frame, in red.

Counters: `frame_kind`, `frames_*_pct`, `faces_new`, `faces_dropped`.

**What would cut moving-frame time next:**
- fuse `mark` and `need_raw`, and read depth at quarter resolution for the edge test
  (both already noted above);
- a one-sided parallax test (reveals lie on the background side, along the motion);
- skip the face-id raster on moving frames by reprojecting last frame's depth instead.
---

## 4. Skip space using last frame's ray distances plus world heuristics

**Idea.** Start each ray at, or near, the hit distance its pixel had last frame. Also use
world knowledge to skip space.

**Analysis.**
- **What last frame proves.** It proves the segment from the *old* camera to the old hit
  was empty. A new ray is safe up to the first point where it leaves the union of those old
  segments, a star-shaped empty region around the old camera. With motion, that is not the
  reprojected depth itself.
- **A conservative start.** Take the minimum old depth over a neighbourhood (for example
  3×3, wider for faster motion) and subtract the camera translation. This is the same kind
  of proof gpu-beam needed.
- **Why it could matter here.** Unlike gpu-beam's empty-space skip, which saved nothing,
  this skips the near-surface steps. Those are most of the ~20 steps per pixel on terrain,
  so the potential gain is large.
- **The risk.** A start that is too late misses a new near occluder: an edit, or something
  revealed by motion. So it needs the conservative bound, and every edit must invalidate the
  start distances.
- **World heuristic that is exact: max-height mipmaps.** For terrain-like worlds, keep a 2D
  max-height pyramid per column. A ray above the max height of a region skips that whole
  region. This is a known, very effective accelerator for heightfields (Tevs et al. 2008),
  cheap, and it also gives sun shadow rays an immediate early-out.

**Verdict.**
- **Max-height mips:** high value, exact. Try them.
- **Temporal start distance:** promising but heuristic. Prototype behind a knob, with the
  audit and reference comparison measuring its errors.

---

## 5. A self-directed discovery query with local-memory "heads"

**Idea.** N heads grow across the frame. Each needs only its previous and current step to
decide where to go next, so its state stays hot in cache. The goal is to discover the whole
frame with no gaps and no head's trail crossing another's. The work happens in clip space,
fixed-point, not pixel space. When a ray hits a block, the area of that block's face is
removed from the frame. No block can hide entirely inside another face's projection, so
nothing is missed.

**Analysis.**
- **Lemma: true.** No block nearer to the camera can project *entirely* inside the
  projection of a farther block's face.

  *Proof.* Let the face lie on axis plane P. A block that occludes part of it lies between
  the camera and P. That block's face parallel to P faces the camera, on a parallel plane
  P1 closer to the camera. Centrally project that face from the camera onto P. It becomes
  an axis-aligned square of side d(P)/d(P1) > 1. A square of side > 1 cannot fit inside the
  unit face. ∎
- **But single containment is not enough.** Removal is applied for many faces, and the
  *union* of several removed faces can hide a block. Take a pillar in front of a wall: if
  three adjacent wall faces are hit first and removed, they can together cover the pillar's
  whole projection, and it is never sampled.
- **A fix for the coplanar case.** The pillar's projection onto the wall plane is a square
  of side > 1, so it contains an integer lattice point, which is a corner of a wall face.
  So **cast a ray at every corner of every removed face**. Any block hidden by a coplanar
  union is then hit.
- **The mixed-plane case is open.** When a block is covered by faces on different planes
  (for example a wall face and a floor face), I don't have a proof yet. Until there is one,
  the fallback is a conservative frustum-occupancy check before removing a face.
- **Heads with O(1) memory.** Two heads with no shared state cannot know they are about to
  overlap. "No trail hits another" needs either shared state (a coverage mask or face-claim
  bits) or an up-front partition. A clean design:
  - Give each head a disjoint clip-space region (a tile, or a Hilbert-curve segment) that it
    never leaves.
  - Inside its region, a head walks the face adjacency. From a hit face, cast one ray just
    beyond each projected edge. That ray lands either on the coplanar neighbour (continue
    the walk) or across a silhouette (start a new walk there).
  - Faces that straddle a region boundary are found twice, which is a cheap de-duplication.
  - A head's working state is then its current face and the edge it came from, as you
    wanted. The per-region coverage mask is the one shared thing it needs to know when the
    region is done.
- **Fixed-point clip space: agreed.** With integer edge functions and a top-left rule (the
  way GPU rasterisers work), adjacent face projections tile the plane exactly, so there are
  no gaps and no double coverage. That is the "no spots missed" property, made precise.
- **Not pixel space, but still tied to a resolution.** The work needs a resolution floor:
  with infinite precision, distant faces shrink without limit and the set to discover
  becomes unbounded. Faces that cover no sample point never affect the image. So the
  natural lattice is the output's sample grid, with sub-pixel fixed-point precision for
  placing the face edges.

**User's corrections.**
- *Heads.* Two heads with no shared state *can* avoid overlapping, if their movement rules
  make contact mathematically impossible. My objection only applies to heads that move
  freely; a rule that keeps trails disjoint by construction (for example monotone or
  interleaved fronts) needs no communication. The open task is to find such a rule that
  also guarantees full coverage.
- *Limit.* The work is bounded by a distance or work limit, with some degradation in the
  distance, not by a pixel resolution. Exactness is then relative to that limit, and the
  degradation is the LOD question.

**Verdict.** Worth building as a CPU research prototype. The lemma and the corner-ray rule
give a real completeness argument for the coplanar case. It belongs on the CPU, where the
idea of keeping each head's state hot in cache fits naturally. GPU threads would serialise
on the walks. Prove or test the mixed-plane case first; the reference comparison and the
audit would catch misses.

---

## 6. Early-out ray casting

Already done:
- primary rays stop at the first hit;
- shadow rays stop at the first occluder;
- rays stop when they leave the loaded region, or at `max_steps`;
- chunk, brick and region skips.

Still open:
- the max-height early-out from §4, for sky and sun rays;
- shadow rays toward a light that end at the light's distance rather than the region edge
  (check every renderer does this);
- skipping shading work for pixels whose ray missed.

---

## 7. Discovery in clip space, in signed fixed point

**Idea (user).** Run discovery in a `[-1, 1]²` coordinate space, independent of
resolution and aspect ratio, which maps directly onto signed fixed-point coordinates.
Rendering is not changed by this: it just receives a list of faces or triangles, as if
from any other method.

**My notes.** Agreed: the refinement should follow geometry, not the pixel grid. Pixels
remain the yardstick for checking the result, because the image is point-sampled there.
"Complete" means nothing that covers a sample point is missing, within the distance or
work limit. Fixed-point edge functions with a top-left rule tile the plane exactly, so
there are no gaps and no double coverage.

---

## 8. Self-balancing Morton breadth-first iteration

**Idea (user).** With N execution units, split the screen into N rectangles. Each unit
subdivides its cells and decides which regions need descent. When all have finished a
level, their buffers of cells needing descent are appended into one large buffer, which
is redistributed evenly across the units for the next level. A detailed region therefore
cannot hold other units idle.

**My notes.** gpu-discover already has this level-synchronous structure: a global queue
of tiles to split, rebalanced across all threads at each level. Two differences from the
idea address its measured costs:
- *Morton order of the work.* The queue now fills in arrival order, so deeper-level rays
  scatter across the screen. That incoherence is why 0.2 rays/px cost more than gpu-dda's
  1.0. Sorting or appending in Morton order would keep neighbouring rays together.
- *Barriers between levels.* Each level is a separate pass that waits on its slowest ray.
  Rebalancing per level is the gentle form; a persistent kernel with work stealing is the
  strong form.

On the CPU this maps onto rayon with a Morton-sorted work vector per level.

---

## 9. A modular renderer library with no performance penalty

**Idea (user).** We have many techniques but have explored few combinations. Assemble
renderers from small modules, with no performance penalty for doing so.

*Storage/traversal boundary (user).* Each storage mechanism implements every traversal
mechanism's preferred query method, through a conversion if necessary, even when that is
slow. Good pairings are fast and bad pairings are slow, without odd architecture.

**My notes.**
- *How to make it free.*
  - On the CPU, Rust generics are monomorphised, so a separate optimised copy is
    generated per combination. `cpu-simd-mem` (generic over its world representation)
    already works this way.
  - On the GPU, shader modules are concatenated into one shader before compilation, as
    `shader_source(...)` already does, so the driver optimises across module boundaries.
- *The query interface.* It suits this well: a trait (CPU) or WGSL function set (GPU)
  with a default, slow conversion per query, which a storage overrides where it has a
  fast path.
- *The hard parts.* Choosing stage boundaries that don't block optimisation, and the
  explosion of combinations. The bench could sweep combinations automatically.
- *Where to start.* Make storage plus traversal swappable first, since about eight of
  the GPU ray casters differ mainly there.

---

## 10. Face-cluster discovery

**Idea (user).**
1. A camera ray hits a block.
2. Neighbouring exposed faces that share edges with it are consolidated into a cluster.
3. Each face casts exactly one ray to each of its corners not shared with a face already
   tried in the cluster, to confirm the face is visible. The persistent state is only the
   set of already-tried faces on the boundary, and avoiding duplicate casts roughly
   halves the rays.
4. The cluster's whole covered area is blacked out in the discovery field.
5. More rays are cast until the whole space is blacked out.

*Sky.* Rays that hit nothing would otherwise be full detail everywhere, so use a sparse,
low-density pass. It is fine if something small takes a few frames to appear through
that sparse pass.

*Far away.* Where faces are smaller than a pixel, consolidate groups of faces and treat
them as one face: a form of LOD.

**My notes.**
- *Coverage.* Shared corners are cast once, not skipped, so interior corners of a cluster
  are tested. With the §5 lemma, any occluder in front of a coplanar cluster projects to a
  square of side > 1 on its plane, so it contains a lattice corner, which is cast. That
  catches a pillar in front of a wall.
- *Corner-ray budget.* Near, large faces give big savings: a few corner rays verify a
  large area. Where faces shrink below a pixel, per-corner rays cost more than per-pixel
  rays, which is exactly where the user's far-field consolidation (LOD) takes over.
- *Output.* A cluster is already a greedy-meshed polygon, so it can go straight to the
  rasteriser.
- *Sky.* An exact alternative is to prove a sky region empty by testing its view cone
  against the coarse emptiness levels of the §1 prefix hierarchy. Given the user's
  tolerance for a few frames of delay, the sparse pass is the simpler choice.

---

## 11. Back-to-front order to turn off depth testing

**Idea (user).** If the face buffer sent to the GPU is ordered back to front, depth
testing can be disabled. That is easy here because discovery tends to add faces front to
back. The order only needs to be local; no global sort is needed.

**My notes.**
- *When it is exact.* It needs every pair of faces whose projections overlap to be drawn
  far to near, so "local" has to cover all overlapping pairs.
- *An exact order for voxels.* Grid-aligned cells have a view-dependent order that is
  always correct: octree traversal with each child order chosen by the camera's octant.
  With Morton-ordered storage, that is the Morton index XOR'd with the camera's octant
  bits, which ties back to §1.
- *The trade-off.* Without depth testing the GPU loses early-z rejection, so overdraw is
  shaded. It pays off where depth-buffer bandwidth dominates, or when shading is cheap or
  deferred (a visibility buffer). It is worth one measured experiment.

---

## 12. The renderer's composition as a knob

**Idea (user).** Split rendering cleanly into parts (storage, algorithm, backend, and so
on), each behind an interface that is uniform, not restrictive, and costs no speed. The
renderer's composition then becomes a knob: implementing one more part yields many new
combinations for free.

**My notes (decision: adopt, built in stages).**
- *The stages.*
  - Storage answers a fixed set of queries: occupancy, block id, the largest provably
    empty region at a level, and chunk iteration for meshers. Each query has a slow
    default built from the others, and a storage overrides what it can do fast.
  - Algorithm: per-pixel traversal, discovery or meshing, written only against those
    queries.
  - Shading: unlit, lit or path-traced, from a hit surface.
  - Backend: CPU (rayon/SIMD) or GPU (wgpu compute, raster, ray queries).
- *No speed penalty.*
  - On the CPU, specialise only the hot inner loop and pick the composition at runtime
    outside it. Monomorphising every combination would bloat compile times.
  - On the GPU, assemble and compile the WGSL modules lazily, per combination actually
    used.
- *Where it can't be uniform.*
  - Backends: storage needs a per-backend form (a CPU layout and its GPU upload).
  - Fused techniques: hardware RT's driver-built BVH, and rasterisers, which need meshes.
    Meshing is simply the conversion from storage to triangles, with an honest cost.
- *As a knob.* Hot swapping already exists, so the GUI and the tuner can change
  composition like any other knob. The bench measures the slow pairings and the tuner
  learns to avoid them.
- *Order.* Start with the storage + traversal slice, after the incremental-level prefix
  traversal (PLAN.md step 1) has shown what a traversal needs to ask of storage.

**Result (implemented: `gpu-compose`, the storage + traversal slice).**
- *Interface.* A storage is a WGSL module providing:
  - `S_LEVELS`, the levels it answers directly;
  - `s_occupied(slot, level, cell)` for top-down walks;
  - `s_probe(slot, cell)`, returning (block, largest empty cube), for skip-size walks;
  - `s_block(slot, cell)`;
  - optional per-slot side data with a CPU builder.

  A traversal is a WGSL module providing `ctrace(o, d, max_t) -> Hit`, written only
  against that interface and the shared chunk table. Shadow rays use the same composed
  walk.
- *Modules so far.* Storages `bricks`, `hier` and `prefix`; traversals `refloor` and
  `incremental`. That gives 6 combinations from 5 modules. Adding `hier` took one
  28-line WGSL file and a 10-line builder, and it works with both traversals.
- *Knobs.* `storage` needs a restart (its side data is built per chunk). `traversal`,
  `exact`, `skip*` and `view` recompile the shader live.
- *Cost of the split: none measured.* `bricks` + `refloor` is gpu-dda's algorithm on
  gpu-dda's data. It runs 7–9% faster than gpu-dda, because gpu-dda also carries LOD
  code.
- *A side finding that changed other renderers.* Ten GPU renderers shaded with
  `select(texel, shade_lit(...), lit)`. WGSL's `select` evaluates both sides, so unlit
  frames paid for lighting and shadow rays: unlit and lit GPU times were identical.
  Branching instead cut unlit GPU time by 25–35% (gpu-rtx 5%) with identical images.
  Unlit GPU numbers recorded before 2026-10-09 are therefore inflated.
- *Not yet:* LOD in gpu-compose; converting the other ray casters (tree64, bitmask,
  clipmap, svo-dag) into storages; rank-compressed ids for `prefix` on the GPU;
  composition as a tuner dimension (it is a knob, so the tuner can already flip
  `traversal`).

---

## 13. A mode for integrating arbitrary polygons (animated meshes)

**Idea (user).** Besides lit and unlit, add a mode that renders arbitrary polygons, such
as animated meshes, together with the voxel scene. Renderers implement it optionally. The
tuner chooses only among renderers matching the requested mode: lit, unlit, lit + mesh,
unlit + mesh. In the GUI, make lighting and meshes two latching buttons.

**My notes (proposed design).**
- *Mode flags.* Represent the mode as independent flags (lighting, meshes) rather than
  four named modes. Each renderer declares the flags it supports. The bench, GUI and
  tuner filter on them.
- *World.* New world updates for mesh upload and per-frame transforms (rigid at first,
  skinning later), plus scripted mesh animation in plans so runs stay reproducible.
- *Reference.* cpu-reference must intersect triangles, or quality cannot be measured.
- *Raster renderers.* Draw meshes into the same depth buffer: cheap and exact.
- *Ray casters.* Rasterise meshes first, then cast rays only up to the mesh depth at
  each pixel. This is exact for primary visibility, and is one shared compositing module
  (§12) that gives every ray caster mesh support.
- *Lit mode.*
  - Baseline: voxels shadow meshes (shadow rays from mesh fragments through the voxel
    world), and meshes shadow each other.
  - Later, optional: meshes shadowing voxels, which needs a mesh BVH in the shadow-ray
    path (hardware RT has it free).
- *GUI.* Two latching toggles, **Lit** and **Meshes**, replacing the Lit/Unlit buttons.

---

## Suggested order

The live plan, with estimates and progress, is in `PLAN.md`.
