#!/usr/bin/env python3
"""The scale rehearsal: where each pipeline stops as the world grows. Per scenario plan (its
generator), the view radius doubles step by step; every pipeline still standing renders a few
frames at each radius, and the first radius at which it fails (or is refused) is recorded with
the reason. Pass / fail only, no timing (it needs no idle machine).

  scripts/scale.py [--plans terrain,city] [--radii 8,16,32,64] [--mem 24G] [--only TEXT] [--no-build]
  scripts/scale.py --table          # the table so far (Markdown), from the journal

Pipelines: every shipped graph as it is, every storage on raycast-gpu and raycast-cpu, every
geometry on rayquery-gpu and raster-gpu (`--only TEXT` keeps those whose label contains TEXT).
The run goes inside a systemd scope capped at `--mem` (no swap), so a host that runs out of
memory is a recorded failure, not a frozen machine. Each step's result is journaled to
target/scale/results.jsonl as it ends; the same command again skips the steps journaled
(delete the file to start over). Runs and logs: target/scale/<plan>-r<radius>/."""
import json
import re
import subprocess
import sys

from mr import ROOT, build_release, fresh, release

OUT = ROOT / "target" / "scale"
JOURNAL = OUT / "results.jsonl"
GPU_STORAGES = "bricks hier prefix distance rows tree64 svo_dag clipmap".split()
CPU_STORAGES = "bricks hier prefix brickmajor morton u8 bits palette".split()
ARGS = ["-s", "run.frames=2", "-s", "run.warmup=1", "-s", "run.resolution=[640,360]", "-s", 'run.modes=["unlit"]',
        "-s", "run.parallel=false", "-s", "run.repeats=1", "-s", "run.max_lod=0",
        "-u", "output.compare", "-u", "output.save", "-u", "output.window"]


def pipelines():
    """name -> renderer spec (inline TOML)."""
    ps = {g: f'{{graph="{g}"}}' for g in sorted(p.stem for p in (ROOT / "pipelines").glob("*.toml"))}
    for g, ss in (("raycast-gpu", GPU_STORAGES), ("raycast-cpu", CPU_STORAGES)):
        for s in ss[1:]:
            ps[f"{g} storage={s}"] = f'{{graph="{g}",edits=[{{node="storage",component="{s}"}}]}}'
    for g, gs in (("rayquery-gpu", ["naive", "boxes"]), ("raster-gpu", ["naive", "cubes"])):
        for s in gs:
            ps[f"{g} geometry={s}"] = f'{{graph="{g}",edits=[{{node="geometry",component="{s}"}}]}}'
    return ps


def journal():
    if not JOURNAL.exists():
        return []
    return [json.loads(line) for line in JOURNAL.read_text().splitlines() if line.strip()]


def reason(err):
    """The first line of an error, shortened."""
    line = next((s for s in err.splitlines() if s.strip()), err).strip()
    line = re.sub(r"^\[[^]]*\][^:]*: worker error: ", "", line)  # the lane and the moment
    return line[:240]


def step(plan, radius, todo, mem):
    """Runs `todo` (name -> spec) on `plan` at `radius`; name -> error (None = rendered)."""
    out = fresh(OUT / f"{plan}-r{radius}")
    names = list(todo)
    spec = "[" + ",".join(todo[n].rstrip("}") + f',label="{n}"}}' for n in names) + "]"
    log = out / "run.log"
    cmd = ["systemd-run", "--user", "--scope", "--quiet", "-p", f"MemoryMax={mem}", "-p", "MemorySwapMax=0",
           str(release()), "run", f"plans/{plan}.toml", *ARGS, "-s", f"run.view_radius={radius}",
           "-s", f"renderer={spec}", "-s", f'output.dir="{out.as_posix()}"']
    with open(log, "w") as f:
        code = subprocess.run(cmd, cwd=ROOT, stdout=f, stderr=subprocess.STDOUT).returncode
    reports = list(out.rglob("report.json"))
    passes, culprit = {}, None
    if reports:
        for p in json.loads(reports[0].read_text()):
            passes[p["label"]] = p.get("error")
    else:
        # The run died: its journal holds the passes it finished, and the one it died in.
        for j in out.rglob("passes.jsonl"):
            for line in j.read_text(errors="replace").splitlines():
                try:
                    e = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if "report" in e:
                    passes[e["report"]["label"]] = e["report"].get("error")
                elif "started" in e:
                    culprit = e["started"]["label"]
                elif "done" in e:
                    culprit = None
    tail = re.sub(r"\x1b\[[0-9;]*m", "", log.read_text(errors="replace")).strip().splitlines()[-1:] or [""]
    killed = f"killed at the memory cap ({mem}, host and workers together)" if code in (-9, 137) else f"exit {code}"
    res = {n: reason(passes[n]) if passes[n] else None for n in names if n in passes}
    if not reports:
        if not res:
            # Dead in its first pass: the host's own world is as likely the cause as the
            # renderer, so every pipeline stops here.
            return {n: f"the run died in its first pass ({killed}): {tail[0][:160]}" for n in names}
        # Passes after the one the run died in did not run: left out, tried again next time.
        res[culprit or names[len(res)]] = f"took the run down ({killed}): {tail[0][:160]}"
    return res


def table(rows):
    """Markdown: pipeline x plan -> the largest radius rendered and where / why it stopped."""
    plans = sorted({r["plan"] for r in rows})
    names = list(pipelines())
    cell = {}
    for r in rows:
        k = (r["name"], r["plan"])
        c = cell.setdefault(k, {"ok": 0, "stop": None})
        if r["error"] is not None:
            r = {**r, "error": reason(r["error"])}
        if r["error"] is None:
            c["ok"] = max(c["ok"], r["radius"])
        elif c["stop"] is None or r["radius"] < c["stop"][0]:
            c["stop"] = (r["radius"], r["error"])
    lines = ["| pipeline | " + " | ".join(plans) + " |", "|---" * (len(plans) + 1) + "|"]
    reasons = {}
    for n in names:
        if not any((n, p) in cell for p in plans):
            continue
        cs = []
        for p in plans:
            c = cell.get((n, p))
            if c is None:
                cs.append("-")
            elif c["stop"] is None:
                cs.append(f"≥ {c['ok']}")
            else:
                i = reasons.setdefault(c["stop"][1], len(reasons) + 1)
                cs.append(f"{c['ok'] or '<' + str(c['stop'][0])} (r{c['stop'][0]}: [{i}])")
        lines.append(f"| {n} | " + " | ".join(cs) + " |")
    lines.append("")
    lines += [f"[{i}] {e}" for e, i in reasons.items()]
    return "\n".join(lines)


def main():
    argv = sys.argv[1:]
    opt = lambda k, d: argv[argv.index(k) + 1] if k in argv else d
    if "--table" in argv:
        print(table(journal()))
        return
    plans = opt("--plans", "terrain,city,lattice,islands").split(",")
    radii = [int(r) for r in opt("--radii", "8,16,32,64").split(",")]
    mem = opt("--mem", "24G")
    only = opt("--only", "")
    if "--no-build" not in argv and not build_release():
        sys.exit("build failed")
    OUT.mkdir(parents=True, exist_ok=True)
    done = journal()
    for plan in plans:
        dead = {r["name"] for r in done if r["plan"] == plan and r["error"] is not None}
        for radius in radii:
            while True:  # again while a run that died left passes untried
                todo = {n: s for n, s in pipelines().items() if only in n and n not in dead
                        and not any(r["plan"] == plan and r["radius"] == radius and r["name"] == n for r in done)}
                if not todo:
                    break
                res = step(plan, radius, todo, mem)
                with open(JOURNAL, "a") as f:
                    for n, e in res.items():
                        f.write(json.dumps({"plan": plan, "radius": radius, "name": n, "error": e}) + "\n")
                done = journal()
                bad = {n: e for n, e in res.items() if e}
                dead |= set(bad)
                print(f"{plan} r{radius}: {len(res) - len(bad)} rendered, {len(bad)} stopped", flush=True)
                for n, e in bad.items():
                    print(f"  {n}: {e}", flush=True)
    print(table(journal()))


if __name__ == "__main__":
    main()
