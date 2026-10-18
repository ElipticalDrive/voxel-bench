#!/usr/bin/env python3
"""pixels.py PLAN RENDERERS [AFTER_RENDERERS] [-s overrides...]: renders 40 frames (saving
frames 0, 20, 39 with reference comparisons) with the saved "before" build
(scripts/ab/save-before.py) and the current build, then compares the saved frames pixel
by pixel. AFTER_RENDERERS (optional, a TOML list) gives the current build different
renderers (e.g. a new knob set to reproduce the old behaviour).
  scripts/ab/pixels.py terrain '[{compose={backend="gpu-compute"}}]'
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from mr import ROOT, before, fresh, release, run_logged

plan, rb = sys.argv[1], sys.argv[2]
rest = sys.argv[3:]
ra = rb
if rest and not rest[0].startswith("-"):
    ra, rest = rest[0], rest[1:]
t = fresh(ROOT / "target" / "ab" / "pixels")
for side, exe, r in (("before", before(), rb), ("after", release(), ra)):
    args = [exe, "run", f"plans/{plan}.toml", "-s", f"renderer={r}", "-s", "run.frames=40", "-s", "run.warmup=5", "-s", "output.compare.every=20", "-s", "output.save.every=20", "-s", f'output.dir="{(t / side).as_posix()}"', *rest]
    if not run_logged(args, t / f"{side}.log"):
        print(f"{side} FAILED (see {t / f'{side}.log'})")
subprocess.run([sys.executable, "-I", str(ROOT / "scripts" / "ab" / "cmp.py"), str(t / "before"), str(t / "after")])
