#!/usr/bin/env python3
"""Runs a command in a frozen copy of one commit, so a long job (a baseline sweep that waits
hours for an idle machine) measures exactly that commit while work goes on in the tree.

  scripts/frozen.py -- scripts/sweep.py terrain                 # HEAD as it is now
  OUT=target/sweep/base scripts/frozen.py -r 083ed26 -- scripts/sweep.py terrain
  scripts/frozen.py -r 083ed26 --path                           # print the copy's directory

The copy is `git archive REV` unpacked into target/frozen/<commit>/ (made once, reused; only
committed files, so uncommitted edits never leak in), with its own cargo target directory
inside it. The command runs there, so scripts/ and plans/ are the commit's too. OUT (set
before frozen.py, not by an `env` inside the command), if relative, is resolved against the
directory frozen.py was started from, so results land in this tree's target/ as an unfrozen
run's would. The copy's commit is in its FROZEN_AT file
and in MR_FROZEN_AT for the command.
"""
import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent


def main(argv):
    rev, path_only = "HEAD", False
    while argv and argv[0] != "--":
        a = argv.pop(0)
        if a == "-r" and argv:
            rev = argv.pop(0)
        elif a == "--path":
            path_only = True
        else:
            print(__doc__)
            return 2
    cmd = argv[1:]
    if not cmd and not path_only:
        print(__doc__)
        return 2
    full = subprocess.run(["git", "rev-parse", "--verify", f"{rev}^{{commit}}"], cwd=ROOT, capture_output=True, text=True)
    if full.returncode != 0:
        print(f"frozen: no commit {rev!r}", file=sys.stderr)
        return 2
    commit = full.stdout.strip()
    copy = ROOT / "target" / "frozen" / commit[:12]
    if not (copy / "FROZEN_AT").exists():
        part = copy.with_name(copy.name + ".part")
        subprocess.run(["rm", "-rf", str(part)], check=True)
        part.mkdir(parents=True)
        archive = subprocess.Popen(["git", "archive", commit], cwd=ROOT, stdout=subprocess.PIPE)
        untar = subprocess.run(["tar", "-x", "-C", str(part)], stdin=archive.stdout)
        if archive.wait() != 0 or untar.returncode != 0:
            print(f"frozen: exporting {commit[:12]} failed", file=sys.stderr)
            return 1
        (part / "FROZEN_AT").write_text(commit + "\n")
        part.rename(copy)
    if path_only:
        print(copy)
        return 0
    env = {**os.environ, "MR_FROZEN_AT": commit}
    env.pop("CARGO_TARGET_DIR", None)
    if env.get("OUT") and not os.path.isabs(env["OUT"]):
        env["OUT"] = os.path.abspath(env["OUT"])
    print(f"frozen: {commit[:12]} in {copy.relative_to(ROOT)}", flush=True)
    return subprocess.run(cmd, cwd=copy, env=env).returncode


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
