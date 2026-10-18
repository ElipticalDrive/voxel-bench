#!/usr/bin/env python3
"""wgsl.py [PLANS...]: dumps every shader the saved "before" build
(scripts/ab/save-before.py) and the current build assemble for every valid composition
(a short wildcard sweep, default plans terrain and meshes), then compares the two sets,
ignoring comment lines. A refactor that must not change what runs leaves them
identical; one that adds combinations keeps every "before" shader.
SWEEP_BEFORE: the before build's sweep, when the compose syntax changed in between.
WGSL_IGNORE: a regex of lines to drop, WGSL_MAP: "old=>new" replacements (both sides),
for a change declared as harmless (e.g. a one-line wrapper the compiler inlines).
"""
import hashlib
import os
import re
import shutil
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from mr import ROOT, before, fresh, release, run_logged

plans = sys.argv[1:] or ["terrain", "meshes"]
t = fresh(ROOT / "target" / "ab" / "wgsl")
# A copy, so rebuilding during the (long) run does not pull the binary from under it.
after = t / ("multirenderer-after" + release().suffix)
shutil.copy2(release(), after)
sweep = os.environ.get("SWEEP") or 'renderer=[{compose={backend="*",storage="*",geometry="*",prepass="*",visibility="*",draw="*",traversal="*",shading="*",meshes="*"}}]'
for side, exe, s in (("before", before(), os.environ.get("SWEEP_BEFORE") or sweep), ("after", after, sweep)):
    for p in plans:
        args = [exe, "run", f"plans/{p}.toml", "-s", s, "-s", "run.frames=3", "-s", "run.warmup=1", "-s", "run.repeats=1", "-s", "run.parallel=false",
                "-u", "output.compare", "-u", "output.save", "-u", "output.window", "-s", f'output.dir="{(t / f"run-{side}").as_posix()}"']
        if not run_logged(args, t / f"{side}-{p}.log", env={"MR_DUMP_WGSL": str(t / side)}):
            print(f"{side} {p} FAILED (see {t / f'{side}-{p}.log'})")

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
