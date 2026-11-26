#!/usr/bin/env python3
"""Sweeps every valid composition over the scenario plans (an overnight job), then writes the
catalogue of what each renderer is good at, its first section the graphs below the floor
or failed (the bugs and missing `accept` rules a sweep finds).

  scripts/sweep.py                     # every scenario plan, at each plan's own length
  FRAMES=120 scripts/sweep.py terrain  # shorter camera paths (faster flight), one plan
  RES=480x270 EVERY=20 FRAMES=60 scripts/sweep.py terrain  # a quick correctness pass
  OUT=target/sweep/base scripts/sweep.py terrain           # into a given directory (adds to it)
  OUT=target/sweep/base scripts/sweep.py                   # again after it died: continues it
  GPU=radv scripts/sweep.py terrain    # on the GPU adapter whose name contains this (also llvmpipe)

Results: target/sweep/<time>/<run>/observations.jsonl (one record per renderer and pass,
tagged with this machine), and target/sweep/<time>/catalogue.md / .csv. Re-run the
catalogue over several sweeps with `multirenderer catalogue target/sweep`. Each run resumes
(`output.resume`): a plan whose run in OUT finished is skipped, one whose run died part-way
continues where it stopped (its log kept as <plan>.died-N.log), so the same command run again
after a sweep died continues it.
"""
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from mr import ROOT, SCENARIOS, build_release, finished, full_sweep, plan_log, release, run_logged
if {"-h", "--help"} & set(sys.argv[1:(sys.argv + ["--"]).index("--")]):  # help, never a run
    sys.exit(print(__doc__))

out = os.environ.get("OUT") or f"target/sweep/{time.strftime('%Y%m%d-%H%M')}"
plans = sys.argv[1:] or SCENARIOS + ["big"]
sweep = full_sweep()
args = ["-s", sweep, "-s", "run.repeats=1", "-s", "run.parallel=false", "-s", f"output.compare.every={os.environ.get('EVERY', 50)}", "-s", f'output.dir="{out}"', "-s", "output.resume=true", "-u", "output.save", "-u", "output.window"]
if os.environ.get("FRAMES"):
    args += ["-s", f"run.frames={os.environ['FRAMES']}", "-s", "run.warmup=10"]
if os.environ.get("COMPILE"):  # off (the default), native, break-in: every CPU pipeline compiled for this machine
    args += ["-s", f'run.compile="{os.environ["COMPILE"]}"']
if os.environ.get("GPU"):
    args += ["-s", f'run.devices={{gpus=["{os.environ["GPU"]}"]}}']
if os.environ.get("RES"):
    w, h = os.environ["RES"].split("x")
    args += ["-s", f"run.resolution=[{w},{h}]"]
if not build_release():
    sys.exit(1)
(ROOT / out).mkdir(parents=True, exist_ok=True)
for p in plans:
    if finished(out, p):
        print(f"=== {p}: finished in {out}, skipped", flush=True)
        continue
    print(f"=== {p} {time.strftime('%H:%M')}", flush=True)
    log = plan_log(out, p)
    run_logged([release(), "run", f"plans/{p}.toml", *args], log)
    lines = log.read_text(errors="replace").splitlines()
    print("\n".join(l for l in lines if "swept compositions run" in l or "observations)" in l))
    print(f"    failed passes: {len({l for l in lines if 'FAILED' in l})}")
subprocess.run([str(release()), "catalogue", out], cwd=ROOT)
