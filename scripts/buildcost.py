#!/usr/bin/env python3
"""buildcost.py: what one more component costs the build, as the catalogue grows.

  scripts/buildcost.py figures [--time] [-o FILE]   the release binary's figures for the tree as it is
  scripts/buildcost.py storage [--time] [-o FILE]   the same with one more CPU storage (`new.py storage`,
                                                    a copy of `bricks`), then the tree restored; prints
                                                    both and the difference
  scripts/buildcost.py walk [--time] [-o FILE]      the same with one more CPU raycast walk: a copy of
                                                    `incremental` matched at run time beside the others
  scripts/buildcost.py shading [--time] [-o FILE]   the same with one more CPU shading: a copy of `direct`
                                                    chosen at run time beside it
  scripts/buildcost.py coverage SETTING [--time] [-o FILE]
                                                    the same built with MR_CPU_COVERAGE=SETTING (e.g.
                                                    "packets=8 isa=avx2"), in target/coverage so the
                                                    usual build is not redone; the narrowed build's saving

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
C = ROOT / "crates/mr-components/src"
RAY = ROOT / "crates/mr-exec-cpu/src/raycast.rs"

# The walk and shading probes: not components (nothing registers them, no graph picks them),
# only the code a real one adds to the generic build, reachable at run time so it is kept:
# (copy from, copy to, renames in the copy), then (file, text, replacement) edits.
PROBES = {
    "walk": ((C / "traversal/incremental/cpu.rs", C / "traversal/costprobe_cpu.rs",
              [("trace_incremental", "trace_costprobe"), ("struct Incremental", "struct CostprobeWalk"),
               ("for Incremental", "for CostprobeWalk"), ('"incremental"', '"costprobe"')]), [
        (C / "traversal/mod.rs", "mod cpu_tests;\n", "mod cpu_tests;\npub mod costprobe_cpu;\n"),
        (RAY, "    IncrementalExact,\n}", "    IncrementalExact,\n    Costprobe,\n}"),
        (RAY, '        "refloor" => Traversal::Refloor,\n',
         '        "refloor" => Traversal::Refloor,\n        "costprobe" => Traversal::Costprobe,\n'),
        (RAY, "            Traversal::Refloor => {}\n",
         "            Traversal::Refloor => {}\n            Traversal::Costprobe => return mr_components::traversal::costprobe_cpu"
               "::trace_costprobe::<R, false>(&self.grid, levels, cam, p, d, dist, bmin, bmax).0,\n"),
        (RAY, "                            Traversal::IncrementalExact => trace_incremental",
         "                            Traversal::Costprobe => mr_components::traversal::costprobe_cpu::trace_costprobe"
               "::<R, false>(&scene.grid, levels, cam_block, o, dirs[l], 1e7, bmin, bmax),\n"
               "                            Traversal::IncrementalExact => trace_incremental"),
    ]),
    "shading": ((C / "shading/direct/cpu.rs", C / "shading/costprobe_cpu.rs",
                 # the copy takes the same context and surface as `direct` (what the caller builds)
                 [("pub struct Ctx<", "pub struct UnusedCtx<"), ("pub struct Surface", "pub struct UnusedSurface"),
                  ("impl Surface", "impl UnusedSurface"),
                  ("use glam::", "#![allow(dead_code)]\nuse crate::shading::direct::cpu::{Ctx, Surface};\nuse glam::")]), [
        (C / "shading/mod.rs", "pub mod deferred;\n", "pub mod deferred;\npub mod costprobe_cpu;\n"),
        (RAY, "        mr_components::shading::direct::cpu::shade(&shading_ctx!(self, S, world, cam, T), s, mode)",
         "        if COSTPROBE.load(std::sync::atomic::Ordering::Relaxed) {\n"
         "            return mr_components::shading::costprobe_cpu::shade(&shading_ctx!(self, S, world, cam, T), s, mode);\n"
         "        }\n"
         "        mr_components::shading::direct::cpu::shade(&shading_ctx!(self, S, world, cam, T), s, mode)"),
        (RAY, "        mr_components::shading::direct::cpu::radiance(&shading_ctx!(self, S, world, cam, true), s, mode)",
         "        if COSTPROBE.load(std::sync::atomic::Ordering::Relaxed) {\n"
         "            return mr_components::shading::costprobe_cpu::radiance(&shading_ctx!(self, S, world, cam, true), s, mode);\n"
         "        }\n"
         "        mr_components::shading::direct::cpu::radiance(&shading_ctx!(self, S, world, cam, true), s, mode)"),
        (RAY, "/// The traversal kernel, with the `exact` knob folded in.\n",
         "pub static COSTPROBE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);\n\n"
         "/// The traversal kernel, with the `exact` knob folded in.\n"),
    ]),
}


def probe_apply(kind):
    (src, dst, renames), edits = PROBES[kind]
    text = src.read_text()
    for a, b in renames:
        text = text.replace(a, b)
    dst.write_text(text)
    for path, old, new in edits:
        t = path.read_text()
        if t.count(old) != 1:
            raise SystemExit(f"buildcost: anchor not found once in {path.relative_to(ROOT)}: {old[:60]!r}")
        path.write_text(t.replace(old, new))


def probe_undo(kind):
    (_, dst, _), edits = PROBES[kind]
    dst.unlink(missing_ok=True)
    sh(["git", "checkout", "--", *sorted({str(p.relative_to(ROOT)) for p, _, _ in edits})])


def sh(cmd, **kw):
    return subprocess.run(cmd, cwd=ROOT, check=True, **kw)


def build(timed, coverage=None):
    if timed:
        # what one component edit costs: everything from the components crate up is rebuilt
        (ROOT / "crates/mr-components/src/lib.rs").touch()
    env = os.environ.copy()
    env.pop("MR_CPU_COVERAGE", None)
    if coverage is not None:
        env.update(MR_CPU_COVERAGE=coverage, CARGO_TARGET_DIR=str(ROOT / "target" / "coverage"))
    t = time.monotonic()
    sh(["nice", "-n", "19", "cargo", "build", "--release", "-q", "--bin", "multirenderer"], env=env)
    return time.monotonic() - t


def figures(timed, probe=None, coverage=None):
    f = {"rebuild_s": None, "check_s": None}
    s = build(timed, coverage)
    if timed:
        f["rebuild_s"] = round(s, 1)
    binary = BIN if coverage is None else ROOT / "target" / "coverage" / "release" / "multirenderer"
    f["binary_bytes"] = binary.stat().st_size
    sizes = sh(["size", "-A", str(binary)], capture_output=True, text=True).stdout
    f["text_bytes"] = next(int(l.split()[1]) for l in sizes.splitlines() if l.startswith(".text "))
    syms = sh(["nm", "-C", "--defined-only", str(binary)], capture_output=True, text=True).stdout.splitlines()
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
    if timed and coverage is None:
        t = time.monotonic()
        subprocess.run([sys.executable, str(ROOT / "scripts/check.py")], cwd=ROOT)
        f["check_s"] = round(time.monotonic() - t, 1)
        steps = ROOT / "target/logs/times.json"
        if steps.exists():
            f["check_steps"] = json.loads(steps.read_text())["steps"]
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
    for n in (k for k in b if k.startswith("functions_naming_")):
        out.append(f"{'fns naming ' + n[17:]:24}{'':>14}{b[n]:>14,}")
    return "\n".join(out)


def main():
    args = sys.argv[1:]
    if not args or args[0] not in ("figures", "storage", "walk", "shading", "coverage") or args[0] == "coverage" and len(args) < 2:
        sys.exit(__doc__)
    mode, timed = args[0], "--time" in args
    out = Path(args[args.index("-o") + 1]) if "-o" in args else \
        ROOT / "target/buildcost" / f"{mode}-{datetime.now():%Y%m%d-%H%M}.json"
    dirty = sh(["git", "status", "--porcelain", "--", "crates", "src"], capture_output=True, text=True).stdout
    if mode not in ("figures", "coverage") and dirty:
        sys.exit("buildcost: crates/ or src/ has uncommitted changes; the scaffold must be undone cleanly")
    res = {"mode": mode, "timed": timed, "commit": sh(["git", "rev-parse", "--short", "HEAD"], capture_output=True,
                                                    text=True).stdout.strip(), "at": datetime.now().isoformat()}
    if timed:
        # times compare only between quiet runs (scripts/ab/buildguard.py); absent tool: unknown
        q = ROOT / ".agent/bin/quiet"
        res["quiet"] = subprocess.run([str(q)], capture_output=True).returncode == 0 if q.exists() else None
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
    elif mode == "coverage":
        res["coverage"] = args[1]
        res["after"] = figures(timed, coverage=args[1])
        print(f"MR_CPU_COVERAGE={args[1]!r}")
        print(table(res["before"], res["after"]))
    elif mode in PROBES:
        try:
            probe_apply(mode)
            res["after"] = figures(timed, "costprobe_cpu")
        finally:
            probe_undo(mode)
            left = sh(["git", "status", "--porcelain", "--", "crates", "src"], capture_output=True, text=True).stdout
            if left:
                print("buildcost: the tree is NOT clean after removing the probe:\n" + left, file=sys.stderr)
        print(table(res["before"], res["after"]))
    else:
        print(json.dumps(res["before"], indent=1))
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(res, indent=1))
    print(f"\n{out.relative_to(ROOT) if out.is_relative_to(ROOT) else out}")


if __name__ == "__main__":
    main()
