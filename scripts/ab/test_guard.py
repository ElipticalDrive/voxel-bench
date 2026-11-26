"""guard.py with passes that lack performance data (it is background work): pixels still compared.

Run by `scripts/check.py test` (MR_BIN, default the dev build's binary); alone:
`python3 -m unittest discover -s scripts/ab -p 'test_*.py'`.
"""
import os, subprocess, sys, tempfile, unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from test_records import record, sweep


def guard(base, new):
    with tempfile.TemporaryDirectory() as d:
        sweep(os.path.join(d, 'base'), base)
        sweep(os.path.join(d, 'new'), new)
        p = subprocess.run([sys.executable, os.path.join(HERE, 'guard.py'), os.path.join(d, 'new'),
                            '--base', os.path.join(d, 'base')], capture_output=True, text=True)
        return p.returncode, p.stdout + p.stderr


def unmeasured(**drop):
    o = record()
    o['gpu_ms'], o['fps'] = None, 0.0  # how a record without timing reads (fps is required)
    o.update(drop)
    return o


class Guard(unittest.TestCase):
    def test_pixels_compared_without_speed(self):
        code, out = guard([record()], [unmeasured(psnr=record()['psnr'] - 5)])
        self.assertEqual(code, 1, out)
        self.assertIn('pixels worse: 1', out)
        self.assertIn('not compared (measured on one side only or neither): speed 1 passes', out)

    def test_unmeasured_speed_is_no_regression(self):
        code, out = guard([record()], [unmeasured()])
        self.assertEqual(code, 0, out)
        self.assertIn('speed 1 passes', out)
        self.assertIn('regressions 0', out)


if __name__ == '__main__':
    unittest.main()
