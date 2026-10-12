#!/bin/bash
# wgsl.sh [PLANS...]: dumps every shader the saved "before" build
# (scripts/ab/save-before.sh) and the current build assemble for every valid composition
# (a short wildcard sweep, default plans terrain and meshes), then compares the two sets.
# A refactor that must not change what runs leaves them identical.
cd "$(dirname "$0")/../.."
PLANS=${*:-terrain meshes}
BEFORE=${BEFORE:-target/ab-before/multirenderer}
T=target/ab/wgsl; rm -rf $T; mkdir -p $T
# A copy, so rebuilding during the (long) run does not pull the binary from under it.
cp target/release/multirenderer $T/multirenderer-after || exit 1
SWEEP='renderer=[{compose={backend="*",storage="*",geometry="*",prepass="*",visibility="*",shading="*",meshes="*"}}]'
for side in before after; do
  BIN=$BEFORE; [ $side = after ] && BIN=$T/multirenderer-after
  for p in $PLANS; do
    MR_DUMP_WGSL=$T/$side $BIN run plans/$p.toml -s "$SWEEP" -s run.frames=3 -s run.warmup=1 -s run.repeats=1 -s run.parallel=false \
      -u output.compare -u output.save -u output.window -s "output.dir=\"$T/run-$side\"" > $T/$side-$p.log 2>&1 || echo "$side $p FAILED (see $T/$side-$p.log)"
  done
done
echo "before: $(ls $T/before | wc -l) shaders, after: $(ls $T/after | wc -l)"
if diff <(ls $T/before) <(ls $T/after) > $T/diff.txt; then echo "identical"; else echo "DIFFERENT:"; cat $T/diff.txt; fi
