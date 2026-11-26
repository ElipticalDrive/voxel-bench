#!/usr/bin/env python3
"""The turn sweep: the full sweep's graphs (`scripts/full-sweep.toml`) covered pairwise
(`[sweep] cover = "pairs"`) instead of as their product: every component of every swept node
on every scenario plan, and of any two swept choices every pair of values some pipeline takes
together, the pairs spread over the plans (`sweep.part` of `sweep.parts`: scenario plan i of
the list below takes part i, the same part each time). A fraction of the full sweep (sweep.py,
the overnight run), for a baseline per change and the regression guard. The same records as
sweep.py's, so `multirenderer catalogue` and `ab/guard.py` read it; against a full baseline
the guard compares the passes both have.

  scripts/turn-sweep.py                     # every scenario plan, at each plan's own length
  scripts/turn-sweep.py --whole             # every pair on every plan (several times longer)
  scripts/turn-sweep.py --list [--whole]    # render nothing: per plan, renderers and passes, turn vs full
  FRAMES=120 scripts/turn-sweep.py terrain  # shorter camera paths, one plan (still its part)
  RES=480x270 EVERY=20 FRAMES=60 scripts/turn-sweep.py terrain  # a quick correctness pass
  OUT=target/turn/base scripts/turn-sweep.py               # into a given directory (adds to it)
  GPU=radv scripts/turn-sweep.py terrain    # on the GPU adapter whose name contains this
  COMPILE=native scripts/turn-sweep.py terrain  # the pipelines compiled for this machine (the native sample)

A plan not in the list below takes every pair. Results: target/turn/<time>/<run>/observations.jsonl,
and target/turn/<time>/catalogue.md / .csv. Runs resume as sweep.py's do: the same command
with the same OUT again continues a sweep that died.
"""
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from mr import ROOT, build_release, finished, full_sweep, plan_log, release, run_logged
if {"-h", "--help"} & set(sys.argv[1:(sys.argv + ["--"]).index("--")]):  # help, never a run
    sys.exit(print(__doc__))

# sweep.py's scenario plans, in its order; a plan's place here is its part of the pairs.
PLANS = "showcase terrain far edits menger random room many-lights streaming city islands lattice gyroid checker meshes".split()

argv = sys.argv[1:]
listing, whole = "--list" in argv, "--whole" in argv
plans = [a for a in argv if not a.startswith("--")] or PLANS


def cover(p):
    """The overrides choosing plan p's share of the sweep."""
    part = [] if whole or p not in PLANS else ["-s", f"sweep.part={PLANS.index(p)}", "-s", f"sweep.parts={len(PLANS)}"]
    return ["-s", 'sweep.cover="pairs"', *part]


args = ["-s", full_sweep(), "-s", "run.repeats=1", "-s", "run.parallel=false", "-s", f"output.compare.every={os.environ.get('EVERY', 50)}", "-u", "output.save", "-u", "output.window"]
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

if listing:
    def counts(p, extra):
        out = subprocess.run([str(release()), "run", f"plans/{p}.toml", *args, *extra, "--list"], cwd=ROOT, capture_output=True, text=True)
        if out.returncode:
            sys.exit(out.stderr)
        tail = [l for l in out.stdout.splitlines() if l.startswith("# ")]
        n = int(re.search(r": (\d+) renderers", tail[0]).group(1))
        return n, sum(int(re.search(r"mode: (\d+) passes", l).group(1)) for l in tail[1:])
    print(f"{'plan':12s} {'renderers':>16s} {'passes':>16s}   (turn / full)")
    tot = [0, 0, 0, 0]
    for p in plans:
        (rt, pt), (rf, pf) = counts(p, cover(p)), counts(p, [])
        tot = [a + b for a, b in zip(tot, (rt, rf, pt, pf))]
        print(f"{p:12s} {rt:7d} / {rf:6d} {pt:7d} / {pf:6d}")
    print(f"{'total':12s} {tot[0]:7d} / {tot[1]:6d} {tot[2]:7d} / {tot[3]:6d}   passes: {100 * tot[2] / max(tot[3], 1):.1f} % of the full sweep's")
    sys.exit(0)

out = os.environ.get("OUT") or f"target/turn/{time.strftime('%Y%m%d-%H%M')}"
(ROOT / out).mkdir(parents=True, exist_ok=True)
for p in plans:
    if finished(out, p):
        print(f"=== {p}: finished in {out}, skipped", flush=True)
        continue
    print(f"=== {p} {time.strftime('%H:%M')}", flush=True)
    log = plan_log(out, p)
    run_logged([release(), "run", f"plans/{p}.toml", *args, "-s", f'output.dir="{out}"', "-s", "output.resume=true", *cover(p)], log)
    lines = log.read_text(errors="replace").splitlines()
    print("\n".join(l for l in lines if "swept pipelines run" in l or "observations)" in l))
    print(f"    failed passes: {len({l for l in lines if 'FAILED' in l})}")
subprocess.run([str(release()), "catalogue", out], cwd=ROOT)
