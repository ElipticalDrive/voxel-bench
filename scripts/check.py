#!/usr/bin/env python3
"""Quiet verification: prints only problems and one-line totals; full logs in target/logs/.

  scripts/check.py [only] [build|clippy|test|gpu|lavapipe|windows|wine|all]...

Default: build clippy test. `gpu`, `windows` and `wine` add to the default (`check.py gpu` runs
build clippy test gpu); `only` runs just what is named (`check.py only gpu`). `build`: dev and release builds, warnings count as failures.
`clippy`: clippy over the workspace, every warning a failure. `test`: the workspace's tests. `gpu`: the tests that need a GPU (the
host's end-to-end live sessions, the tuner and scale-curve demos run briefly, the device pool, and `limits.py`: every shipped
GPU graph on a device clamped to WebGPU's baseline sizes);
only `gui_screenshot` stays manual. `lavapipe`: the GPU tests that fit Mesa's software Vulkan
(the device pool, the live sessions of the GPU graphs, the split, meshes, HDR, the headless GUI;
not the compiled build, nor the tuner and scale-curve demos, which time things) on lavapipe,
whatever GPU the machine has: what CI runs, where no GPU is. `windows`: zero-warning
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
if not os.environ.get("MR_IDLE_JOB"):
    busy = subprocess.run([str(ROOT / ".agent" / "bin" / "idle"), "busy"], capture_output=True, text=True)
    if busy.returncode == 0:
        print(f"warning: {busy.stdout.strip()}; building now disturbs it", flush=True)


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
        os.environ.setdefault("SECONDS", "8")  # the tuner demo's run, long enough for trials
        for name, args in (("gpu e2e", ["--bin", "multirenderer", "--", "--ignored", "--skip", "gui_screenshot"]), ("gpu pool", ["-p", "mr-gpu", "--", "--ignored"])):
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
        subprocess.run(["cargo", "build", "--release", "-q"], cwd=ROOT, capture_output=True)  # the sessions' workers
        e2e = ["gui::live::tests::live_session_end_to_end", "gui::live::tests::live_hdr_window", "gui::live::tests::live_meshes_end_to_end", "gui::live::tests::live_split_end_to_end", "gui::tests::gui_headless_smoke"]
        for name, args in (("lavapipe e2e", ["--bin", "multirenderer", "--", "--ignored", "--exact", *e2e]), ("lavapipe pool", ["-p", "mr-gpu", "--", "--ignored"])):
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
sys.exit(rc)
