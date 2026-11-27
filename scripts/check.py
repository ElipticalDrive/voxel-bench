#!/usr/bin/env python3
"""Quiet verification: prints only problems and one-line totals; full logs in target/logs/.

  scripts/check.py [only] [build|clippy|test|gpu|lavapipe|windows|wine|all]...

Default: build clippy test. `gpu`, `windows` and `wine` add to the default (`check.py gpu` runs
build clippy test gpu); `only` runs just what is named (`check.py only gpu`). `build`: dev and release builds, warnings count as failures.
`clippy`: clippy over the workspace, every warning a failure. `test`: the workspace's tests, then the scripts' (`scripts/ab/test_*.py`). `gpu`: the tests that need a GPU (the
host's end-to-end live sessions, the tuner and scale-curve demos run briefly, the device pool, and `limits.py`: every shipped
GPU graph on a device clamped to WebGPU's baseline sizes);
only `gui_screenshot` stays manual. `lavapipe`: the GPU tests that fit Mesa's software Vulkan
(the device pool, the live sessions of the GPU graphs, the split, meshes, the headless GUI;
not the HDR window, which needs a display, the compiled build, nor the tuner and scale-curve
demos, which time things) on lavapipe, their waits times `MR_TEST_SLOW` (default 3 here),
whatever GPU the machine has: what CI runs, where no GPU is. `windows`: zero-warning
cross-checks for both Windows targets (`rustup target add x86_64-pc-windows-gnu
x86_64-pc-windows-msvc`). `wine`: the unit and GPU end-to-end tests of the `-gnu` build under
Wine (mingw64-gcc, wine); not in `all`."""
import os
import json
import re
import subprocess
import sys
import time

from mr import ROOT
if {"-h", "--help"} & set(sys.argv[1:(sys.argv + ["--"]).index("--")]):  # help, never a run
    sys.exit(print(__doc__))

logs = ROOT / "target" / "logs"
logs.mkdir(parents=True, exist_ok=True)
DEFAULT = ["build", "clippy", "test"]
args = sys.argv[1:]
if args[:1] == ["only"]:
    what = args[1:]
else:
    # Extras add to the default: "add `gpu`" ran gpu alone twice, the unit tests unrun.
    what = [w for w in DEFAULT if w not in args] + args if not set(args) & set(DEFAULT) else args
if "all" in what:
    what = ["build", "clippy", "test", "gpu", "windows"]

# A timing job of `.agent/bin/idle` is running: this build is load from outside it, which gets
# the job stopped and redone. Said, not refused (the queue may be waiting, not timing).
# `.agent/` is local (git-ignored): absent on CI and in fresh checkouts.
idle = ROOT / ".agent" / "bin" / "idle"
if not os.environ.get("MR_IDLE_JOB") and idle.exists():
    busy = subprocess.run([str(idle), "busy"], capture_output=True, text=True)
    if busy.returncode == 0:
        print(f"warning: {busy.stdout.strip()}; it holds while this builds (background work yields)", flush=True)


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


# Each step's wall time, for the build guard (scripts/ab/buildguard.py): target/logs/times.json,
# with the commit, whether the tree had changes, and whether the machine was quiet
# (`.agent/bin/quiet`; None where it is absent, as on CI), since times only compare when quiet.
quiet = ROOT / ".agent" / "bin" / "quiet"
times = {"at": time.strftime("%Y-%m-%dT%H:%M:%S"), "argv": what, "steps": {},
         "commit": subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=ROOT, capture_output=True, text=True).stdout.strip(),
         "dirty": bool(subprocess.run(["git", "status", "--porcelain", "--", "crates", "src", "Cargo.toml", "Cargo.lock"], cwd=ROOT, capture_output=True, text=True).stdout),
         "quiet": subprocess.run([str(quiet)], capture_output=True).returncode == 0 if quiet.exists() else None}
rc = 0
for w in what:
    started = time.monotonic()
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
        # The CPU executor in a narrowed build (`MR_CPU_COVERAGE`, crates/mr-exec-cpu/build.rs):
        # its own target dir, so the usual build is not redone each time.
        env = {**os.environ, "MR_CPU_COVERAGE": "packets=8 isa=", "CARGO_TARGET_DIR": str(ROOT / "target" / "coverage")}
        with open(logs / "test-coverage.log", "w") as f:
            s = subprocess.run(["cargo", "test", "-q", "-p", "mr-exec-cpu"], cwd=ROOT, env=env, stdout=f, stderr=subprocess.STDOUT).returncode
        rc |= tests("tests narrowed (packets=8 isa=: 1, 8, scalar)", s, (logs / "test-coverage.log").read_text(errors="replace"))
        # The scripts' own tests (records.py), on the dev build's binary the cargo tests built.
        env = {**os.environ, "MR_BIN": os.environ.get("MR_BIN", str(ROOT / "target" / "debug" / "multirenderer"))}
        with open(logs / "test-scripts.log", "w") as f:
            s = subprocess.run([sys.executable, "-W", "error::ResourceWarning", "-m", "unittest", "discover", "-s", "scripts/ab", "-p", "test_*.py"], cwd=ROOT, env=env, stdout=f, stderr=subprocess.STDOUT).returncode
        t = (logs / "test-scripts.log").read_text(errors="replace")
        n = re.search(r"Ran (\d+) test", t)
        print(f"script tests: {'ok' if s == 0 else 'FAILED'} ran={n[1] if n else 0}")
        if s: print(t[-3000:])
        rc |= s != 0
    elif w == "gpu":
        subprocess.run(["cargo", "build", "--release", "-q"], cwd=ROOT, capture_output=True)
        os.environ.setdefault("SECONDS", "8")  # the tuner demo's run, long enough for trials
        # One end-to-end test at a time: each starts a GUI and worker processes that fill the
        # machine, so in parallel they starve each other's waits (11 at once: load 100, three
        # timed out in 120 s; one at a time: 47 s, all pass).
        for name, args in (("gpu e2e", ["--bin", "multirenderer", "--", "--ignored", "--skip", "gui_screenshot", "--test-threads=1"]), ("gpu pool", ["-p", "mr-gpu", "--", "--ignored"]), ("gpu scale", ["--test", "scale", "--", "--ignored"]), ("gpu gates", ["--test", "gates", "--", "--ignored"]), ("gpu late", ["--test", "late", "--", "--ignored"])):
            s, t = cargo(["test", "-q", "--release", *args], logs / f"{name.replace(' ', '-')}.log")
            rc |= tests(name, s, t)
        r = subprocess.run([sys.executable, str(ROOT / "scripts" / "limits.py"), "--no-build"], cwd=ROOT, capture_output=True, text=True)
        print(r.stdout.strip() or r.stderr.strip()[-1000:])
        rc |= r.returncode != 0
    elif w == "lavapipe":
        icd = next((p for p in ("/usr/share/vulkan/icd.d/lvp_icd.x86_64.json", "/usr/share/vulkan/icd.d/lvp_icd.json") if os.path.exists(p)), None)
        if icd is None:
            print("lavapipe: FAILED no lvp_icd json under /usr/share/vulkan/icd.d (package mesa-vulkan-drivers)")
            rc = True
            continue
        os.environ.update(VK_DRIVER_FILES=icd, VK_ICD_FILENAMES=icd)
        os.environ.setdefault("MR_TEST_SLOW", "3")  # the live tests' waits, scaled for software Vulkan
        subprocess.run(["cargo", "build", "--release", "-q"], cwd=ROOT, capture_output=True)  # the sessions' workers
        e2e = ["gui::live::tests::live_session_end_to_end", "gui::live::tests::live_meshes_end_to_end", "gui::live::tests::live_split_end_to_end", "gui::tests::gui_headless_smoke"]
        for name, args in (("lavapipe e2e", ["--bin", "multirenderer", "--", "--ignored", "--test-threads=1", "--exact", *e2e]), ("lavapipe pool", ["-p", "mr-gpu", "--", "--ignored"])):
            s, t = cargo(["test", "-q", "--release", *args], logs / f"{name.replace(' ', '-')}.log")
            rc |= tests(name, s, t)
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
    times["steps"][w] = round(time.monotonic() - started, 1)
times["ok"] = not rc
(logs / "times.json").write_text(json.dumps(times, indent=1))
sys.exit(rc)
