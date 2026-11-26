# Baseline: the sweep of 2026-11-24/25 (the graph architecture)

The first baseline of the graph architecture, on AMD Ryzen 9 9955HX + NVIDIA GeForce RTX 5060
Laptop GPU. The full sweep runs as background work (the idle queue, `.agent/bin/idle`): this
baseline holds the plans measured so far, and each plan is folded in when its run finishes.

| plan | run | passes | built from |
|---|---|---:|---|
| showcase | `20261124-074806-showcase` | 1423 | e0bc16f |
| terrain-flight | `20261124-081857-terrain-flight` | 1423 | e0bc16f |
| far | `20261126-030120-far` | 1423 | b642d4d |

Queued, not measured yet (the catalogue sweep): edits, menger, random, room, many-lights, streaming, city,
islands, lattice, gyroid, checker, meshes (the rest of `scripts/sweep.py`, from a frozen copy of
b642d4d), then the RADV sweep, the Windows check and the turn sweep.

- `*/observations.jsonl.gz`: one record per renderer and pass (observation version 6; read as
  version 7, its metrics moved into the map: `docs/plans.md`, "Observations and the catalogue").
  `scripts/ab/guard.py` and `sweepcmp.py` read them as they are and take this directory as the
  baseline when it is the newest `baselines/sweep-*`; for `multirenderer catalogue`, gunzip
  them first (`gunzip -k baselines/sweep-20261125/*/observations.jsonl.gz`).
- `*/plan.toml`: the plan each run ran.
- `catalogue.md`, `catalogue.csv.gz`: the catalogue of these runs.

`far` ran partly unsupervised (2026-11-26 01:47-11:16, the queue's service stopped): builds
outside it ran 01:52-02:11, so the 109 passes whose logs ended in that window are listed in
`20261126-030120-far/possibly-disturbed.txt` (label-mode, as the log names); read their timings
with care. Their pixels are unaffected.

Below the floor: on `far`, the naive ray-query geometry runs out of device memory (named in the
record: a defined failure, as in `docs/limits.md`); otherwise only the `columns` entries (a heightfield of the terrain's surface, which
cannot show overhangs or caves; accepted as such).
