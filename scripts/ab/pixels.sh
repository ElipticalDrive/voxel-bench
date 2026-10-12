#!/bin/bash
# pixels.sh PLAN RENDERERS [AFTER_RENDERERS] [-s overrides...]: renders 40 frames (saving
# frames 0, 20, 39 with reference comparisons) with the saved "before" build
# (scripts/ab/save-before.sh) and the current build, then compares the saved frames pixel
# by pixel. AFTER_RENDERERS (optional, a TOML list) gives the current build different
# renderers (e.g. a new knob set to reproduce the old behaviour).
#   scripts/ab/pixels.sh terrain '[{compose={backend="gpu-compute"}}]'
cd "$(dirname "$0")/../.."
PLAN=$1; R=$2; shift 2
RB=$R; if [[ $# -gt 0 && $1 != -* ]]; then RB=$R; R=$1; shift; fi
BEFORE=${BEFORE:-target/ab-before/multirenderer}
T=target/ab/pixels; rm -rf $T; mkdir -p $T
$BEFORE run plans/$PLAN.toml -s "renderer=$RB" -s run.frames=40 -s run.warmup=5 -s output.compare.every=20 -s output.save.every=20 -s "output.dir=\"$T/before\"" "$@" > $T/before.log 2>&1 || echo "before FAILED (see $T/before.log)"
./target/release/multirenderer run plans/$PLAN.toml -s "renderer=$R" -s run.frames=40 -s run.warmup=5 -s output.compare.every=20 -s output.save.every=20 -s "output.dir=\"$T/after\"" "$@" > $T/after.log 2>&1 || echo "after FAILED (see $T/after.log)"
python3 -I scripts/ab/cmp.py $T/before $T/after
