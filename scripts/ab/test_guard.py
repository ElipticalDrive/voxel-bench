"""guard.py with passes that lack performance data (it is background work): pixels still compared.

Run by `scripts/check.py test` (MR_BIN, default the dev build's binary); alone:
`python3 -m unittest discover -s scripts/ab -p 'test_*.py'`.
"""
import json, os, subprocess, sys, tempfile, unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from test_records import record, sweep
import records


def guard(base, new):
    with tempfile.TemporaryDirectory() as d:
        sweep(os.path.join(d, 'base'), base)
        sweep(os.path.join(d, 'new'), new)
        p = subprocess.run([sys.executable, os.path.join(HERE, 'guard.py'), os.path.join(d, 'new'),
                            '--base', os.path.join(d, 'base')], capture_output=True, text=True)
        return p.returncode, p.stdout + p.stderr


def v7(o, **metrics):
    """A version-7 record: the v6 test record's metrics in the map, then `metrics` set (None removes)."""
    o = json.loads(json.dumps(o))
    records.from_v6(o)
    o['version'] = 7
    for k, v in metrics.items():
        if v is None: o['metrics'].pop(k, None); o['methods'].pop(k, None)
        else: o['metrics'][k], o['methods'][k] = (v, 1) if not isinstance(v, tuple) else v
    return o


def unmeasured(**metrics):
    """A record without timing (performance data is background work): no gpu_ms, no fps."""
    return v7(record(), gpu_ms=None, fps=None, **metrics)


class Guard(unittest.TestCase):
    def test_pixels_compared_without_speed(self):
        code, out = guard([record()], [unmeasured(psnr=record()['psnr'] - 5)])  # base: a version-6 record
        self.assertEqual(code, 1, out)
        self.assertIn('pixels worse: 1', out)
        self.assertIn('not compared (measured on one side only or neither): speed 1 passes', out)

    def test_unmeasured_speed_is_no_regression(self):
        code, out = guard([record()], [unmeasured()])
        self.assertEqual(code, 0, out)
        self.assertIn('speed 1 passes', out)
        self.assertIn('regressions 0', out)


    def test_method_change_is_not_a_speed_change(self):
        base = v7(record(), gpu_ms=2.0)
        code, out = guard([base], [v7(record(), gpu_ms=(4.0, 2))])
        self.assertEqual(code, 0, out)
        self.assertIn('measured another way (method version changed; not compared as speed): gpu_ms 1 passes', out)
        code, out = guard([base], [v7(record(), gpu_ms=4.0)])
        self.assertEqual(code, 1, out)
        self.assertIn('slower beyond noise: 1', out)

    def test_slower_pass_names_changed_conditions(self):
        code, out = guard([v7(record(), gpu_ms=2.0, gpu_sm_mhz=2100.0, on_ac=1.0)], [v7(record(), gpu_ms=4.0, gpu_sm_mhz=1050.0, on_ac=1.0)])
        self.assertEqual(code, 1, out)
        self.assertIn('[GPU MHz 2100 -> 1050]', out)

    def test_unknown_metric_is_read_and_named(self):
        code, out = guard([v7(record(), test_metric=1.0)], [v7(record(), test_metric=(9.0, 3))])
        self.assertEqual(code, 0, out)
        self.assertIn('metrics this build does not know (not compared): test_metric', out)


if __name__ == '__main__':
    unittest.main()
