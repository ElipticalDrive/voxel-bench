#!/usr/bin/env python3
"""Every shipped GPU graph on a device clamped to WebGPU's baseline sizes (`MR_GPU_LIMITS=sizes`:
128 MiB a binding, 256 MiB a buffer, 8192 texels), so a big GPU shows what a small one refuses.

  scripts/limits.py [--no-build]

Small world (showcase): every graph renders, with the same pixels as on the unclamped device
(the voxel pool then spans several bindings). Big world (terrain at 4K, radius 40): every graph
renders or fails naming the limit it is over (a `max_...` limit); a generic wgpu error or a
silent failure is a problem. Prints only problems and a total; logs and runs in target/limits/.
Uses the release build; `check.py gpu` runs it."""
import json
import subprocess
import sys
import tomllib

from mr import ROOT, fresh, release, run_logged
if {"-h", "--help"} & set(sys.argv[1:(sys.argv + ["--"]).index("--")]):  # help, never a run
    sys.exit(print(__doc__))

OUT = ROOT / "target" / "limits"
GRAPHS = sorted(p.stem for p in (ROOT / "pipelines").glob("*.toml") if tomllib.loads(p.read_text()).get("executor") == "gpu")
SMALL = ["-s", "run.frames=6", "-s", "run.warmup=1"]
BIG = ["-s", "run.frames=2", "-s", "run.warmup=1", "-s", "run.resolution=[3840,2160]", "-s", "run.view_radius=40", "-s", 'run.modes=["unlit"]']


def run(name, plan, renderers, args, clamp):
    """The passes of one run (label, mode) -> pass record; None if the run itself failed."""
    out = fresh(OUT / name)
    spec = "[" + ",".join(renderers) + "]"
    log = OUT / f"{name}.log"
    ok = run_logged([release(), "run", f"plans/{plan}.toml", "-s", f"renderer={spec}", "-s", f'output.dir="{out.as_posix()}"', *args], log, env={"MR_GPU_LIMITS": clamp})
    if not ok:
        print(f"{name}: the run failed, see {log}")
        return None
    report = next(out.rglob("report.json"))
    return {(p["label"], p["mode"]): p for p in json.loads(report.read_text())}


def psnr(p):
    return (p.get("quality") or {}).get("psnr", {}).get("mean")


def main():
    if "--no-build" not in sys.argv:
        b = subprocess.run(["cargo", "build", "--release", "-q"], cwd=ROOT, capture_output=True, text=True)
        if b.returncode:
            sys.exit("build failed:\n" + b.stderr[-2000:])
    OUT.mkdir(parents=True, exist_ok=True)
    graphs = [f'{{graph="{g}"}}' for g in GRAPHS]
    problems = []
    small = ['{name="cpu-reference"}', *graphs]
    free = run("small-free", "showcase", small, SMALL, "")
    held = run("small-sizes", "showcase", small, SMALL, "sizes")
    big = run("big-sizes", "terrain", graphs, BIG, "sizes")
    if None in (free, held, big):
        sys.exit(1)
    for key, p in held.items():
        if p["renderer"] == "cpu-reference":
            continue
        if p["error"]:
            problems.append(f"small world {key[0]} {key[1]}: {p['error'][:300]}")
        elif key in free and psnr(p) is not None and abs(psnr(p) - psnr(free[key])) > 0.05:
            problems.append(f"small world {key[0]} {key[1]}: {psnr(p):.2f} dB clamped, {psnr(free[key]):.2f} dB not")
    failed = 0
    big = {k: p for k, p in big.items() if p["renderer"] != "cpu-reference"}
    for key, p in big.items():
        if p["error"]:
            failed += 1
            if "`max_" not in p["error"]:
                problems.append(f"big world {key[0]}: fails without naming a limit: {p['error'][:300]}")
    for line in problems:
        print(line)
    print(f"limits: {len(GRAPHS)} GPU graphs; small world {sum(p["renderer"] != "cpu-reference" for p in held.values())} passes, big world {len(big) - failed} rendered, {failed} over a named limit; {len(problems)} problems")
    sys.exit(1 if problems else 0)


if __name__ == "__main__":
    main()
