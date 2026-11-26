"""Every script answers `-h` / `--help` with its usage and runs nothing: a flag a script does not
know must never start the job it runs by default (scale.py -h once started a scale run).

Run by `scripts/check.py test`; alone: `python3 -m unittest discover -s scripts/ab -p 'test_*.py'`.
"""
import ast, pathlib, re, subprocess, sys, unittest

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent
GUARD = 'if {"-h", "--help"} & set(sys.argv[1:(sys.argv + ["--"]).index("--")]):'
# Scripts that refuse anything but their own subcommands, printing their usage.
REFUSE = {"buildcost.py", "frozen.py", "new.py"}


def scripts():
    """Every script run from the command line (it reads sys.argv); modules and tests left out."""
    for p in sorted([*SCRIPTS.glob("*.py"), *SCRIPTS.glob("ab/*.py")]):
        src = p.read_text()
        if p.name.startswith("test_") or "sys.argv" not in src or p.name == "mr.py":
            continue
        yield p, src


class Help(unittest.TestCase):
    def test_every_script_has_a_guard(self):
        for p, src in scripts():
            with self.subTest(script=str(p.relative_to(SCRIPTS))):
                self.assertTrue(ast.get_docstring(ast.parse(src)), "no docstring to print as its usage")
                self.assertTrue(GUARD in src or "argparse" in src or p.name in REFUSE,
                                f"no -h guard: add\n    {GUARD}\n        sys.exit(print(__doc__))\nafter its imports")

    def test_help_prints_usage_and_runs_nothing(self):
        for p, src in scripts():
            if not (GUARD in src or "argparse" in src or p.name in REFUSE):
                continue  # the static test names it; running it might start its job
            for flag in ("-h", "--help"):
                with self.subTest(script=str(p.relative_to(SCRIPTS)), flag=flag):
                    r = subprocess.run([sys.executable, str(p), flag], capture_output=True, text=True, timeout=30,
                                       cwd=SCRIPTS.parent)
                    missing = re.search(r"No module named '([^']+)'", r.stderr)
                    if missing and not (SCRIPTS / f"{missing[1]}.py").exists() and not (SCRIPTS / "ab" / f"{missing[1]}.py").exists():
                        self.skipTest(f"{missing[1]} not installed")
                    first = ast.get_docstring(ast.parse(src)).splitlines()[0].strip()
                    out = r.stdout + r.stderr
                    self.assertTrue(first[:40] in out or "usage" in out.lower(), f"exit {r.returncode}, printed: {out[:300]}")
                    self.assertIn(r.returncode, (0, 1, 2) if p.name in REFUSE else (0,), out[:300])


if __name__ == "__main__":
    unittest.main()
