#!/bin/bash
# perf.sh PLAN RENDERERS ROUNDS [-s overrides...]: interleaves runs of the saved "before"
# build and the current build (no reference, no saved frames) and prints median GPU ms and
# fps per pass. Both builds must use the same code alignment (.cargo/config.toml).
#   scripts/ab/perf.sh terrain '[{compose={backend="gpu-raster"}}]' 3 -s run.frames=300
cd "$(dirname "$0")/../.."
PLAN=$1; R=$2; N=$3; shift 3
BEFORE=${BEFORE:-target/ab-before/multirenderer}
T=target/ab/perf; rm -rf $T; mkdir -p $T
for i in $(seq $N); do for b in before after; do
  BIN=./target/release/multirenderer; [ $b = before ] && BIN=$BEFORE
  $BIN run plans/$PLAN.toml -s "renderer=$R" -u output.compare -u output.save -s "output.dir=\"$T/$b/$i\"" "$@" > $T/$b-$i.log 2>&1 || echo "$b FAILED (see $T/$b-$i.log)"
done; done
python3 -I scripts/ab/perfsum.py $T
