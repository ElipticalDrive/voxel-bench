#!/bin/bash
# perf.sh PLAN RENDERERS ROUNDS [-s overrides...]: interleaves runs of the saved "before"
# build and the current build (no reference, no saved frames) and prints median GPU ms and
# fps per pass. Both builds must use the same code alignment (.cargo/config.toml).
#   scripts/ab/perf.sh terrain '[{compose={backend="gpu-raster"}}]' 3 -s run.frames=300
# AFTER_ARGS: extra arguments for the current build only (with BEFORE pointing at the same
# binary, an A/B of one setting, e.g. AFTER_ARGS='-s run.compile="native"'). AFTER_BIN: another
# binary as the "after" side (e.g. a layout probe: the old commit with a meaningless change).
cd "$(dirname "$0")/../.."
PLAN=$1; R=$2; N=$3; shift 3
BEFORE=${BEFORE:-target/ab-before/multirenderer}
T=target/ab/perf; rm -rf $T; mkdir -p $T
for i in $(seq $N); do for b in before after; do
  BIN=${AFTER_BIN:-./target/release/multirenderer}; [ $b = before ] && BIN=$BEFORE
  EXTRA=(); [ $b = after ] && [ -n "$AFTER_ARGS" ] && eval "EXTRA=($AFTER_ARGS)"
  $BIN run plans/$PLAN.toml -s "renderer=$R" -u output.compare -u output.save -s "output.dir=\"$T/$b/$i\"" "$@" "${EXTRA[@]}" > $T/$b-$i.log 2>&1 || echo "$b FAILED (see $T/$b-$i.log)"
done; done
python3 -I scripts/ab/perfsum.py $T
