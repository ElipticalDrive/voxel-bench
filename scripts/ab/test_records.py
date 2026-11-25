"""records.py across turns: rename pairs, records naming a removed component, newer versions.

Run by `scripts/check.py test` (MR_BIN, default the dev build's binary); alone:
`python3 -m unittest discover -s scripts/ab -p 'test_*.py'`.
"""
import contextlib, io, json, os, sys, tempfile, unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import records

ROOT = os.path.dirname(os.path.dirname(HERE))
os.environ.setdefault('MR_BIN', os.path.join(ROOT, 'target', 'debug', 'multirenderer'))
with open(os.path.join(HERE, 'testdata', 'record.json')) as f:
    RECORD = json.load(f)  # gpu: bricks + refloor + ...


def sweep(d, recs):
    os.makedirs(os.path.join(d, 'run'), exist_ok=True)
    with open(os.path.join(d, 'run', 'observations.jsonl'), 'w') as f:
        for r in recs: f.write(json.dumps(r) + '\n')


def record(run='20261101-000000-x', **edit):
    o = json.loads(json.dumps(RECORD))
    o['run'] = run
    for node, (field, value) in edit.items():
        o['graph']['nodes'][node][field] = value
    return o


class Records(unittest.TestCase):
    def load(self, recs, renames=''):
        with tempfile.TemporaryDirectory() as d:
            sweep(os.path.join(d, 's'), recs)
            with open(os.path.join(d, 'renames.toml'), 'w') as f:
                f.write(renames)
            os.environ['RENAMES'] = os.path.join(d, 'renames.toml')
            out = io.StringIO()
            try:
                with contextlib.redirect_stdout(out):
                    got = records.load_all(os.path.join(d, 's'))
            finally:
                del os.environ['RENAMES']
            return got, out.getvalue()

    def test_removed_component_counted_with_reason(self):
        (out, unread, _), text = self.load([record(), record(kernel=('component', 'gone'))])
        self.assertEqual((len(out), unread), (1, 1))
        self.assertIn('unknown traversal component `gone`', text)

    def test_component_rename_keys_as_before(self):
        (real, _, _), _ = self.load([record()])
        renamed = '[[rename]]\ndate = 2026-11-24\nrole = "traversal"\ncomponent = "gone=refloor"\n'
        (out, unread, _), text = self.load([record(kernel=('component', 'gone'))], renamed)
        self.assertEqual((list(out), unread), (list(real), 0))
        self.assertIn('renamed traversal component gone -> refloor (before 20261124) in 1 records', text)
        # A record of the renaming turn or later keeps its names: `gone` there is not `refloor`.
        (out, unread, _), _ = self.load([record('20261124-000000-x', kernel=('component', 'gone'))], renamed)
        self.assertEqual((len(out), unread), (0, 1))

    def test_knob_rename_keys_as_before(self):
        (real, _, _), _ = self.load([record(storage=('knobs', {'pool_mb': 64}))])
        renamed = '[[rename]]\ndate = 2026-11-24\nrole = "storage"\ncomponent = "bricks"\nknob = "pool=pool_mb"\n'
        (out, _, _), text = self.load([record(storage=('knobs', {'pool': 64}))], renamed)
        self.assertEqual(list(out), list(real))
        self.assertIn('renamed storage bricks knob pool -> pool_mb', text)

    def test_bad_renames_refused(self):
        for bad in ('role = "storage"\ncomponent = "a=b"', 'date = 2026-11-24\ncomponent = "a=b"',
                    'date = 2026-11-24\nrole = "storage"\ncomponent = "a"',
                    'date = 2026-11-24\nrole = "storage"\ncomponent = "a=b"\nwhy = 1'):
            with self.assertRaises(SystemExit):
                self.load([record()], '[[rename]]\n' + bad + '\n')

    def test_newer_version_refused_by_name(self):
        o = record()
        o['version'] = 99
        with self.assertRaises(SystemExit) as e:
            self.load([o])
        self.assertIn('version 99', str(e.exception))


if __name__ == '__main__':
    unittest.main()
