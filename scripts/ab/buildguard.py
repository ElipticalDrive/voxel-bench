#!/usr/bin/env python3
"""buildguard.py: the build's own figures, tracked as the catalogue grows.

  scripts/ab/buildguard.py [--pct P] [--time-pct P]   compare the tree's figures with the last record
  scripts/ab/buildguard.py record                     append them to baselines/build.jsonl
  scripts/ab/buildguard.py show                       print the records, one line each

The figures, each from where it is measured:
- sizes (deterministic, any machine): binary and .text bytes and functions of the release
  binary, from the newest `scripts/buildcost.py figures` JSON of this commit (run when missing);
- the release rebuild after a component edit (`rebuild_s`), when that JSON was made with --time;
- `check.py`'s wall time (total and per step) right after that rebuild, from the same JSON: a
  check.py run's own time depends on what was built before it, so only this defined start counts;
- CI's job times (`check`, `lavapipe`, minutes) of the newest successful run of this commit (gh).

A time is compared only when both records were taken on a quiet machine (`.agent/bin/quiet`,
as check.py and `buildcost.py --time` record it); CI's runners are compared as they come, which
varies far more than a change does (check 15-24 min, lavapipe 8-16 min on one tree), so CI
times are recorded and shown, never flagged. A quiet time is flagged past --time-pct % (default
10, as guard.py's --speed). Sizes grow with every component,
so a size is flagged when it grew by more than --pct % (default 5) of the last record: the
threshold is a setting, not a judgement; the deltas are always printed. Exit 1 when something was
flagged, 0 otherwise, 2 when there is no record to compare with.
"""
import argparse
import glob
import json
import os
import subprocess
import sys
from datetime import datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
RECORDS = ROOT / "baselines" / "build.jsonl"


def sh(*cmd):
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)


def commit():
    return sh("git", "rev-parse", "--short", "HEAD").stdout.strip()


def sizes(c):
    """The newest buildcost figures JSON made on commit c (made now when there is none)."""
    def newest():
        for f in sorted(glob.glob(str(ROOT / "target/buildcost/figures-*.json")), key=os.path.getmtime, reverse=True):
            d = json.loads(Path(f).read_text())
            if d.get("commit") == c:
                return d
    d = newest()
    if d is None:
        if sh("git", "status", "--porcelain", "--", "crates", "src").stdout:
            sys.exit("buildguard: crates/ or src/ has changes; sizes are recorded for a commit")
        r = subprocess.run([sys.executable, str(ROOT / "scripts/buildcost.py"), "figures"], cwd=ROOT, capture_output=True, text=True)
        if r.returncode:
            sys.exit("buildguard: buildcost.py figures failed:\n" + r.stderr[-2000:])
        d = newest()
    f = d["before"]
    out = {k: f[k] for k in ("binary_bytes", "text_bytes", "functions")}
    if d.get("timed") and f.get("rebuild_s") is not None:
        out["rebuild_s"] = f["rebuild_s"]
        out["rebuild_quiet"] = d.get("quiet")
    if d.get("timed") and f.get("check_s") is not None:
        out["check"] = {"total_s": f["check_s"], "steps": f.get("check_steps"), "quiet": d.get("quiet")}
    return out


def ci_times(full):
    r = sh("gh", "run", "list", "--commit", full, "--status", "success", "-L", "1", "--json", "databaseId")
    if r.returncode or not json.loads(r.stdout or "[]"):
        return None
    run = json.loads(r.stdout)[0]["databaseId"]
    jobs = json.loads(sh("gh", "run", "view", str(run), "--json", "jobs").stdout)["jobs"]
    ts = lambda s: datetime.fromisoformat(s.replace("Z", "+00:00"))
    return {"run": run, **{j["name"]: round((ts(j["completedAt"]) - ts(j["startedAt"])).total_seconds() / 60, 1) for j in jobs}}


def current():
    c = commit()
    rec = {"commit": c, "at": datetime.now().isoformat(timespec="seconds"), **sizes(c)}
    ci = ci_times(sh("git", "rev-parse", "HEAD").stdout.strip())
    if ci:
        rec["ci"] = ci
    return rec


def records():
    return [json.loads(l) for l in RECORDS.read_text().splitlines() if l.strip()] if RECORDS.exists() else []


def line(r):
    s = f"{r['commit']} {r['at'][:16]}  .text {r['text_bytes'] / 1e6:.1f} MB  binary {r['binary_bytes'] / 1e6:.1f} MB  fns {r['functions']:,}"
    if "rebuild_s" in r:
        s += f"  rebuild {r['rebuild_s']} s{'' if r.get('rebuild_quiet') else ' (not quiet)'}"
    if "check" in r:
        s += f"  check.py {r['check']['total_s']} s{'' if r['check'].get('quiet') else ' (not quiet)'}"
    if "ci" in r:
        s += "  CI " + " ".join(f"{k} {v} min" for k, v in r["ci"].items() if k != "run")
    return s


def compare(old, new, pct, time_pct):
    flagged = []
    print(f"base {line(old)}\nnow  {line(new)}")
    def cmp(label, a, b, limit, why=""):
        if a is None or b is None:
            return
        d = (b - a) / a * 100 if a else 0
        flag = limit is not None and d > limit
        print(f"  {label:22}{a:>14,}{b:>14,}{d:>+8.1f} %{'  FLAGGED' if flag else ''}{why}")
        if flag:
            flagged.append(label)
    for k in ("text_bytes", "binary_bytes", "functions"):
        cmp(k, old[k], new[k], pct)
    if "rebuild_s" in old and "rebuild_s" in new:
        both = old.get("rebuild_quiet") and new.get("rebuild_quiet")
        cmp("rebuild_s", old["rebuild_s"], new["rebuild_s"], time_pct if both else None, "" if both else "  (not both quiet: shown only)")
    if "check" in old and "check" in new:
        both = old["check"].get("quiet") is True and new["check"].get("quiet") is True
        cmp("check.py s", old["check"]["total_s"], new["check"]["total_s"], time_pct if both else None, "" if both else "  (not both quiet: shown only)")
    for job in sorted(set(old.get("ci", {})) & set(new.get("ci", {})) - {"run"}):
        cmp(f"CI {job} min", old["ci"][job], new["ci"][job], None, "  (CI runners vary: shown only)")
    return flagged


def main():
    ap = argparse.ArgumentParser(usage=__doc__)
    ap.add_argument("what", nargs="?", default="compare", choices=["compare", "record", "show"])
    ap.add_argument("--pct", type=float, default=5.0)
    ap.add_argument("--time-pct", type=float, default=10.0)
    a = ap.parse_args()
    rs = records()
    if a.what == "show":
        for r in rs:
            print(line(r))
        return
    new = current()
    if a.what == "record":
        RECORDS.parent.mkdir(exist_ok=True)
        with open(RECORDS, "a") as f:
            f.write(json.dumps(new) + "\n")
        print(line(new))
        print(f"appended to {RECORDS.relative_to(ROOT)}")
        return
    if not rs:
        print(line(new))
        sys.exit("buildguard: no record in baselines/build.jsonl yet (`buildguard.py record`)")
    flagged = compare(rs[-1], new, a.pct, a.time_pct)
    print(f"flagged: {', '.join(flagged)}" if flagged else "nothing flagged")
    sys.exit(1 if flagged else 0)


if __name__ == "__main__":
    main()
