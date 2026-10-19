#!/usr/bin/env python3
"""Sweeps every valid composition over the scenario plans (an overnight job), then writes the
catalogue of what each renderer is good at.

  scripts/sweep.py                     # every scenario plan, at each plan's own length
  FRAMES=120 scripts/sweep.py terrain  # shorter camera paths (faster flight), one plan

Results: target/sweep/<time>/<run>/observations.jsonl (one record per renderer and pass,
tagged with this machine), and target/sweep/<time>/catalogue.md / .csv. Re-run the
catalogue over several sweeps with `multirenderer catalogue target/sweep`.
"""
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from mr import ROOT, build_release, release, run_logged

out = f"target/sweep/{time.strftime('%Y%m%d-%H%M')}"
plans = sys.argv[1:] or "showcase terrain far edits menger random room many-lights streaming city islands lattice gyroid checker meshes".split()
sweep = 'renderer=[{compose={backend="*",storage="*",geometry="*",prepass="*",visibility="*",draw="*",traversal="*",shading="*",meshes="*"}}]'
args = ["-s", sweep, "-s", "run.repeats=1", "-s", "run.parallel=false", "-s", "output.compare.every=50", "-s", f'output.dir="{out}"', "-u", "output.save", "-u", "output.window"]
if os.environ.get("FRAMES"):
    args += ["-s", f"run.frames={os.environ['FRAMES']}", "-s", "run.warmup=10"]
if not build_release():
    sys.exit(1)
(ROOT / out).mkdir(parents=True, exist_ok=True)
for p in plans:
    print(f"=== {p} {time.strftime('%H:%M')}", flush=True)
    log = ROOT / out / f"{p}.log"
    run_logged([release(), "run", f"plans/{p}.toml", *args], log)
    lines = log.read_text(errors="replace").splitlines()
    print("\n".join(l for l in lines if "swept compositions run" in l or "observations)" in l))
    print(f"    failed passes: {len({l for l in lines if 'FAILED' in l})}")
subprocess.run([str(release()), "catalogue", out], cwd=ROOT)
