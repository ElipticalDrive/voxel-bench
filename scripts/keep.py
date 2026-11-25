#!/usr/bin/env python3
"""keep.py [--keep N] [--apply] [--root REPO] [PLACE...]: the keep-rule for old sweeps and baselines.

Each sweep directory (a directory of runs: target/sweep/<name>, target/turn/<name>,
baselines/sweep-<date>) is kept when any of these holds, else it goes:

- it is among the newest N of its place (default 3; `--keep N`), by name then time;
- a finding cites it: its name appears in docs/*.md, notes/*.md or ROADMAP.md (the findings,
  the log of A/B results and losses, the plan);
- a sweep is under way or died in it: a run in it has a journal (`passes.jsonl`) and no report,
  so `output.resume` can still continue it;
- it holds a file named KEEP (kept by hand).

Prints each directory with its size and why it stays or goes, then the total freed. Removes
nothing unless `--apply` (baselines/ is in git: what goes there is removed from the working
tree, to commit). PLACE names other places to apply the rule to instead of the three above.
"""
import argparse, os, pathlib, re, shutil, sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PLACES = ["target/sweep", "target/turn", "baselines"]


def size(p):
    return sum(f.stat().st_size for f in p.rglob("*") if f.is_file() and not f.is_symlink())


def citations(root):
    """The text the findings live in."""
    text = []
    for pat in ("docs/**/*.md", "notes/*.md", "ROADMAP.md"):
        for f in root.glob(pat):
            try:
                text.append(f.read_text(errors="replace"))
            except OSError:
                pass
    return "\n".join(text)


def sweeps(place):
    """The sweep directories of a place: those holding runs (a run: observations, a report or a journal)."""
    if not place.is_dir():
        return []
    out = []
    for d in place.iterdir():
        if d.is_dir() and not d.is_symlink() and any((r / f).exists() for r in d.iterdir() if r.is_dir() for f in ("observations.jsonl", "observations.jsonl.gz", "report.json", "passes.jsonl")):
            out.append(d)
    return sorted(out, key=lambda d: (d.name, d.stat().st_mtime))


def verdicts(place, keep, cited):
    """[(dir, reason it stays or None)] for one place, newest last."""
    ds = sweeps(place)
    newest = set(ds[-keep:]) if keep > 0 else set()
    out = []
    for d in ds:
        why = []
        if d in newest:
            why.append(f"among the newest {keep}")
        if re.search(r"(?<![\w-])" + re.escape(d.name) + r"(?![\w-])", cited):
            why.append("cited")
        if any((r / "passes.jsonl").exists() and not (r / "report.json").exists() for r in d.iterdir() if r.is_dir()):
            why.append("a run unfinished (resumable)")
        if (d / "KEEP").exists():
            why.append("KEEP")
        out.append((d, ", ".join(why) or None))
    return out


def main(argv=None, root=ROOT):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--keep", type=int, default=3, help="newest sweeps kept per place (default 3)")
    ap.add_argument("--apply", action="store_true", help="remove what goes (default: only print)")
    ap.add_argument("--root", type=pathlib.Path, default=root, help="the repository (default this one)")
    ap.add_argument("places", nargs="*", help="places instead of target/sweep, target/turn, baselines")
    a = ap.parse_args(argv)
    root = a.root
    cited = citations(root)
    freed = 0
    for place in [pathlib.Path(p) for p in a.places] or [root / p for p in PLACES]:
        vs = verdicts(place, a.keep, cited)
        if not vs:
            continue
        print(f"{place}:")
        for d, why in vs:
            n = size(d)
            print(f"  {'keep' if why else 'GO  '} {n / 1e6:9.1f} MB  {d.name}{'  (' + why + ')' if why else ''}")
            if not why:
                freed += n
                if a.apply:
                    shutil.rmtree(d)
    print(f"{'freed' if a.apply else 'would free (--apply removes)'}: {freed / 1e6:.1f} MB")
    return 0


if __name__ == "__main__":
    sys.exit(main())
