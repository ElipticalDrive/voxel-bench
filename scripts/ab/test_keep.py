"""keep.py's rule (newest N, cited, unfinished, KEEP) and the sweeps' resume helpers in mr.py.

Run by `scripts/check.py test`; alone: `python3 -m unittest discover -s scripts/ab -p 'test_*.py'`.
"""
import contextlib, io, os, pathlib, sys, tempfile, unittest

SCRIPTS = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, SCRIPTS)
import keep, mr


def run_dir(sweep, name, *files):
    d = sweep / name
    d.mkdir(parents=True)
    for f in files:
        (d / f).write_text("x")


class Keep(unittest.TestCase):
    def test_rule(self):
        with tempfile.TemporaryDirectory() as t:
            root = pathlib.Path(t)
            place = root / "target" / "sweep"
            for name in ["20261101-0000", "20261102-0000", "20261103-0000", "20261104-0000", "20261105-0000", "20261106-0000"]:
                run_dir(place / name, "20261101-000000-terrain", "observations.jsonl", "report.json")
            run_dir(place / "20261100-0000", "20261100-000000-far", "passes.jsonl")  # died: resumable
            (place / "20261102-0000" / "KEEP").write_text("")
            (root / "docs").mkdir()
            (root / "docs" / "findings.md").write_text("measured in target/sweep/20261101-0000.\n")
            (place / "not-a-sweep").mkdir()
            with contextlib.redirect_stdout(io.StringIO()) as out:
                keep.main(["--keep", "2", "--root", str(root)])
            self.assertIn("would free", out.getvalue())
            self.assertEqual(len(list(place.iterdir())), 8, "a dry run removes nothing")
            with contextlib.redirect_stdout(io.StringIO()) as out:
                keep.main(["--keep", "2", "--apply", "--root", str(root)])
            left = sorted(d.name for d in place.iterdir())
            self.assertEqual(left, ["20261100-0000", "20261101-0000", "20261102-0000", "20261105-0000", "20261106-0000", "not-a-sweep"], out.getvalue())


class Resume(unittest.TestCase):
    def test_finished_and_logs(self):
        with tempfile.TemporaryDirectory(dir=mr.ROOT / "target") as t:
            out = pathlib.Path(t).relative_to(mr.ROOT)
            self.assertFalse(mr.finished(out, "terrain"))
            run_dir(mr.ROOT / out, "20261101-000000-terrain-flight", "passes.jsonl")
            self.assertFalse(mr.finished(out, "terrain"), "died part-way: not finished")
            (mr.ROOT / out / "20261101-000000-terrain-flight" / "observations.jsonl").write_text("")
            self.assertTrue(mr.finished(out, "terrain"))
            self.assertFalse(mr.finished(out, "far"))
            mr.plan_log(out, "far").write_text("first")
            mr.plan_log(out, "far").write_text("second")
            self.assertEqual((mr.ROOT / out / "far.died-1.log").read_text(), "first")
            self.assertEqual((mr.ROOT / out / "far.log").read_text(), "second")


if __name__ == "__main__":
    unittest.main()
