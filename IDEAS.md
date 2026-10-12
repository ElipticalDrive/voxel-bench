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

**Result (implemented: `cpu-simd-prefix`, `Prefix`, now `components/src/storage/prefix/cpu.rs`; the renderer is now the composition `cpu: prefix`).**
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

**Result 3: greedy merge of the discovered set (PLAN 2b; gpu-discover `merge = rows`,
view `quads`).**
- *How it works.* One GPU pass over S. A face whose predecessor along its face's u axis
  differs starts a run, and the run takes the following faces with the same signature,
  up to 64. "Differs" means not in S, another block, or another pattern of coplanar
  neighbours above and below.
- *Why runs agree on their v-neighbours.* That makes each long edge of a run either
  entirely a seam (the plane continues beyond it) or entirely a silhouette.
- *No "re-merge only changed chunks" bookkeeping.* Merging the whole set every frame is
  one cheap pass.
- *Texturing.* Merged quads look up the block of the cell under each fragment (§2), so
  seams texture like a ray caster.
- *T-junctions are the hard part.* A long quad edge passes through the corners of
  shorter neighbouring quads, and pixel centres on that line can fall between both.
  - Without countermeasures: ~100 crack pixels per frame on terrain, −4 dB.
  - Growing every edge closes them but bleeds over silhouettes. Even 1/128 px of growth
    gave more wrong pixels (750–1400) than the cracks it fixed, because silhouettes far
    outnumber seams.
  - Growing seam edges only, by 1/16 px, brings it to ~25 extra crack pixels per frame
    (baseline ~15) and −2 dB. The rest is at convex corners, where a merged face meets
    perpendicular faces whose corners fall mid-edge. Growing those would bleed again.

| scene | mode | faces → quads | GPU ms, none → rows | PSNR, none → rows |
|---|---|---|---|---|
| terrain | unlit | 272k → 140k (1.94 per quad) | 1.91 → 2.32 | 52.7 → 50.6 |
| terrain | lit | same | 4.10 → **3.70** | 49.8 → 47.6 |
| edits | unlit | 54k → 36k (1.52) | 1.33 → 1.50 | 55.9 → 54.3 |
| edits | lit | same | 2.18 → 2.19 | 50.0 → 49.1 |

- *Speed.* The merge pass costs ~0.35 ms (three hash lookups per cell per test). Lit
  rasterisation on terrain gains 17% from fewer, larger quads (tiny quads waste shading
  lanes), so lit terrain is 10% faster overall. Edits gain nothing (1.5 faces per
  quad), and unlit is slower everywhere.
- *For the integration use (a face list handed to a normal raster pipeline):* about half
  the quads, at the price of T-junction handling. Default off.
- *Next if pursued:*
  - 2D (rectangle) merging, which has more T-junctions;
  - a CPU-side merge of a stable set;
  - closing convex-corner cracks by drawing those edges' neighbour faces unmerged.
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

**Result (PLAN step 3, gpu-discover).** Measured before changing anything (terrain
flight, full re-discovery, 1080p): rays 0.92 ms, classify 1.00 ms, clear/survive 0.24
ms, raster 0.15 ms. The deep levels (2-px and 1-px tiles) dominate.
- *Morton order: no gain.* Level 0 was enumerated along a Z-curve, and queue appends
  were made order-preserving (a workgroup prefix sum instead of a local atomic), so deep
  levels inherit the locality. GPU time was unchanged within noise (3.07 vs 3.04 ms
  full, 3.69 vs 3.61 ms incremental, same run). Kept as the `morton` knob, off by
  default; the ordered appends stay because they make the face list's order
  deterministic.
- *Why: the passes, not the order.* Per level, rays cost 1.2–2.9 ns each, against 0.45
  ns per pixel for gpu-dda including shading. Level 0's 33k rays take 0.095 ms, about the
  same as level 1's 97k. Each level pass lasts at least as long as its slowest ray plus
  the gap between dependent passes, which adds up to roughly 0.6 ms of a 1.9 ms frame.
  This is the "barriers between levels" cost. A persistent kernel (step 3b) is what would
  remove it.
- *What did pay: inline small footprints.* The footprint pass gave each accepted tile a
  64-thread workgroup, but deep tiles have 1–4 cells. Footprints of up to `inline_fp`
  (8) cells are now expanded by the classifying thread:
  - level-2 jobs: 38k → 4.6k;
  - level-2 classify: −31%;
  - edits: full 1.72 → 1.35 ms, incremental 1.90 → 1.58 ms;
  - images identical.

  At 32 cells the single threads diverge and level 1 slows down, so 8 is the default.

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
    already worked this way; it became the `cpu` back end.
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

*Components (2026-10-10, PLAN phase 4k).* Discovery is now two back ends, `gpu-discover`
(visibility `quadtree`) and `cpu-discover` (visibility `quadtree` or `cluster`), over any
storage of their side: the same discovery, now combinable with prefix, hier, tree64 and
the rest. Same rays, faces and images as before; quadtree discovery over `prefix` is
8–10% faster on the GPU, over `hier` 7% faster on the CPU.

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

**Result (implemented: `cpu-cluster`, i.e. cpu-discover with `algorithm = cluster`;
PLAN step 5).**
- *Seeds.* A pixel lattice is walked coarse to fine (`seed_step` 16 → 1 px). Every
  lattice pixel not yet covered casts a ray.
- *Flood fill.* From the hit face, a fill crosses shared edges to exposed faces on the
  same plane. Each face is confirmed by rays to its four corners. Corner visibility
  belongs to the point, so each lattice point is cast once per frame, whichever faces
  (of any plane) share it.
- *Covering.* A face with all corners visible is confirmed: it covers the pixel centres
  of its projection, and the fill continues through it. A face with an occluded corner
  still joins S but stops the fill.
- *Sky.* Two rules, knob `sky`:
  - `corners` (default): a lattice cell at spacing ≤ `sky_tile` (16) is sky when all four
    corner seeds miss;
  - `sparse`: the user's rule, where a single miss covers its cell.
- *Clip-space fixed point (§7)* was not needed. The pixel lattice is the yardstick (an
  image is point-sampled at pixel centres), and coverage is an exact pixel-centre test
  against projected faces.
- *A stronger lemma, and its limit.* An axis-aligned square of side > 1 that overlaps a
  unit face contains one of *that face's* corners: per axis, an interval longer than 1
  that overlaps [0, 1] contains 0 or 1. An occluder's near face projects onto the plane
  as such a square. But its full shadow is the hull of its near and far faces, and a
  sliver of that hull can overlap a face without covering a corner. So completeness is
  not proven. It holds in practice:
  - a unit test (floor, wall, pillar in front of it, a floating block; three
    viewpoints; two seed spacings) finds zero missing pixels;
  - with `sky_tile = 1` (no sky shortcut), terrain has zero missed pixels in the audit.

Synchronous (`pipelined = false`), 1080p unlit, audit on, against the quadtree
(cpu-discover defaults):

| scene | quadtree rays/px | cluster rays/px | missed px quadtree → cluster | PSNR quadtree → cluster |
|---|---|---|---|---|
| terrain | 0.254 | **0.242** (seeds 0.106, corners 0.135) | 0.0062% → **0.0038%** | 43.8 → **51.0** |
| menger | 0.114 | 0.150 | 0.68% → **0** | 26.4 → **51.5** |
| room | 0.027 | **0.025** | 0 → 0 | 60.5 → 60.5 |

- **More complete than the quadtree on every scene, at about the same rays.** Menger is
  the striking case: the quadtree's plane-coherent tiles accept tiles that hide the
  sponge's holes, while clusters only cover what their corners prove.
- **About one corner ray per confirmed face.** Shared corners are cast once: terrain
  confirms ~144k faces with ~0.135 rays/px of corner rays.
- **Sky is what decides cost.** With the `sparse` rule at 8 px: 0.26 rays/px but 0.15%
  missed (thin horizon features vanish within a cell). Sampling every sky pixel: 0.89
  rays/px but zero misses. The four-corner rule gets both: few rays and almost no
  misses.
- **The quadtree is faster on the CPU:** 100 ms per pass on terrain against 40 ms. The
  fill runs in ~460 waves per frame (a plane needs as many waves as its radius in
  faces), and corner and face bookkeeping is about a million hash operations. Already
  done: fast hashing, parallel classification per wave, small waves kept on one thread
  (135 → 101 ms).
- **Pipelined (default), lit, fast terrain flight:** both lose ~1% of pixels to the
  frame of latency. Cluster 35.6 dB / 1.08% missed, quadtree 30.6 dB / 1.38%.
- *Views:* `view = clusters` (one colour per cluster) and `view = kinds` (confirmed
  green, an occluded corner yellow, carried over grey). Yellow traces every silhouette,
  where occluders cut a plane.
- *Not done yet:*
  - far-field consolidation (super-faces where faces are sub-pixel): terrain did not
    need it for the ray count, but distant faces still cost a corner ray each;
  - a scanline fill (row runs per plane, the previous row's corners in an array) to
    remove the hash traffic;
  - a GPU version.

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

**Result (implemented: gpu-discover `order = painter`, view `overdraw`).**
- *An exact order.* The Morton-XOR order above is not exact in general: the child order
  depends on which side of each node's split planes the camera lies, which differs
  between nodes. A simpler key is exact. Along any ray, the voxel walk only moves away
  from the camera's cell on each axis, so a cell's Manhattan distance from the camera
  cell strictly increases along the ray, and an occluder always has a smaller distance.
  Drawing by decreasing distance is a correct painter's order, and since the key is a
  small integer, a GPU counting sort does it (histogram, one-workgroup scan, scatter).
  Images are identical to depth-tested drawing (same PSNR, zero missed pixels).
- *Measured (terrain, full discovery):*
  - overdraw is 1.03 fragments per covered pixel, because discovery yields almost
    exactly the visible faces;
  - unlit: the sort costs ~0.12 ms (raster 0.15 → 0.28 ms) and saves nothing;
  - lit: painter is ~3% faster (2.15 vs 2.21 ms raster), from skipping the depth
    pre-pass.
- *With incremental mode it backfires.* The kept unseen faces raise overdraw to ~5 (p99
  12) on edits, against 1.3 for full discovery. Lit painter is then 56% slower (4.36 vs
  2.79 ms), because hidden faces get shaded with shadow rays. Depth order stays the
  default.
- *Side finding from the overdraw view:* incremental's `keep_unseen` (64 frames) keeps
  many hidden faces in occlusion-heavy scenes. The depth pre-pass hides the cost in lit
  mode, but the set is ~2× larger than needed (edits: 119k vs 57k faces).

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

**Design for replacing the monoliths (2026-10-10).** The user's goal: replace the
individual renderers with components assembled through interfaces into renderers, at no
performance cost.

*Component kinds.* A renderer is
**storage × visibility × (geometry) × shading × back end**:

| kind | what it does | interface |
|---|---|---|
| storage | holds the world, answers queries | required: `S_LEVELS`, `s_occupied(slot, level, c)`, `s_probe(slot, l) -> (block, empty cube)`, `s_block(slot, l)`; optional fast paths: `s_dist`, `s_row`, … |
| visibility | finds the surface per pixel or per face set | ray walks: `ctrace(o, d, max_t) -> Hit`; discovery: produces a face set; raster: draws geometry |
| geometry | converts storage to triangles for rasterisers | greedy mesh, naive faces, merged discovered faces (merge = rows) |
| shading | colour of a hit | unlit texel; lit with shadow rays (via `occluded` = the same traversal); path tracing |
| back end | where it runs | CPU (generics), GPU compute (WGSL modules), GPU raster, hardware RT |

*Contracts.*
- `s_occupied` may report "occupied" for an empty cube but never the reverse, and must be
  exact at the voxel level. Traversals descend until they reach exactness.
- `s_probe`'s cube must be truly empty.
- An optional query is never required: the assembler adds a generic fallback built from
  the required ones (`defaults.wgsl`). This is the §9 rule, "good pairings are fast, bad
  pairings are slow".
- Side data lives per pool slot in `aux` and is built by the storage's CPU builder.

*Why it costs nothing.*
- GPU: modules are pasted into one shader before compilation.
- CPU: Rust generics monomorphise per combination.
- Measured: `bricks + refloor` runs 7–9% faster than gpu-dda (no LOD code);
  `distance + sphere` 0.60 ms vs gpu-sdf 0.64 ms; `rows + rowrun` 1.02 ms vs gpu-bitmask
  1.02 ms.

*Inventory* (renderer → components; ✓ = available as a component now):

| renderer | storage | visibility | shading / back end | status |
|---|---|---|---|---|
| gpu-dda | bricks ✓ | refloor ✓ | lit, compute | composed: `bricks + refloor` |
| gpu-sdf | distance ✓ | sphere ✓ | lit, compute | composed: `distance + sphere` |
| gpu-bitmask | rows ✓ | rowrun ✓ | lit, compute | composed: `rows + rowrun` |
| cpu-simd-* (6) | u16, u8, bits, hier, prefix, palette (CPU `Repr` ✓) | refloor, incremental (CPU ✓) | lit, CPU | composed on the CPU side already |
| gpu-tree64 | 64-tree (own world) | tree descent | lit (own occlusion), compute | to port: storage + traversal |
| gpu-svo-dag | SVO-DAG (own world) | tree descent | compute | to port |
| gpu-clipmap | toroidal 3D texture | DDA | compute | to port: storage |
| gpu-beam | bricks | beam prepass + DDA | compute | to port: prepass as a visibility stage |
| gpu-dda-lod | bricks with LOD | refloor with LOD | compute | needs LOD in the storage interface |
| gpu-rtx(-bricks) | hardware BVH | ray query | hardware RT | fused: a back-end-specific storage + traversal pair |
| gpu-pathtrace | bricks | refloor | path tracing | to port: shading component |
| gpu-raster, -naive, cubes, visbuffer | via meshing | raster | lit, raster | to port: geometry + raster visibility |
| gpu-discover, cpu-discover, cpu-cluster | bricks / CPU grid | discovery (quadtree, cluster) | raster | partly: discovery is a knob in cpu-discover |
| gpu-frag-dda, gpu-hybrid | bricks | raster + fragment DDA | hybrid | to port: hybrid visibility |
| cpu-reference, cpu-raster, cpu-simd, cpu-simd-loc (4) | CPU grid | DDA, software raster | CPU | the reference stays a monolith on purpose |
| gpu-splat, gpu-voxelspace, cpu-voxelspace | point / heightmap representations | splat, column raster | — | different representations: own components later |

*Migration rules.*
- **New techniques are built as components only** (PLAN.md working agreement).
- Port by yield: components that add a storage, traversal or visibility shape first.
  Renderers whose parts already exist retire once a composed equivalent matches their
  image and speed; until then they stay as controls.
- *Next ports, in order of yield:* tree64 and SVO-DAG (storages with tree-descent
  traversals), clipmap (storage), the beam prepass (a visibility stage before any
  traversal), path tracing as a shading component, LOD in the storage interface.

*The user's target (2026-10-10):* no chooser of monolithic renderers at all. Renderer
pieces are organised in files; a renderer is assembled by choosing a back end and one
component per slot; the UI does not allow unsupported combinations. The monolith names
may survive only as legacy entries while porting continues. Phases:
1. Components crate, the GPU-compute back end, compositions in plans, CLI and GUI.
   **Done:**
   - `components/` (`mr-components`): one directory per piece, metadata, compatibility;
   - `backends/gpu-compute`;
   - `compose = { … }` in plans and `-r backend:storage+visibility` on the CLI;
   - the GUI chooser with disabled options and reasons.
2. The CPU back end (the same components on the CPU). **Done:**
   - `backends/cpu` (was cpu-simd-mem);
   - CPU implementations in `storage/*/cpu.rs` (bricks, hier, prefix, and CPU-only u8,
     bits, palette), `visibility/{refloor,incremental}/cpu.rs` and
     `shading/direct/cpu.rs`;
   - the six cpu-simd-* renderers retired;
   - all 12 CPU combinations render the reference image.
3. The GPU-raster back end. **Done** (2026-10-10), except discovery:
   - a `geometry` slot that only raster back ends have (`BackendInfo.slots`, with
     per-back-end defaults; ray-walk compositions leave it empty and reject it);
     components `greedy` (mr-mesh), `naive`, `cubes`, each a CPU mesher plus a small
     WGSL vertex interface (`G_VERTS`, `g_face`, `g_size`, `g_corner`);
   - raster visibility `raster` (forward, depth pre-pass) and `visbuffer` (ids, then
     compute shading), both built on one `face_hit` that turns a pixel and a face into a
     `Hit`, so the shading component `direct` is the same WGSL as on gpu-compute;
   - storage on raster is what the lit-mode shadow rays walk (refloor), so all five GPU
     storages apply;
   - meshes in every raster composition (visbuffer draws them after shading, against the
     kept depth);
   - gpu-raster, gpu-raster-naive, gpu-cubes and gpu-visbuffer retired.
   Discovery (`quadtree`, `cluster`) as raster visibility components is still to do.
4. Port the remaining unique pieces. **Done** (2026-10-10, PLAN 4a–4l): tree64,
   svo_dag and clipmap as storages (with region / DAG / texture storage kinds and a top
   level `s_top` in the interface); the beam as a `prepass` slot; path tracing as a
   shading component; LOD in the storage interface; hardware RT (`gpu-rt`), CPU raster,
   discovery (`gpu-discover`, `cpu-discover`), splats and Voxel Space as back ends. Same
   images as every former renderer; speeds equal or better except tree64 (−5%) and the
   CPU layout storages (−3–4%).
5. Remove the legacy section. **Done** (2026-10-10): only cpu-reference stays
   standalone; a compact stage grid (back end + its knobs, then each stage's component
   with its own knobs; unavailable choices greyed with the conflict on hover) replaces
   the chooser in the plan editor and the live session.

*Done on the GPU so far (gpu-compose, now gpu-compute).* Storages bricks, hier, prefix, distance, rows;
traversals refloor, incremental, sphere, rowrun. All 20 combinations render the
reference image like the originals (58.8 dB on terrain). The fallbacks make odd
pairings correct but slow: sphere without a distance field ~2 ms, rowrun with a
generated row 0.9–1.7 ms. New pairings worth noting: `distance + refloor` 0.73 ms,
`prefix + rowrun` 0.95 ms (terrain, single runs).

*Result (phase 3, the raster back end).* Every composed equivalent renders the same image
as its monolith (terrain unlit: greedy 53.6 dB, naive 55.5, cubes 55.4, exposure merge
53.6; meshes 63.4 / 71.1 dB). GPU time, terrain, 3 interleaved repeats, medians:

| composition (was) | unlit ms | lit ms |
|---|---|---|
| greedy + raster (gpu-raster) | 0.26 (0.27) | 1.11 (1.35) |
| greedy + visbuffer (gpu-visbuffer) | 0.31 (0.31) | 0.68 (0.71) |
| naive + raster (gpu-raster-naive) | 0.70 (0.63; ranges overlap, both min 0.63) | 1.99 (2.30) |
| cubes + raster (gpu-cubes) | 5.33 (6.16) | 10.92 (11.77) |
| cubes + visbuffer (new) | 5.38 | 5.76 |

Lit is 7–18% faster because the shadow rays now take the composed refloor walk (the
7–9% gpu-compute vs gpu-dda effect). Cubes are 13% faster unlit, likely from the shared
vertex code. The new pairing `cubes + visbuffer` halves cubes' lit time: their overdraw
no longer pays for lighting. On menger (single runs) `prefix + greedy + visbuffer` takes
1.15 ms lit against 2.03 for `bricks + greedy + raster`; how much of that is prefix and how
much visbuffer is not separated yet.

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

**Result (implemented: mesh mode core, PLAN step 7, plus gpu-raster as the first
renderer, 7b).**
- *Modes.* A pass mode is lighting plus meshes: `unlit`, `lit`, `unlit+meshes`,
  `lit+meshes` (`PassMode` in mr-core). Renderers keep receiving `RenderMode`, plus
  `FrameRequest.meshes`.
- *Opting in.* Renderers declare support with the `meshes` tag and receive geometry
  through `Renderer::update_meshes`, which does nothing by default. None of the 34
  renderers without mesh support needed a change. Benchmarks skip them in `+meshes`
  passes; the GUI draws their voxels and labels the tile "(no meshes)".
- *Scene.* `scene.meshes`: procedural box, sphere and torus with per-vertex sRGB
  colours, scattered copies (`count`, `spread`), and spin / orbit / bob animation, all
  deterministic in the frame index. Geometry is sent once; instances are sent whenever
  they change.
- *Semantics* (documented in `mr_core::mesh`):
  - interpolated sRGB vertex colours, with perspective-correct barycentrics;
  - flat normals on the side facing the viewer;
  - voxels and meshes occlude each other by distance;
  - lit uses the voxel lighting model, with voxels shadowing meshes;
  - **meshes cast no shadows yet**, neither on voxels nor on each other. That is a
    simplification of the proposal above, so the reference and every renderer agree
    now; mesh shadows are 7d.
- *Reference.* Two-sided f64 Möller–Trumbore against world-space triangles with
  bounding-sphere culling; the nearest mesh hit in front of the voxel hit wins.
- *GPU.* `mr_gpu::meshes` is shared mesh drawing for rasterisers: per frame, the CPU
  writes camera-relative triangles, which are drawn into the renderer's depth buffer
  with fill, depth-pre-pass and equal-depth pipelines. gpu-raster uses it. The ray
  casters (7c) can reuse it for "raster meshes first, rays capped at mesh depth".
- *Measured* (plans/meshes.toml, the voxel Cornell box with a tumbling torus, spheres
  orbiting the pillar and 12 bobbing boxes, 5,520 triangles, 1080p):
  - gpu-raster: unlit 63.4 dB (64.4 without meshes), lit 71.1 dB (72.7); GPU time
    0.21 → 0.23 ms lit, so meshes cost about 1 dB of edge pixels and almost no time;
  - in the live GUI end-to-end test, a probe in `lit+meshes` scored gpu-raster 60.9 dB
    and gpu-tree64 (no mesh support) 29.7 dB.

**Result 2: meshes for the GPU ray casters (PLAN 7c, `mr_gpu::meshes::MeshComposite`).**
- *How it works.* Before the ray caster's dispatch, the meshes are rasterised, fully
  shaded and with voxel shadows in lit mode, into a colour and depth target (bind group
  3). The shader casts each primary ray only up to the mesh's distance at that pixel
  (`mesh_t`: reverse-Z depth turned into distance along the ray) and shows the mesh
  colour where no voxel is nearer. That is exact for primary visibility, and capped
  rays trace less.
- *Cost of adding it to a renderer:* three shader lines, an extra bind group, one call
  before the dispatch; the mesh pass carries the frame's begin timestamp. The other
  renderers were converted mechanically from gpu-dda's change.
- *Supported:* gpu-dda (and gpu-dda-lod), gpu-compose (every storage × traversal),
  gpu-bitmask, gpu-sdf and gpu-beam.
- *Measured* (plans/meshes.toml, 2 interleaved repeats): every one scores 71.3 dB in
  `unlit+meshes` and 73.5 dB in `lit+meshes`, identical because the compositing is
  identical and their voxel images already match the reference. Meshes cost
  +0.01–0.05 ms.
- *Not yet:* gpu-tree64, gpu-svo-dag, gpu-clipmap and gpu-rtx keep their voxels in their
  own structures, so the shared voxel world has no data for the mesh pass's shadow
  rays: lit meshes would get no voxel shadows. They need their occlusion code in the mesh
  pass, so they stay without the `meshes` tag. gpu-pathtrace would need meshes in its
  bounces.

---

## 14. A library interface with a planner that learns this machine

**Idea (user, 2026-10-10).** Over all of this, a simpler library-style interface so the
project can be used as a renderer: the incoming scene, edits and so on are the input
(various data structures). Something like the tuner, but with persistent memory across
sessions about the user's machine, uses all the information it can to determine very
cheaply a decent render plan (composition and knobs). Ideally it keeps learning, the
whole time the user interacts with it, how best to render things on that machine. It
replaces (or extends) the tuner. Not to be implemented yet. Sweeping every combination
(PLAN, "plan sweep") gives it data about every combination's performance on this machine.

**My notes (feedback; nothing built yet beyond the data it needs).**
- *Precedent.* This is how FFTW's "wisdom", ATLAS, cuDNN's autotuner and Halide's
  autoscheduler work: plans measured on this machine, persisted, reused. A database
  query optimiser with a learned cost model is the closest analogue for picking among
  hundreds of plans from cheap scene statistics. Learning during use is a contextual
  bandit: the context is the scene and settings, the arms are compositions plus knobs,
  and the reward is speed under a quality floor.
- *Features, not scenario names.* The planner has to generalise from the sweep's
  scenarios to the user's scene, so it needs a cheap feature vector of the scene and
  request: resolution, view radius and loaded chunks, solid and surface fractions (how
  open or cave-like), camera speed and turn rate, edits per frame, lights, mesh
  triangles, lit or unlit. Observations record the scene only as its plan spec today.
  Recording measured features beside it is the next step for the data side.
- *Constraints first, then the objective.* Mode support, a quality floor against the
  reference, and VRAM and RAM budgets are filters. Frame time (mean and p99), latency,
  and later power are the objective. The catalogue already applies the quality floor.
- *Switching costs.* A knob change is cheap and live. A composition change rebuilds data
  structures and compiles shaders, which costs a hot swap (seconds). So the planner should
  pick the composition rarely (at scene load, or when the context drifts far) and tune
  knobs continuously. That suggests two levels: a learned model picks the composition, and
  the existing Bayesian tuner tunes its knobs.
- *Model.* Start with something small and honest about uncertainty, such as
  per-composition regression of log frame time on the features, or nearest neighbours
  over observations. Exploration during use must not cause visible stutter: try
  alternatives on a hidden standby lane (the live session already warms one for hot
  swaps), in idle time, or only when the model's uncertainty is large.
- *Noise and drift.* Laptop boost and thermals spread runs by ±5–8% (seen in the
  repeat tables). Driver updates change performance. Key observations by machine and
  driver (they carry both), weight recent ones more, and re-measure the incumbent
  occasionally.
- *Coverage.* One night sweeps every composition at default knobs over ten scenarios.
  The knob space is far too big to sweep, so knobs must be learned online or with
  targeted sweeps around the best compositions.
- *Library interface.* The `Renderer` trait plus `WorldUpdate` / `MeshUpdate` /
  `FrameRequest` is close to the input side already. A library needs:
  - in-process renderers (`isolation = "thread"` exists);
  - one canonical world copy on the library side, so a composition switch can re-stream
    the world into the new renderer (the live session does this for hot swaps);
  - **output into the caller's surface**: today frames are read back to host memory.
    GPU back ends would have to render into a caller-provided wgpu device and texture.
    Each back end creates its own device today, so this is the API decision worth
    making early.

## 15. Composing across back ends (a device per stage)

**Question (user, 2026-10-10).** Are renderers fully modular now? In the GUI each
renderer is one back end with its components. The wish: choose the algorithm and the
back end per stage, in any supported combination.

**Answer.** Not yet. A composition is one back end plus one component per stage; the
registry says which back ends implement each component, so the back end comes first.
- *Why.* The storage↔visibility boundary is a source-level interface: WGSL functions
  (`s_top`, `ctrace`) fused into one shader. That fusion is why the split costs nothing
  (IDEAS §12). A walk reads its storage every step, so the two must live on the same
  device. A device per stage only makes sense at coarse boundaries where little data
  crosses per frame:
  - geometry (CPU meshing) → raster (GPU): already how gpu-raster works;
  - discovery (CPU) → raster (GPU): cpu-discover is a hand-built cross-device pipeline;
  - visibility (GPU) → shading on another device: rarely worth it.
- *What it takes.*
  1. Model devices per stage: each component lists implementations per device (CPU,
     GPU compute, GPU raster, RT) instead of per back end.
  2. Define fused groups (storage + visibility always share a device) and data-level
     contracts at the cut points (a hit or visibility buffer, a face list).
  3. Add transfer stages (upload, readback, pipelining) and a small frame-graph executor
     that runs the groups on their devices. Today's back ends become those groups'
     executors.
  4. Change the GUI to a device picker per stage, with conflicts greyed as now.
- *Verdict.* An incremental refactor rather than a rewrite. The stage boundaries drawn so
  far are the right ones, and the registry and stage grid extend naturally. The real
  work is the data-level contracts at a few cut points and the executor. The sweep, the
  catalogue and the observations need no change: a composition just gets more fields.

## 16. Maximum control over combinations, minimal code of redundant purpose

**Goal (user, 2026-10-11).** Maximum control over which pieces combine, with as little code
of redundant purpose as possible. Different back-end implementations of the same feature
(a storage in Rust for the CPU and in WGSL for the GPU, greedy meshes exposed to a raster
pipeline and to a BLAS) are **not** redundant: they serve different back ends. Redundant
means the same purpose written twice for the same kind of back end. The question that
started it: will the current architecture make us write copies of components just so they
support every combination?

**This is one exploration of the problem, not a decision.** A wider first-principles
design (world + change log; representations with update costs; traversal kernels;
visibility strategies; surface sources; shading with inline or deferred placement;
history; post; an executor; per-stage timings; §18) was discussed alongside it, and §15 is
a related one. This section only assesses where the current structure duplicates and where
it limits combinations.

**Analysis (from the code, 2026-10-11).**
- *Protected: storage × traversal kernel.* They meet through a small source-level interface
  (`s_block`, `s_occupied`, `s_top`, `s_probe`, … and `ctrace`), with generic fallbacks
  (`storage/fallbacks.wgsl`) for optional fast paths. A new storage works with every kernel
  and vice versa: cost grows as storages + kernels, not storages × kernels. CPU and GPU
  versions of one storage or kernel are back-end implementations, not redundancy.
  Cross-cutting features (LOD, later material masks) touch each storage and kernel once:
  wide, but additive.
- *Not protected: everything after visibility.* Shading, drawing surfaces and the
  visibility buffer live inside each back end, so they are rewritten per back end, and a
  piece that lives inside one back end cannot be chosen anywhere else. Copies and missing
  combinations have the same cause.

  Redundant purpose (same purpose, same kind of back end):

  | purpose | copies | where |
  |---|---|---|
  | lighting (GPU) | `shade_hit` + 3 own | `shade_hit` (shading component) serves gpu-compute, gpu-raster, gpu-rt, gpu-discover; gpu-splat, gpu-voxelspace and cpu-discover's `raster.wgsl` light on their own |
  | lighting (CPU) | `shade` + 2 own | `shading/direct/cpu.rs` serves `cpu`; cpu-raster and cpu-voxelspace light on their own |
  | drawing a face quad (`face_axes`) | 3 | gpu-raster `main.wgsl`, gpu-discover `raster.wgsl`, cpu-discover `raster.wgsl` |
  | `block_at`, `pixel_ray` | 2 each | gpu-raster with gpu-discover; gpu-raster with gpu-voxelspace |

  Combinations missing because a piece is locked inside a back end:
  - visbuffer / gbuffer exist only in gpu-raster, though gpu-discover and cpu-discover also
    rasterise faces;
  - path tracing is offered by gpu-compute and gpu-rt only; gpu-discover's fragments have a
    hit and could use it;
  - the shading slot of gpu-splat, both voxelspace back ends and cpu-raster offers only
    their own lighting;
  - cpu-discover's lit-mode shadow rays walk its own GPU voxel structure, gpu-voxelspace's
    its own pool: not the composed storage.
- *How it grows.* Every feature after visibility costs one copy per back end that keeps
  its own version: transparency would touch ~7 lighting paths and 3 face drawers; each
  new lighting feature (AO, soft shadows, flood-fill light) or discovery variant (a new
  drawer, discovery with a visbuffer or path tracing) multiplies the same way.

**Direction.** Make the **hit record** the boundary after visibility, as `s_*` / `ctrace`
already are between storage and kernel:
1. *One shading implementation per kind of back end.* Every visibility strategy (ray
   walk, raster, discovery, splat, voxelspace columns) produces a hit (position, normal,
   block, face) and calls the shading component: `shade_hit` on the GPU, `shade` on the
   CPU. Fused, it is a function call (as gpu-compute does today); deferred, it is a
   visibility buffer. Moves the five own lighting paths onto it, and makes every shading
   component (direct, pathtrace, later AO, transparency) available to every strategy.
2. *One surface drawer* for face quads, used by gpu-raster, gpu-discover and
   cpu-discover; visbuffer and gbuffer then become choices for both discovery back ends.
3. *Shadow and secondary rays through the composed storage everywhere* (cpu-discover,
   gpu-voxelspace), so the storage slot means the same thing in every back end.
4. *A guard test:* no WGSL function defined in more than one back end's shaders, and every
   back end's lighting goes through the shading component. Catches backsliding.
5. Later, splitting the "visibility" slot into *traversal kernels* (refloor, incremental,
   sphere, rowrun, rayquery) and *visibility strategies* (ray cast, raster, discovery,
   splat, columns), so any strategy can use any kernel; discovery's own pieces (sampler,
   hole detection, retention policy, drawer) would become choices of their own.

**Verdict.** Worth doing in steps, before transparency and before the next round of
discovery work, which would both land on the duplicated parts. Step 1 is the largest
multiplier. Costs to measure: a deferred visibility buffer costs bandwidth against
shading at the hit, so placement (inline or deferred) should stay a choice; the steps
must keep each back end's speed (compare against the sweep). Not started; ask first.

**Status (2026-10-11): step 1 done** (PLAN progress log). The boundary is a `Surface`
(point, normal, block-local point, block, face) rather than a ray hit, so strategies that
do not walk a ray (splat's clamped point, voxelspace's slope normal) fit; ray walks build
it with `surface_of`. Every GPU back end lights through `shade_surface`; per-pixel shading
state (pathtrace's accumulation) is one shared piece (`shading::state`), and pathtrace
now runs on gpu-splat, gpu-raster and gpu-discover too. On the CPU, cpu-voxelspace moved
onto `direct::cpu::shade`; cpu-raster keeps the reference's f64 model, a fundamental
difference (precision; the f32 component cost 4–22 dB of lit PSNR), not a copy. The
lighting rows of the table above are resolved; `shade_lit` remains only as the direct
model's body and the mesh shader's lighting.

**Status (2026-10-11): step 2 done.** One face drawer (`visibility/faces.wgsl`, host side
`visibility/drawer.rs`) with face sources; `face_axes`, `block_at` (`s_block_at`) and
`pixel_ray` exist once; visbuffer and gbuffer are choices (`draw`) of both discovery back
ends.

**Status (2026-10-12): step 3 done.** Every shadow and bounce ray walks a storage
component: the heightmap became a component with a ray walk (CPU and GPU), cpu-discover's
GPU side walks a chosen GPU storage (`shadow_storage`), and the voxel world's own DDA is
gone.

## 17. The pipeline model, the requirements, and compiling on demand

**The user's model (2026-10-11), a picture of the level of genericity wanted, not a
design.** A *stage* is a device, the interface used to program it (an API; on the CPU, a
compiler), and a program and data model. Stages are joined by *communication machinery*,
elided when data can pass directly without moves or conversions. A *pipeline* runs from a
defined input interface to a defined output interface (these are not stages). A pipeline
is valid when every connection has a communication path both sides support.

**Requirements (the user's).**
1. No cost from composability while the pipeline runs. Compiling before it runs
   (at run time of the program) is fine.
2. No architectural opinion on hardware configuration (e.g. "one GPU only").
3. As much code reused as is practical.
4. As many combinations as is practical, grown over time with *adapters* ("transition
   code") that give a method a feature it lacks natively, even if slower. More
   combinations, not fewer.
5. Combinations without a valid path do not exist (greyed with the reason); nothing
   substitutes for them silently.
6. Compilation: a generic precompiled version of none, some or all variants, plus variants
   compiled on demand for the user's machine and cached. A user-triggered **break-in mode**
   spends as long as it needs to build the most optimised version for that machine;
   the generic builds keep the user free to choose when. `rustc` will ship with the final
   product. On the CPU this is option 2 of the discussion: generate Rust for a
   composition, compile it to a library, load it (in a worker process).
7. **Existing, well-supported compilers everywhere.** No compiler or JIT of our own (no
   Cranelift back end, no custom IR): `rustc` for CPU pipelines, also at run time; the
   driver (via wgpu / naga) for shaders; an existing shader module system (`naga_oil`,
   WESL) if text-joining strains. Options considered and rejected: a Cranelift JIT (a
   compiler for our own IR, less optimisation than `rustc`). Kept as a possible extra back
   end, not a replacement: running the WGSL components on the CPU through a software
   Vulkan driver (lavapipe), which makes every GPU component CPU-capable.

**Analysis.**
- *Adapters exist already.* `storage/fallbacks.wgsl` is one: it gives storages without a
  distance field an `s_dist` built from their occupancy queries, compiled in when the
  composition is built. Adapters must be compiled into the stage like this; a dynamic
  adapter on the hot path would break requirement 1.
- *A testable meaning of "no cost".* A composed pipeline runs as fast as a hand-written
  pipeline of the same pieces on the same devices (agreed by the user). Transfers between
  devices are not a cost of composability (the hand-written version pays them too). Measured once
  (`bricks + refloor` vs the former gpu-dda); it should be a standing check.
- *Graphs, not chains.* Shading reads the visibility result and also queries the storage
  (shadow rays); history feeds a frame's output into the next. Validity generalises: every
  input of every stage is connected to a compatible output.
- *Where today's code breaks requirement 2.* Each GPU back end creates its own device
  (`Gpu::new` in seven back ends), one per process. Devices should be resources handed to
  stages, so two GPUs or a CPU and a GPU are a configuration.
- *Where today's code limits requirement 6 on the CPU.* `backends/cpu` instantiates the hot
  loop ahead of time for storage (8) × traversal (3) × packet width (4); every new CPU
  option multiplies compile time and binary size. Compiling on demand removes the product.
- *Break-in mode has three layers, cheapest first:* compile for this CPU
  (`target-cpu=native`, LTO, constants specialised for the run: resolution, packet width);
  profile-guided optimisation with a short sweep as the training run; autotuning knobs and
  choosing compositions (the planner, §14). On the GPU the driver compiles anyway; the
  gains there are specialised constants and persistent pipeline caches.
- *Further requirements (suggested, agreed by the user 2026-10-11):* every newly compiled pipeline is
  checked against the reference before it is trusted; compiled variants are keyed by
  source hash, toolchain, CPU and driver; the generic build runs while the optimised one
  compiles, then hot-swaps (the live session already can); generated code runs in worker
  processes, so a bad build only kills its worker.

**Status (2026-10-11).** The user agrees this is the direction and agrees the condition:
grow it in steps, each useful on its own and checked against the zero-cost definition,
never as a rewrite; generalise an interface once two or three real techniques need it.
The step sequence is in PLAN.md ("Next: the architecture refactor"). Open questions to
settle along the way: the pieces after the split (§16 step 5), one capability model for all
interfaces or one per boundary, which data crosses between devices, when the CPU moves
to on-demand compilation, and how the GUI shows a deeper composition.

**The user's question (2026-10-12): can the CPU do what the GPU does, joining the parts
into one source before compiling so the abstraction disappears?** Yes; it already happens
ahead of time, and step 8 does it at run time.
- *Today, ahead of time:* the `cpu` back end is generic over the storage (`CpuBackend<R:
  Repr>`, one instantiation per storage) and the walk and shading are generic functions
  and closures (`trace_packet::<R, N>`, `shade` with closures), so `rustc` sees one
  composition at a time and inlines across the boundaries: the GPU's text-joining, done by
  monomorphisation. What stays at run time: a `match` on the traversal per ray (`occluded`,
  the incremental walk), the shadow mode and the knob values (levels mask, packet layout,
  view radius), all read from memory, and a generic x86-64 target with hand-selected
  AVX2 / AVX-512 entry points. Cost: every combination is compiled whether used or not
  (storage 8 × traversal 3 × packet width 4 ISA variants), so adding parts multiplies build
  time and binary size.
- *What compiling one composition on demand adds* (PLAN step 8): only the chosen
  combination is compiled (the product stops growing with every part); run-time values
  become constants, as `const LOD` / `EXACT` / `LEVEL_MASK` already do in the GPU shaders
  (dead branches and loops over known counts fold away); `target-cpu=native` for this
  machine instead of the generic target; later PGO with a short sweep as training. The
  gain from removing abstraction itself is small (monomorphisation already removed it);
  the gain from specialisation and the native target is unknown until measured (often
  0–15% on such loops). First measurement before building anything: compile today's
  `cpu` back end with `-C target-cpu=native` and the knobs as constants, interleaved
  against the generic build.
- *Costs:* `rustc` per new composition (seconds to a minute; cached by source hash,
  toolchain and CPU; the generic build runs meanwhile), shipping a toolchain, and running
  generated code in a worker process.
- *The user's intent (2026-10-12):* every pipeline compiled at run time eventually (the
  combinations will outgrow precompilation; the only way to full zero cost), `rustc` in a
  background or worker process on the user's machine. A common subset ships precompiled;
  break-in runs PGO and the rest on those; the learning planner's runtime feedback picks
  which further pipelines to precompile. Not to build now; prepare for it in the
  architecture. What that asks of the refactor steps:
  1. *A composition is data:* back end, components and the knob values that get baked in,
     canonical and hashable (the cache key with toolchain and CPU). `Composition` plus
     options is most of it; knobs need a declared kind: *specialisable* (a constant in
     generated code, e.g. levels mask, packet width, shadow mode, bounces) or *live*
     (changes without recompiling, e.g. tile size, exposure).
  2. *CPU components as a library a generated crate can instantiate:* generic over their
     parts (as `CpuBackend<R>` is over the storage), with specialisable knobs reachable
     as const generics or associated consts, so generated code is little more than
     `type P = Pipeline<Prefix, Refloor, Direct, Consts>;` and an entry point. No
     component may depend on being linked into the main binary in a non-generic way.
  3. *One boundary for every pipeline:* the `Renderer` interface, callable across a
     process boundary (frames, updates, knobs, counters), so precompiled, generated and
     generic builds are interchangeable and hot-swappable (the GUI already hot-swaps).
  4. *Observations the planner can learn from:* runtime usage and per-stage timings on
     top of sweep observations, machine-tagged (already the observation format).
  5. *Trust:* each newly compiled pipeline checked against the reference before use
     (already a requirement).

## 18. A first-principles design (what the pieces are)

**Context (2026-10-11).** Asked to design as if no architecture existed, from the problem
space. This is the long-term picture the steps in PLAN.md move toward; it is not to be
built in one go (§17's condition).

**The problem.** Each frame answers: what colour is each pixel, given a world that keeps
changing? Four kinds of work, and techniques differ in how they divide them:
1. *Keeping data current:* every structure a technique relies on (occupancy trees,
   distance fields, meshes, BLAS, face sets) is rebuilt or patched as the world changes.
   The 2026-10 sweeps show this often decides the ranking: `distance + sphere` has the
   fastest walk but 47 fps on far (rebuilds); rasterisers lose large scenes to CPU
   meshing; gpu-rt to BLAS builds.
2. *Primary visibility:* image order (ray casting), object order (raster, splat), sparse
   plus reuse (discovery), special orders (voxelspace columns).
3. *Shading:* needs secondary queries (any hit, incoherent), which are visibility
   questions of another kind.
4. *Reuse over time:* face sets, depth, lighting from earlier frames (incremental
   discovery, event-driven skipping, beams; later TAA, radiance caches).

**The pieces.**
1. *World and change log:* the canonical chunks, materials (opaque, cut-out,
   translucent), meshes and lights; everything else is derived and learns of changes
   through a list of deltas.
2. *Representations:* derived data built and kept current from the change log; each
   declares its queries (occupancy at level k, distance bound, column height, face list,
   triangles), its device, its update cost and memory. Storages, meshes, BLAS,
   heightmaps, point sets, LOD pyramids; discovery's face set (view-dependent); baked or
   flood-fill light.
3. *Traversal kernels:* answer ray queries against any representation with the needed
   capabilities (refloor: occupancy levels; sphere: distance bounds; rayquery: BLAS).
   Always compiled fused with the representation on one device.
4. *Visibility strategies:* organise primary visibility (ray cast, raster, splat,
   discovery, columns, near/far hybrids) using kernels; output one common form, a hit
   record / **visibility buffer** (depth, block and face, position on the face).
   Discovery's pieces: sampler (quadtree, cluster, heads), hole detector, retention
   policy (`keep_unseen`, event-driven), surface drawer.
5. *Surface sources:* meshes (and later non-cubic blocks) write into the same visibility
   buffer with a depth test; meshes stop being a special mode.
6. *Shading:* reads hits and materials, issues secondary queries through a kernel (any hit,
   opaque-only mask for transparency), may read caches (representations). **Placement**
   is a choice: inline (fused into the kernel, cheapest for ray casting) or deferred (a
   pass over the visibility buffer, best for raster per the sweep).
7. *History:* earlier frames' products as explicit inputs any stage may read.
8. *Composition and post:* translucent layers, tone mapping / HDR, upscaling, debug views
   with colour scales, written once.
9. *Executor:* places pieces on devices, inserts transfers at data boundaries, overlaps
   frames (as cpu-discover works one frame ahead). Back ends become executors.

**Two kinds of composition.** *Fused (source level):* representation + kernel + inline
shading compiled together; same device; where per-step cost lives. *Graph level (data
products):* change log, surface sets, visibility buffer, history, images; only these
cross devices (§15's cut points).

**Exceptions.** Techniques that fill several roles at once (voxelspace columns, path
tracing's bounce loop, RT pipelines) declare it rather than being forced apart.

**Measurement built in.** Every piece reports its own GPU / CPU time, counters, and an
optional debug view with a colour scale. Per-stage costs let the planner (§14) predict a
pipeline's cost as a sum of stages instead of measuring every combination; update cost
is measured separately from per-frame cost.

**Tensions.** A deferred visibility buffer costs bandwidth against shading at the hit
(placement as a choice handles it); shader variants multiply (build only what a plan
uses); capability contracts must stay small or fusion gets harder (§17's capability
model with adapters is the answer the user chose).

---

## Suggested order

The live plan, with estimates and progress, is in `PLAN.md`.
