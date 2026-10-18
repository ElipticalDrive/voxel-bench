#!/usr/bin/env python3
"""perf.py PLAN RENDERERS ROUNDS [-s overrides...]: interleaves runs of the saved "before"
build and the current build (no reference, no saved frames) and prints median GPU ms and
fps per pass. Both builds must use the same code alignment (.cargo/config.toml).
  scripts/ab/perf.py terrain '[{compose={backend="gpu-raster"}}]' 3 -s run.frames=300
AFTER_ARGS: extra arguments for the current build only (with BEFORE pointing at the same
binary, an A/B of one setting, e.g. AFTER_ARGS='-s run.compile="native"'). AFTER_BIN: another
binary as the "after" side (e.g. a layout probe: the old commit with a meaningless change).
"""
import os
import shlex
import subprocess
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from mr import ROOT, before, fresh, release, run_logged

plan, renderers, rounds, extra = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4:]
t = fresh(ROOT / "target" / "ab" / "perf")
for i in range(1, rounds + 1):
    for side in ("before", "after"):
        exe = before() if side == "before" else os.environ.get("AFTER_BIN") or release()
        more = shlex.split(os.environ.get("AFTER_ARGS", "")) if side == "after" else []
        args = [exe, "run", f"plans/{plan}.toml", "-s", f"renderer={renderers}", "-u", "output.compare", "-u", "output.save", "-s", f'output.dir="{(t / side / str(i)).as_posix()}"', *extra, *more]
        if not run_logged(args, t / f"{side}-{i}.log"):
            print(f"{side} FAILED (see {t / f'{side}-{i}.log'})")
subprocess.run([sys.executable, "-I", str(ROOT / "scripts" / "ab" / "perfsum.py"), str(t)])
