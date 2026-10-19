+++
title = "Prefix: Morton masks with a popcount prefix"
category = "storage"
status = "measured"
components = ["storage/prefix"]
related = ["hier", "bricks", "incremental", "refloor", "svo-dag", "tree64"]
origin = "user"
sources = ["G. M. Morton, A computer oriented geodetic data base and a new technique in file sequencing, IBM 1966", "G. Jacobson, Space-efficient static trees and graphs, FOCS 1989 (rank by popcount)"]
+++

# Prefix: Morton masks with a popcount prefix

## What it is

Occupancy as one 64-bit mask per 4³ brick, the bricks in Morton order, plus a running
popcount over them. Any aligned cube from 2³ to 16³ is a contiguous range of the Morton
order, so its emptiness is a single compare of two prefix values. It is the project owner's
design.

## How it works

- **Masks.** The 512 bricks of a chunk are stored in Morton (Z-) order, and the 64 bits
  inside each mask are Morton-ordered too, so each byte of a mask is a 2³ sub-cube.
- **Prefix.** `P[i]` is the number of solid voxels in all masks before mask `i` (513 u16
  values). An aligned cube of `n` bricks starting at Morton index `a` is empty exactly when
  `P[a] == P[a + n]`.
- **Levels.** Voxel (a bit), 2³ (a byte), 4³ (the mask is zero), 8³ and 16³ (a prefix
  compare); the whole chunk from the chunk table. The probe goes bottom-up and stops at the
  first occupied level.
- **Rank.** `P[i] + popcount(m_i & below(bit))` is the rank of a solid voxel, so block ids can
  be stored compressed, solid voxels only, in Morton order, with an O(1) lookup at a hit (done
  on the CPU; the GPU reads ids from the shared pool).
- **Size.** 4 KiB of masks and 1 KiB of prefixes per chunk. An edit rewrites the prefixes
  after it (at most 512) and shifts the compressed ids.
- **Building** in one Morton-order pass produces masks, prefixes and ids together (rank
  order is Morton order).

## In this project

`components/src/storage/prefix/` (`mod.rs` builds the words, `gpu.wgsl`, `cpu.rs`). On every
back end with `bricks`. An exhaustive test checks that every claimed empty cube is empty,
that the probe never under-skips, and that ids read by rank are right.

## Measured

- **GPU: the fastest ray walk on open scenes.** With `refloor`, terrain 0.75 ms unlit and
  1.07 ms lit (bricks 0.86 / 1.20, hier 0.76 / 1.10); edits 0.51 / 0.75 (bricks 0.65 / 0.93).
  On menger (short rays) it is even with bricks. With LOD it is 22% faster than bricks. The GPU
  pays memory latency per step, so fewer, larger steps pay off.
- **CPU: fewest steps, not fewest cycles.** 31–51% fewer steps than bricks and 15–27% fewer
  than hier, and the fewest instructions on terrain and edits, but 3–9% behind hier in cycles
  under `refloor`: a non-unit step goes through the walk's general path (a floor on every
  axis), so big steps cost more than unit steps. Under `incremental` it is within 2% of hier
  on terrain (21.0 against 21.5 fps). Every level helps: terrain 20.1 fps with all levels,
  19.5 without 2³, 19.1 with only 4³ and 8³, 16.6 with only 8³.
- World data is 20% smaller than bricks on the CPU (200.6 against 251.8 MB on terrain),
  because ids are stored for solid voxels only.
- The 2³ level rounds differently at grazing edges: ±0.1 dB, in both directions.
- Building in one pass cut world updates from 7.2 to 2.0 ms per frame on terrain; edited
  chunks still cost +0.7 ms per frame on the GPU's edits plan.
- Tried and dropped: a branch-free probe (always pays every load: slower).

## Open questions

- Keep the Morton index incrementally as the cell moves, instead of encoding it per test.
- Rank-compressed ids on the GPU.
