#!/usr/bin/env python3
"""Quiet verification: prints only problems and one-line totals; full logs in target/logs/.

  scripts/check.py [build|clippy|test|gpu|windows|wine|all]...

Default: build clippy test. `build`: dev and release builds, warnings count as failures.
`clippy`: clippy over the workspace, every warning a failure. `test`: the workspace's tests. `gpu`: the end-to-end tests that need a GPU. `windows`: zero-warning
cross-checks for both Windows targets (`rustup target add x86_64-pc-windows-gnu
x86_64-pc-windows-msvc`). `wine`: the unit and GPU end-to-end tests of the `-gnu` build under
Wine (mingw64-gcc, wine); not in `all`."""
import os
import re
import subprocess
import sys

from mr import ROOT

logs = ROOT / "target" / "logs"
logs.mkdir(parents=True, exist_ok=True)
what = sys.argv[1:] or ["build", "clippy", "test"]
if "all" in what:
    what = ["build", "clippy", "test", "gpu", "windows"]


def cargo(args, log):
    with open(log, "w") as f:
        s = subprocess.run(["cargo", *args], cwd=ROOT, stdout=f, stderr=subprocess.STDOUT).returncode
    return s, log.read_text(errors="replace")


def tests(name, s, t):
    """Prints a test run's totals and failures; True if it failed."""
    p = f = i = 0
    for m in re.finditer(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored", t):
        p += int(m[1]); f += int(m[2]); i += int(m[3])
    print(f"{name}: {'ok' if s == 0 else 'FAILED'} passed={p} failed={f} ignored={i}")
    for m in re.finditer(r"^---- (\S+) .*?\n(.*?)(?=\n\n)", t, re.M | re.S):
        print(" FAIL", m[1]); print("  " + "\n  ".join(m[2].splitlines()[:8]))
    if s != 0 and p + f == 0:
        print(t[-3000:])
    return s != 0


def compiled(name, s, t):
    """Prints a build's totals and problems; True if it failed or warned."""
    warn = re.findall(r"^\S+:\d+:\d+: warning.*$", t, re.M)
    err = re.findall(r"^\S+:\d+:\d+: error.*$|^error.*$", t, re.M)
    print(f"{name}: {'ok' if s == 0 else 'FAILED'} warnings={len(warn)} errors={len(err)}")
    for l in sorted(set(warn + err))[:40]:
        print(l)
    return s != 0 or bool(warn)


rc = 0
for w in what:
    if w == "build":
        for prof, args in (("dev", ["build", "--workspace", "--all-targets", "--message-format=short"]), ("release", ["build", "--release", "--workspace", "--message-format=short"])):
            s, t = cargo(args, logs / f"build-{prof}.log")
            rc |= compiled(f"build {prof}", s, t)
    elif w == "clippy":
        s, t = cargo(["clippy", "--workspace", "--all-targets", "--message-format=short"], logs / "clippy.log")
        rc |= compiled("clippy", s, t)
    elif w == "test":
        s, t = cargo(["test", "-q", "--workspace"], logs / "test.log")
        rc |= tests("tests", s, t)
    elif w == "gpu":
        subprocess.run(["cargo", "build", "--release", "-q"], cwd=ROOT, capture_output=True)
        s, t = cargo(["test", "-q", "--release", "--bin", "multirenderer", "--", "--ignored", "--skip", "tuner_demo", "--skip", "scale_curve", "--skip", "gui_screenshot"], logs / "gpu.log")
        res = re.findall(r"test result: .*", t)
        print(f"gpu e2e: {'ok' if s == 0 else 'FAILED'} {res[-1][:60] if res else ''}")
        if s != 0:
            print("\n".join([l for l in t.splitlines() if l.startswith("---- ") or "panicked" in l][:20]))
        rc |= s != 0
    elif w == "windows":
        for target in ("x86_64-pc-windows-gnu", "x86_64-pc-windows-msvc"):
            s, t = cargo(["check", "--workspace", "--all-targets", "--message-format=short", "--target", target], logs / f"check-{target}.log")
            rc |= compiled(f"check {target}", s, t)
    elif w == "wine":
        os.environ.update(WINEDEBUG="-all", CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER="wine")
        gnu = ["--target", "x86_64-pc-windows-gnu"]
        s, t = cargo(["test", "-q", "--no-fail-fast", "--workspace", *gnu], logs / "wine-test.log")
        rc |= tests("wine tests", s, t)
        s, t = cargo(["test", "-q", "--release", *gnu, "--bin", "multirenderer", "--", "--ignored", "--skip", "tuner_demo", "--skip", "scale_curve", "--skip", "gui_screenshot"], logs / "wine-gpu.log")
        rc |= tests("wine gpu e2e", s, t)
    else:
        print(f"unknown: {w}")
        rc = 2
sys.exit(rc)
