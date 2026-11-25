# records.py: reads the observation records of a sweep (a directory of runs, each with an
# `observations.jsonl[.gz]`), keyed per pass by (plan, mode, graph key, options). Used by
# sweepcmp.py and guard.py.
# Pipelines are keyed by their canonical graph (`graph_key`) and shown by their graph's label, both recomputed for
# every record by `multirenderer graph --keys` (MR_BIN, default target/release/multirenderer), so records written
# under an older canonical form compare by one key. Records the binary cannot key (a pipeline from before
# version 6, the flat `composition`; a component since removed) are left out, counted by the binary's reason.
# Records newer than the binary reads (a `version` above its observations' version) are refused by name:
# rebuild it.
# Names changed between turns are pairs in `baselines/renames.toml` (RENAMES names another file), each
# applied to the records written before its date, so a renamed component or knob keys as it did:
#   [[rename]]
#   date = 2026-11-24          # the turn that renamed it: records of runs before this day read old as new
#   role = "storage"           # the node's role
#   component = "bricks=tiles" # a component renamed, old=new
#   [[rename]]
#   date = 2026-11-24
#   role = "storage"
#   component = "tiles"        # optional: only this component's knob (else every component of the role)
#   knob = "pool=pool_mb"      # a knob renamed, old=new (in the node and in the record's options)
# Pairs naming no date or role, without old=new, or with an unknown key are refused. The guard prints each
# pair it applied and how often.
import collections, glob, gzip, json, os, subprocess, sys, tomllib

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def renames(path=None):
    """The rename pairs: [(date 'YYYYMMDD', role, component or None, kind, old, new)]."""
    path = path or os.environ.get('RENAMES') or os.path.join(ROOT, 'baselines', 'renames.toml')
    if not os.path.exists(path):
        return []
    out = []
    with open(path, 'rb') as f:
        for i, r in enumerate(tomllib.load(f).get('rename', []), 1):
            bad = set(r) - {'date', 'role', 'component', 'knob'}
            if bad or 'date' not in r or 'role' not in r or ('knob' in r) == ('=' in r.get('component', '')):
                sys.exit(f'{path}: rename {i}: needs date, role and either component = "old=new" or knob = "old=new"'
                         f'{" (unknown " + ", ".join(sorted(bad)) + ")" if bad else ""}')
            kind, pair = ('knob', r['knob']) if 'knob' in r else ('component', r['component'])
            old, eq, new = pair.partition('=')
            if not (eq and old and new):
                sys.exit(f'{path}: rename {i}: `{pair}` is not old=new')
            comp = r.get('component') if kind == 'knob' else None
            out.append((r['date'].strftime('%Y%m%d'), r['role'], comp, kind, old, new))
    return out


def rename(o, pairs, used):
    """Applies the pairs dated after the record's run to its graph and options; counts each in `used`."""
    day = (o.get('run') or '')[:8]
    nodes = (o.get('graph') or {}).get('nodes') or {}
    for p in pairs:
        date, role, comp, kind, old, new = p
        if not day or day >= date:
            continue
        for n in nodes.values():
            if n.get('role') != role:
                continue
            if kind == 'component' and n.get('component') == old:
                n['component'] = new
                used[p] += 1
            elif kind == 'knob' and comp in (None, n.get('component')) and old in (n.get('knobs') or {}):
                n['knobs'][new] = n['knobs'].pop(old)
                if old in o['options']: o['options'][new] = o['options'].pop(old)
                used[p] += 1


def graph_keys(records):
    """Graph keys and labels for every record, computed by the binary (one call for all; a stored key
    may come from an older canonical form, a stored label from the flat form)."""
    if records:
        exe = os.environ.get('MR_BIN', 'target/release/multirenderer')
        text = ''.join(json.dumps(o) + '\n' for o in records)
        run = subprocess.run([exe, 'graph', '--keys'], input=text, capture_output=True, text=True)
        if run.returncode:
            sys.exit(f'{exe} graph --keys: {run.stderr.strip()}')
        lines = run.stdout.splitlines()
        assert len(lines) == len(records), (len(lines), len(records))
        for o, line in zip(records, lines):
            k, label, why = (line.split('\t') + ['', ''])[:3]
            if k != '-': o['graph_key'] = k
            else: o['unread'] = why or 'no reason given'
            if label: o['label'] = label


def key(o):
    return (o['plan'], o['mode'], o['graph_key'], json.dumps(o['options'], sort_keys=True))


def load_all(d):
    """{key: [records…]} (several when the sweep repeated passes), how many were left out, and how many
    records of each observation version were read ({version: count})."""
    records, pairs, used = [], renames(), collections.Counter()
    for f in sorted(glob.glob(d + '/*/observations.jsonl*')):
        op = gzip.open if f.endswith('.gz') else open
        with op(f, 'rt') as lines:
            recs = [json.loads(line) for line in lines]
        for o in recs:
            if not (o.get('graph') or o.get('composition')): continue  # a standalone renderer
            o['options'] = o.get('options') or {}
            o.pop('graph_key', None)
            rename(o, pairs, used)
            records.append(o)
    for (date, role, comp, kind, old, new), n in sorted(used.items()):
        print(f'{d}: renamed {role} {comp + " " if comp else ""}{kind} {old} -> {new} (before {date}) in {n} records')
    graph_keys(records)
    versions = collections.Counter(o.get('version') for o in records)
    out = {}
    for o in records:
        if 'graph_key' in o: out.setdefault(key(o), []).append(o)
    why = collections.Counter(o['unread'] for o in records if 'graph_key' not in o)
    unread = sum(why.values())
    if unread:
        print(f'{d}: {unread} of {len(records)} records left out (this build cannot key them):')
        for w, n in why.most_common(): print(f'  {n:6d}  {w}')
    return out, unread, dict(sorted(versions.items(), key=lambda kv: kv[0] or 0))


def load(d):
    """{key: (record, label)}, the last record of each pass."""
    return {k: (v[-1], v[-1]['label']) for k, v in load_all(d)[0].items()}
