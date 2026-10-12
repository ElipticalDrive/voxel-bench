#!/bin/bash
# wgsl.sh [PLANS...]: dumps every shader the saved "before" build
# (scripts/ab/save-before.sh) and the current build assemble for every valid composition
# (a short wildcard sweep, default plans terrain and meshes), then compares the two sets,
# ignoring comment lines. A refactor that must not change what runs leaves them
# identical; one that adds combinations keeps every "before" shader.
# SWEEP_BEFORE: the before build's sweep, when the compose syntax changed in between.
# WGSL_IGNORE: a regex of lines to drop, WGSL_MAP: "old=>new" replacements (both sides),
# for a change declared as harmless (e.g. a one-line wrapper the compiler inlines).
cd "$(dirname "$0")/../.."
PLANS=${*:-terrain meshes}
BEFORE=${BEFORE:-target/ab-before/multirenderer}
T=target/ab/wgsl; rm -rf $T; mkdir -p $T
# A copy, so rebuilding during the (long) run does not pull the binary from under it.
cp target/release/multirenderer $T/multirenderer-after || exit 1
SWEEP=${SWEEP:-'renderer=[{compose={backend="*",storage="*",geometry="*",prepass="*",visibility="*",draw="*",traversal="*",shading="*",meshes="*"}}]'}
for side in before after; do
  BIN=$BEFORE; S=${SWEEP_BEFORE:-$SWEEP}
  [ $side = after ] && BIN=$T/multirenderer-after && S=$SWEEP
  for p in $PLANS; do
    MR_DUMP_WGSL=$T/$side $BIN run plans/$p.toml -s "$S" -s run.frames=3 -s run.warmup=1 -s run.repeats=1 -s run.parallel=false \
      -u output.compare -u output.save -u output.window -s "output.dir=\"$T/run-$side\"" > $T/$side-$p.log 2>&1 || echo "$side $p FAILED (see $T/$side-$p.log)"
  done
done
python3 -I - "$T" <<'PY'
import hashlib, os, pathlib, re, sys
t = pathlib.Path(sys.argv[1])
ignore = re.compile(os.environ["WGSL_IGNORE"]) if os.environ.get("WGSL_IGNORE") else None
maps = [m.split("=>", 1) for m in os.environ.get("WGSL_MAP", "").split(";") if "=>" in m]
def norm(side):
    out = {}
    for f in (t / side).glob("*.wgsl"):
        lines = [l for l in f.read_text().split("\n") if not l.lstrip().startswith("//") and not (ignore and ignore.search(l))]
        body = "\n".join(lines)
        for a, b in maps:
            body = body.replace(a, b)
        out.setdefault(hashlib.sha1(body.encode()).hexdigest(), f.name)
    return out
b, a = norm("before"), norm("after")
missing = [b[h] for h in b if h not in a]
print(f"before: {len(b)} distinct shaders, after: {len(a)}; before-only: {len(missing)}, after-only: {len(set(a) - set(b))}")
for m in sorted(missing):
    print("  before-only:", m)
print("identical" if not missing and len(a) == len(b) else ("every before shader kept" if not missing else "DIFFERENT"))
PY
