# Baseline: the sweep of 2026-10-11

The re-run overnight sweep after comparisons moved off the frame loop (2026-10-11), on AMD Ryzen 9 9955HX + NVIDIA GeForce RTX 5060 Laptop GPU (driver 610.43.03).
The reference for the zero-cost check of the architecture refactor:
a refactored composition must match these GPU ms / fps (allowing run-to-run noise, ±1% GPU
ms and ±4% fps for 90% of passes).

- `*/observations.jsonl.gz`: one record per renderer and pass (gunzip before use).
- `catalogue.md`, `catalogue.csv.gz`: the catalogue of the full sweep.

To compare a new sweep: `gunzip -k baselines/sweep-20261011/*/observations.jsonl.gz`, then
`multirenderer catalogue baselines/sweep-20261011 target/sweep/<new>` or compare per pass.
