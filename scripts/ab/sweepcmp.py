# sweepcmp.py BASE_SWEEP NEW_SWEEP (two directories of runs, e.g. target/sweep/<time>; the baseline of 2026-10-11 names its pipelines in a form no longer read): compares two sweeps per (plan, mode, pipeline, options): GPU ms, fps, PSNR.
# Pipelines are keyed by their canonical graph (`graph_key`) and shown by their graph's label, both recomputed for
# every record by `multirenderer graph --keys` (MR_BIN, default target/release/multirenderer), so records written
# under an older canonical form (geometry later placed on the CPU) compare by one key. Records of pipelines from
# before version 6 (the flat `composition`) are not read by the binary: left out and counted.
import sys, json, glob, gzip, os, subprocess, statistics as st, collections
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
def load(d):
    records = []
    for f in glob.glob(d + '/*/observations.jsonl*'):
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
        if 'graph_key' not in o: continue
        # The graph key decides; the label is what is shown.
        k = (o['plan'], o['mode'], o['graph_key'], json.dumps(o['options'], sort_keys=True))
        out[k] = (o, o['label'])
    unread = sum(1 for o in records if 'graph_key' not in o)
    if unread: print(f'{d}: {unread} of {len(records)} records left out (pipelines in a form this build no longer reads)')
    return out
base, new = load(sys.argv[1]), load(sys.argv[2])
shown = {k: v[1] for d in (base, new) for k, v in d.items()}
base = {k: v[0] for k, v in base.items()}
new = {k: v[0] for k, v in new.items()}
common = [k for k in base if k in new]
print(f'base {len(base)} obs, new {len(new)} obs, common {len(common)}, only new {len(set(new)-set(base))}, only base {len(set(base)-set(new))}')
if not common: sys.exit('nothing in common to compare')
rg, rf, dq = [], [], []
worst = []
fails = []
for k in common:
    b, n = base[k], new[k]
    if n.get('error') and not b.get('error'): fails.append((k, n['error'][:120]))
    if b.get('error') or n.get('error'): continue
    if b.get('gpu_ms') and n.get('gpu_ms'):
        r = n['gpu_ms'] / b['gpu_ms']; rg.append(r); worst.append((r, 'gpu', k, b['gpu_ms'], n['gpu_ms']))
    if b.get('fps') and n.get('fps'):
        rf.append(n['fps'] / b['fps'])
    if b.get('psnr') is not None and n.get('psnr') is not None:
        d = n['psnr'] - b['psnr']; dq.append((d, k, b['psnr'], n['psnr']))
def summ(name, v):
    v = sorted(v); n = len(v)
    within = lambda t: sum(1 for x in v if abs(x - 1) <= t) / n * 100
    print(f'{name}: n={n} median {st.median(v):.4f}  p10 {v[n//10]:.3f} p90 {v[9*n//10]:.3f}  within ±1%: {within(.01):.0f}%  ±2%: {within(.02):.0f}%  ±4%: {within(.04):.0f}%')
summ('GPU ms new/base', rg)
summ('fps   new/base', rf)
print('PSNR drops > 0.5 dB:', sum(1 for d in dq if d[0] < -0.5), 'of', len(dq))
for d, k, a, b in sorted(dq)[:12]:
    if d < -0.5: print(f'  {d:+.2f} dB  {k[0]} {k[1]} {shown[k]} {k[3]}  {a:.1f} -> {b:.1f}')
print('GPU ms worst 20:')
for r, _, k, a, b in sorted(worst, reverse=True)[:20]:
    print(f'  {r:.3f}  {k[0]:15s} {k[1]:10s} {a:7.3f} -> {b:7.3f}  {shown[k]} {k[3] if k[3] != "{}" else ""}')
print('new failures:', len(fails))
for k, e in fails[:10]: print('  ', k[0], k[1], shown[k], e)
# per executor medians
byb = collections.defaultdict(list)
for r, _, k, a, b in worst: byb[shown[k].split(':')[0]].append(r)
print('GPU ms median ratio by executor:', {b: round(st.median(v), 4) for b, v in sorted(byb.items())})
