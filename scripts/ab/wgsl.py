#!/usr/bin/env python3
"""wgsl.py [PLANS...]: dumps every shader the saved "before" build
(scripts/ab/save-before.py) and the current build assemble for every valid composition
(a short wildcard sweep, default plans terrain and meshes), then compares the two sets:
each program with its `#ifdef`s resolved by its own shader defs (the dump's header line),
comments, blank lines and `#import` lines dropped (imports follow from the code), imports
compared after that as a second line. A refactor that must not change what runs leaves them
identical; one that adds combinations keeps every "before" shader.
--compare: only compare the dumps of the last run.
WGSL_REUSE_BEFORE=1: keep the before side's dumps from the last run (only the after side runs).
SWEEP_BEFORE: the before build's sweep, when the compose syntax changed in between.
WGSL_IGNORE: a regex of lines to drop, WGSL_MAP: "old=>new" replacements (both sides),
for a change declared as harmless (e.g. a one-line wrapper the compiler inlines).
"""
import hashlib
import os
import re
import shutil
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from mr import ROOT, before, fresh, full_sweep, release, run_logged

compare_only = "--compare" in sys.argv
plans = [a for a in sys.argv[1:] if a != "--compare"] or ["terrain", "meshes"]
import filecmp
if not compare_only and filecmp.cmp(before(), release(), shallow=False):
    sys.exit(f"{before()} is the current build: save the before side again (scripts/ab/save-before.py at the old commit)")
reuse = os.environ.get("WGSL_REUSE_BEFORE") == "1" and (ROOT / "target" / "ab" / "wgsl" / "before").is_dir()
t = ROOT / "target" / "ab" / "wgsl"
if compare_only:
    pass
elif reuse:
    for d in ("after", "run-after"):
        shutil.rmtree(t / d, ignore_errors=True)
else:
    t = fresh(t)
# A copy, so rebuilding during the (long) run does not pull the binary from under it.
after = t / ("multirenderer-after" + release().suffix)
if not compare_only:
    shutil.copy2(release(), after)
sweep = os.environ.get("SWEEP") or full_sweep()
for side, exe, s in (("before", before(), os.environ.get("SWEEP_BEFORE") or sweep), ("after", after, sweep)):
    if compare_only or (side == "before" and reuse):
        continue
    for p in plans:
        args = [exe, "run", f"plans/{p}.toml", "-s", s, "-s", "run.frames=3", "-s", "run.warmup=1", "-s", "run.repeats=1", "-s", "run.parallel=false",
                "-u", "output.compare", "-u", "output.save", "-u", "output.window", "-s", f'output.dir="{(t / f"run-{side}").as_posix()}"']
        if not run_logged(args, t / f"{side}-{p}.log", env={"MR_DUMP_WGSL": str(t / side)}):
            print(f"{side} {p} FAILED (see {t / f'{side}-{p}.log'})")

ignore = re.compile(os.environ["WGSL_IGNORE"]) if os.environ.get("WGSL_IGNORE") else None
maps = [m.split("=>", 1) for m in os.environ.get("WGSL_MAP", "").split(";") if "=>" in m]


def active(text):
    """The lines the module system compiles: `#ifdef` / `#ifndef` / `#else` / `#endif` resolved
    by the program's shader defs (its `// ---- shader defs: A, B` header)."""
    m = re.search(r"^// ---- shader defs: (.*)$", text, re.M)
    defs = {d.strip() for d in m.group(1).split(",")} if m else set()
    out, open_ = [], []
    for l in text.split("\n"):
        s = l.strip()
        d = re.match(r"#(ifdef|ifndef)\s+(\w+)", s)
        if d:
            open_.append((d.group(2) in defs) == (d.group(1) == "ifdef"))
        elif s.startswith("#else"):
            open_[-1] = not open_[-1]
        elif s.startswith("#endif"):
            open_.pop()
        elif all(open_):
            out.append(l)
    return out


def norm(side, imports):
    out = {}
    for f in (t / side).glob("*.wgsl"):
        lines = [l.split("//")[0].rstrip() for l in active(f.read_text())]
        lines = [l for l in lines if l.strip() and (imports or not l.startswith("#import")) and not (ignore and ignore.search(l))]
        body = "\n".join(lines)
        for a, b in maps:
            body = body.replace(a, b)
        out.setdefault(hashlib.sha1(body.encode()).hexdigest(), f.name)
    return out


for imports in (False, True):
    b, a = norm("before", imports), norm("after", imports)
    missing = [b[h] for h in b if h not in a]
    what = "with imports" if imports else "code"
    print(f"{what}: before {len(b)} distinct shaders, after {len(a)}; before-only {len(missing)}, after-only {len(set(a) - set(b))}")
    for m in sorted(missing)[:40]:
        print("  before-only:", m)
    print("  identical" if not missing and len(a) == len(b) else ("  every before shader kept" if not missing else "  DIFFERENT"))
