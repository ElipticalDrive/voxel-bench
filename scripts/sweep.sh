#!/bin/bash
# Sweeps every valid composition over the scenario plans (an overnight job), then writes the
# catalogue of what each renderer is good at.
#
#   scripts/sweep.sh                     # every scenario plan, at each plan's own length
#   FRAMES=120 scripts/sweep.sh terrain  # shorter camera paths (faster flight), one plan
#
# Results: target/sweep/<time>/<run>/observations.jsonl (one record per renderer and pass,
# tagged with this machine), and target/sweep/<time>/catalogue.md / .csv. Re-run the
# catalogue over several sweeps with `multirenderer catalogue target/sweep`.
cd "$(dirname "$0")/.."
OUT=target/sweep/$(date +%Y%m%d-%H%M)
PLANS=${*:-showcase terrain far edits menger random room many-lights streaming meshes}
SWEEP='renderer=[{compose={backend="*",storage="*",geometry="*",prepass="*",visibility="*",draw="*",traversal="*",shading="*",meshes="*"}}]'
ARGS=(-s "$SWEEP" -s run.repeats=1 -s run.parallel=false -s output.compare.every=50 -s "output.dir=\"$OUT\"" -u output.save -u output.window)
[ -n "$FRAMES" ] && ARGS+=(-s run.frames="$FRAMES" -s run.warmup=10)
cargo build --release -q || exit 1
mkdir -p "$OUT"
for p in $PLANS; do
  echo "=== $p $(date +%H:%M)"
  ./target/release/multirenderer run "plans/$p.toml" "${ARGS[@]}" > "$OUT/$p.log" 2>&1
  grep -E "swept compositions run|observations\)" "$OUT/$p.log"
  echo "    failed passes: $(grep "FAILED" "$OUT/$p.log" | sort -u | wc -l)"
done
./target/release/multirenderer catalogue "$OUT"
