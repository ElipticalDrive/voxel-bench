#!/usr/bin/env python3
"""buildcost.py: what one more component costs the build, as the catalogue grows.

  scripts/buildcost.py figures [--time] [-o FILE]   the release binary's figures for the tree as it is
  scripts/buildcost.py storage [--time] [-o FILE]   the same with one more CPU storage (`new.py storage`,
                                                    a copy of `bricks`), then the tree restored; prints
                                                    both and the difference

Figures without --time are deterministic and need no idle machine: binary size, .text size,
functions in the binary (all, per workspace crate) and the functions whose name mentions the
added component's type (its instantiations). --time adds what needs one (`.agent/bin/quiet`
first, or queue it with `.agent/bin/idle`): the release rebuild after touching the components
crate (what a component edit costs) and `check.py` (build + clippy + test). Builds run at
nice 19. Writes JSON to FILE (default target/buildcost/<mode>-<date>.json). Nothing is committed."""
import json
import os
import re
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "target" / "release" / "multirenderer"
CRATES = ["mr_core", "mr_world", "mr_ipc", "mr_serve", "mr_worker", "mr_gpu", "mr_mesh", "mr_components",
          "mr_exec_cpu", "mr_exec_gpu", "mr_reference", "multirenderer"]
NAME = "costprobe"  # the scaffolded storage
TYPE = "Costprobe"


def sh(cmd, **kw):
    return subprocess.run(cmd, cwd=ROOT, check=True, **kw)


def build(timed):
    if timed:
        # what one component edit costs: everything from the components crate up is rebuilt
        (ROOT / "crates/mr-components/src/lib.rs").touch()
    t = time.monotonic()
    sh(["nice", "-n", "19", "cargo", "build", "--release", "-q", "--bin", "multirenderer"])
    return time.monotonic() - t


def figures(timed, probe=None):
    f = {"rebuild_s": None, "check_s": None}
    s = build(timed)
    if timed:
        f["rebuild_s"] = round(s, 1)
    f["binary_bytes"] = BIN.stat().st_size
    sizes = sh(["size", "-A", str(BIN)], capture_output=True, text=True).stdout
    f["text_bytes"] = next(int(l.split()[1]) for l in sizes.splitlines() if l.startswith(".text "))
    syms = sh(["nm", "-C", "--defined-only", str(BIN)], capture_output=True, text=True).stdout.splitlines()
    fns = [l.split(" ", 2)[2] for l in syms if len(l.split(" ", 2)) == 3 and l.split(" ", 2)[1] in "tTwW"]
    f["functions"] = len(fns)
    per = {c: 0 for c in CRATES}
    for n in fns:
        # the crate a function belongs to: the first workspace path in its name (impls name the type first)
        m = re.search(r"\b(" + "|".join(CRATES) + r")::", n)
        if m:
            per[m.group(1)] += 1
    f["functions_per_crate"] = per
    if probe:
        f[f"functions_naming_{probe}"] = sum(1 for n in fns if re.search(rf"\b{probe}\b", n))
    if timed:
        t = time.monotonic()
        subprocess.run([sys.executable, str(ROOT / "scripts/check.py")], cwd=ROOT)
        f["check_s"] = round(time.monotonic() - t, 1)
    return f


def table(a, b):
    rows = [("binary bytes", "binary_bytes"), (".text bytes", "text_bytes"), ("functions", "functions"),
            ("release rebuild s", "rebuild_s"), ("check.py s", "check_s")]
    rows += [(f"  fns in {c}", ("functions_per_crate", c)) for c in CRATES]
    out = [f"{'':24}{'before':>14}{'after':>14}{'delta':>12}"]
    for label, k in rows:
        get = (lambda f: f[k[0]][k[1]]) if isinstance(k, tuple) else (lambda f: f[k])
        x, y = get(a), get(b)
        if x is None or y is None:
            continue
        if not x and not y:
            continue
        out.append(f"{label:24}{x:>14,}{y:>14,}{y - x:>+12,}")
    n = f"functions_naming_{TYPE}"
    out.append(f"{'fns naming ' + TYPE:24}{'':>14}{b.get(n, 0):>14,}")
    return "\n".join(out)


def main():
    args = sys.argv[1:]
    if not args or args[0] not in ("figures", "storage"):
        sys.exit(__doc__)
    mode, timed = args[0], "--time" in args
    out = Path(args[args.index("-o") + 1]) if "-o" in args else \
        ROOT / "target/buildcost" / f"{mode}-{datetime.now():%Y%m%d-%H%M}.json"
    dirty = sh(["git", "status", "--porcelain", "--", "crates", "src"], capture_output=True, text=True).stdout
    if mode == "storage" and dirty:
        sys.exit("buildcost: crates/ or src/ has uncommitted changes; the scaffold must be undone cleanly")
    res = {"mode": mode, "timed": timed, "commit": sh(["git", "rev-parse", "--short", "HEAD"], capture_output=True,
                                                    text=True).stdout.strip(), "at": datetime.now().isoformat()}
    res["before"] = figures(timed)
    if mode == "storage":
        sh([sys.executable, "scripts/new.py", "storage", NAME], stdout=subprocess.DEVNULL)
        try:
            res["after"] = figures(timed, TYPE)
        finally:
            sh([sys.executable, "scripts/new.py", "remove", "storage", NAME], stdout=subprocess.DEVNULL)
            left = sh(["git", "status", "--porcelain", "--", "crates", "src"], capture_output=True, text=True).stdout
            if left:
                print("buildcost: the tree is NOT clean after removing the scaffold:\n" + left, file=sys.stderr)
        print(table(res["before"], res["after"]))
    else:
        print(json.dumps(res["before"], indent=1))
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(res, indent=1))
    print(f"\n{out.relative_to(ROOT) if out.is_relative_to(ROOT) else out}")


if __name__ == "__main__":
    main()
