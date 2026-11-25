import io
import unittest
from contextlib import redirect_stdout

from buildguard import compare

BASE = {"commit": "a", "at": "2026-11-25T00:00:00", "text_bytes": 100, "binary_bytes": 1000, "functions": 50,
        "check": {"total_s": 100.0, "quiet": True}, "ci": {"run": 1, "check": 20.0}}


def run(new, **kw):
    with redirect_stdout(io.StringIO()):
        return compare(BASE, {**BASE, **new}, kw.get("pct", 5), kw.get("time_pct", 10))


class Compare(unittest.TestCase):
    def test_size_growth_past_pct_is_flagged(self):
        self.assertEqual(run({"text_bytes": 106}), ["text_bytes"])
        self.assertEqual(run({"text_bytes": 105}), [])

    def test_times_flag_only_when_both_quiet(self):
        self.assertEqual(run({"check": {"total_s": 120.0, "quiet": True}}), ["check.py s"])
        self.assertEqual(run({"check": {"total_s": 120.0, "quiet": False}}), [])
        self.assertEqual(run({"check": {"total_s": 120.0, "quiet": None}}), [])

    def test_ci_times_are_never_flagged(self):
        self.assertEqual(run({"ci": {"run": 2, "check": 60.0}}), [])


if __name__ == "__main__":
    unittest.main()
