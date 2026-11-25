"""Shared bits of the tools (Windows and Linux): the repository, the binaries, quiet runs."""
import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
EXE = ".exe" if os.name == "nt" else ""


def release(name="multirenderer"):
    """The release build of `name`."""
    return ROOT / "target" / "release" / (name + EXE)


def before():
    """The saved "before" build of A/B checks (`$BEFORE` overrides it)."""
    return pathlib.Path(os.environ.get("BEFORE") or ROOT / "target" / "ab-before" / ("multirenderer" + EXE))


def build_release():
    """`cargo build --release -q`; True when it succeeded."""
    return subprocess.run(["cargo", "build", "--release", "-q"], cwd=ROOT).returncode == 0


def after_build():
    """The "after" side of an A/B: release rebuilt from the working tree (save-before.py leaves the
    old code's build there), unless AFTER_BIN names another binary; exits when the build fails or
    equals the saved "before" build (fine with AFTER_ARGS: an A/B of a setting)."""
    import filecmp
    if os.environ.get("AFTER_BIN"):
        return
    if not build_release():
        sys.exit("release build failed")
    if before().exists() and filecmp.cmp(before(), release(), shallow=False) and not os.environ.get("SAME_OK") and not os.environ.get("AFTER_ARGS"):
        sys.exit("the release build equals the saved \"before\" build: nothing to compare (SAME_OK=1 for a noise floor)")


def run_logged(args, log, env=None):
    """Runs `args` in the repository with stdout and stderr into `log`; True on success."""
    with open(log, "w") as f:
        return subprocess.run([str(a) for a in args], cwd=ROOT, stdout=f, stderr=subprocess.STDOUT, env=None if env is None else {**os.environ, **env}).returncode == 0


def finished(out, plan):
    """Whether the sweep directory `out` holds a finished run of `plans/<plan>.toml` (its
    observations written); sweeps skip it, and resume (`output.resume`) one that died."""
    import re
    import tomllib
    with open(ROOT / "plans" / f"{plan}.toml", "rb") as f:
        name = re.sub(r"[^\w-]", "_", tomllib.load(f).get("name", "plan"))
    return any((d / "observations.jsonl").exists() for d in (ROOT / out).glob(f"*-*-{name}") if d.name[16:] == name)


def plan_log(out, plan):
    """The log of a sweep's run of `plan` in `out`; a dead run's log is kept as `<plan>.died-N.log`."""
    log = ROOT / out / f"{plan}.log"
    if log.exists():
        n = 1
        while (ROOT / out / f"{plan}.died-{n}.log").exists():
            n += 1
        log.rename(ROOT / out / f"{plan}.died-{n}.log")
    return log


def fresh(d):
    """Empties directory `d` (created if missing) and returns it."""
    import shutil
    d = pathlib.Path(d)
    shutil.rmtree(d, ignore_errors=True)
    d.mkdir(parents=True)
    return d


def full_sweep():
    """The `-s` override that replaces a plan's renderers by the full sweep (`scripts/full-sweep.toml`)."""
    return "renderer=" + (ROOT / "scripts" / "full-sweep.toml").read_text().split("renderer = ", 1)[1].strip()
