"""Shared bits of the tools (Windows and Linux): the repository, the binaries, quiet runs."""
import os
import pathlib
import subprocess

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


def run_logged(args, log, env=None):
    """Runs `args` in the repository with stdout and stderr into `log`; True on success."""
    with open(log, "w") as f:
        return subprocess.run([str(a) for a in args], cwd=ROOT, stdout=f, stderr=subprocess.STDOUT, env=None if env is None else {**os.environ, **env}).returncode == 0


def fresh(d):
    """Empties directory `d` (created if missing) and returns it."""
    import shutil
    d = pathlib.Path(d)
    shutil.rmtree(d, ignore_errors=True)
    d.mkdir(parents=True)
    return d
