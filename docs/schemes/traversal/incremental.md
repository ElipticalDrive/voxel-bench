+++
title = "Incremental hierarchical DDA"
category = "traversal"
status = "measured"
components = ["traversal/incremental"]
related = ["refloor", "prefix", "hier"]
origin = "user"
sources = []
+++

# Incremental hierarchical DDA

## What it is

A DDA that walks a hierarchy of levels, keeping per-axis exit times at the current level and
moving up and down incrementally, so that large steps cost as little as unit steps. It is the
traversal the project owner intended for the `prefix` storage from the start.

## How it works

The ray keeps a cell `c` at the current level (absolute coordinates shifted by the level),
per-axis exit times `tmax`, and cell sizes in t, `tdelta`. Nothing is re-floored:
- **Step:** a classic DDA step at the current level.
- **Ascend:** on axes where the cell is the first child in the ray's direction, `tmax` gains
  `tdelta`; then `tdelta` doubles. An ascent is tried only after a step crosses the next
  level's boundary, because otherwise the parent is the one already known to be occupied.
- **Descend:** `tdelta` halves, the midplane exit is `tmax − tdelta`, and the child is the
  second one along an axis exactly when the entry time has passed the midplane.

It is written against one storage query, `s_occupied(level, cell)`, and the storage's
`S_LEVELS` (the levels it answers directly). Knobs: `exact` recomputes the exit times from the
integer cell after each move (one multiply, no float drift; on by default), and
`skip2` … `skip16` choose the levels used.

## In this project

`components/src/traversal/incremental/` (`gpu.wgsl`, `cpu.rs`). On every back end with
`refloor` except `cpu-discover`. Tests compare it with `refloor` on 12,000 random rays per
storage over a multi-chunk world (negative coordinates, a missing chunk, a uniform chunk,
axis-aligned and diagonal rays): zero rays differ.

## Measured

- **CPU** (unlit, CCD-pinned, medians of 3): each move is much cheaper (170–210 instructions
  per occupancy test against 240–370 per refloor step), but there are more tests: every
  boundary crossing tries an ascent, and near surfaces most fail. Net: terrain (long rays)
  wins, `prefix` +13–17% and `hier` +5–10%; edits about even for hier, −3–7% for prefix;
  menger (3–4 steps per ray) −3–12%, the per-ray set-up not amortised.
- **Float drift is real:** accumulating exit times costs 1.5–3 dB at grazing edges;
  `exact` removes it at 3–7% of the speed.
- **GPU: 3.5–8× slower than `refloor` on NVIDIA, cause unknown.** On an AMD iGPU (RADV) the
  same shader is only 1.27× slower, about the CPU's ratio. On NVIDIA, `view = "steps"` (which
  writes only the iteration count) runs it at 1.18 ms against refloor's 0.85 with nearly the
  same iterations; the extra ~3.3 ms appears only when the ray's result is consumed. Ruled out:
  dynamic vector indexing, private globals, struct writes or returns inside the loop,
  unbounded inner loops, `continue`, a flattened loop, runaway rays, extra live state.
- Tried and dropped: delaying ascents for N steps after a descent (cut ascent tests by up to
  30%, but the extra steps and bookkeeping ate the gain).

## Open questions

- The NVIDIA slowdown (register allocation or reconvergence are suspected; seeing it needs
  pipeline statistics, which wgpu does not expose).
- Testing the next level up only after the current level has been empty for a while, without
  per-step bookkeeping.
