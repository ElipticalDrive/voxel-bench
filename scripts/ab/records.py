# records.py: reads the observation records of a sweep (a directory of runs, each with an
# `observations.jsonl[.gz]`), keyed per pass by (plan, mode, graph key, options). Used by
# sweepcmp.py and guard.py.
# Pipelines are keyed by their canonical graph (`graph_key`) and shown by their graph's label, both recomputed for
# every record by `multirenderer graph --keys` (MR_BIN, default target/release/multirenderer), so records written
# under an older canonical form compare by one key. Records of pipelines from before version 6 (the flat
# `composition`) are not read by the binary: left out and counted.
import glob, gzip, json, os, subprocess


def graph_keys(records):
    """Graph keys and labels for every record, computed by the binary (one call for all; a stored key
    may come from an older canonical form, a stored label from the flat form)."""
    if records:
        exe = os.environ.get('MR_BIN', 'target/release/multirenderer')
        text = ''.join(json.dumps(o) + '\n' for o in records)
        lines = subprocess.run([exe, 'graph', '--keys'], input=text, capture_output=True, text=True, check=True).stdout.splitlines()
        assert len(lines) == len(records), (len(lines), len(records))
        for o, line in zip(records, lines):
            k, _, label = line.partition('\t')
            if k != '-': o['graph_key'] = k
            if label: o['label'] = label


def key(o):
    return (o['plan'], o['mode'], o['graph_key'], json.dumps(o['options'], sort_keys=True))


def load_all(d):
    """{key: [records…]} (several when the sweep repeated passes) and how many were left out."""
    records = []
    for f in sorted(glob.glob(d + '/*/observations.jsonl*')):
        op = gzip.open if f.endswith('.gz') else open
        for line in op(f, 'rt'):
            o = json.loads(line)
            if not (o.get('graph') or o.get('composition')): continue  # a standalone renderer
            o['options'] = o.get('options') or {}
            o.pop('graph_key', None)
            records.append(o)
    graph_keys(records)
    out = {}
    for o in records:
        if 'graph_key' in o: out.setdefault(key(o), []).append(o)
    unread = sum(1 for o in records if 'graph_key' not in o)
    if unread: print(f'{d}: {unread} of {len(records)} records left out (pipelines in a form this build no longer reads)')
    return out, unread


def load(d):
    """{key: (record, label)}, the last record of each pass."""
    return {k: (v[-1], v[-1]['label']) for k, v in load_all(d)[0].items()}
