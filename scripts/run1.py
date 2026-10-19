#!/usr/bin/env python3
"""Renders one plan (default terrain) with the release build into target/run1, quietly, and
prints the summary of `report.py`; the log is target/logs/run1.log.

  scripts/run1.py RENDERER_TOML [PLAN] [-s overrides...]
  scripts/run1.py '[{compose={backend="gpu-compute",storage="prefix"}}]'
  scripts/run1.py '[{name="cpu-reference"}]' terrain -s run.frames=60"""
import os
import subprocess
import sys

here = os.path.dirname(os.path.abspath(__file__))
from mr import ROOT, fresh, release, run_logged

r, rest = sys.argv[1], sys.argv[2:]
plan = "terrain"
if rest and not rest[0].startswith("-"):
    plan, rest = rest[0], rest[1:]
out = fresh(ROOT / "target" / "run1")
(ROOT / "target" / "logs").mkdir(parents=True, exist_ok=True)
b = subprocess.run(["cargo", "build", "--release", "-q"], cwd=ROOT, capture_output=True, text=True)
print("\n".join([l for l in b.stderr.splitlines() if l.startswith(("warning", "error"))][:10]), end="")
log = ROOT / "target" / "logs" / "run1.log"
if not run_logged([release(), "run", f"plans/{plan}.toml", "-s", f"renderer={r}", "-s", f'output.dir="{out.as_posix()}"', *rest], log):
    print("run FAILED:")
    print("\n".join(log.read_text(errors="replace").splitlines()[-15:]))
    sys.exit(1)
sys.exit(subprocess.run([sys.executable, os.path.join(here, "report.py"), str(out)]).returncode)
