# sweepcmp.py BASE_SWEEP NEW_SWEEP (e.g. baselines/sweep-20261011 target/sweep/<time>): compares two sweeps per (plan, mode, pipeline, options): GPU ms, fps, PSNR.
# Pipelines are keyed by their canonical graph (`graph_key`, PLAN step 9d), recomputed for every record by
# `multirenderer graph --keys` (MR_BIN, default target/release/multirenderer), so records written before they
# carried one, or under an older canonical form (step 10 placed geometry on the CPU), compare by one key.
import sys, json, glob, gzip, os, subprocess, statistics as st, collections
KERNELS = {'refloor', 'incremental', 'sphere', 'rowrun', 'rayquery'}
DRAWS = {'raster', 'visbuffer', 'gbuffer'}
# Slots added by the architecture refactor (step 7) and their defaults per back end.
ADDED = {'gpu-compute': {'visibility': 'raycast'}, 'cpu': {'visibility': 'raycast'}, 'gpu-rt': {'visibility': 'raycast'},
         'gpu-raster': {'visibility': 'faces', 'draw': 'raster', 'traversal': 'refloor'}, 'cpu-raster': {'visibility': 'faces', 'draw': 'raster'},
         'gpu-discover': {'draw': 'raster', 'traversal': 'refloor'}, 'cpu-discover': {'draw': 'raster', 'traversal': 'refloor'},
         'gpu-splat': {'traversal': 'refloor'}}
def canonical(comp, opts):
    """One form for records from before and after step 7: kernels in `traversal`, the
    rasterisers' and discovery's drawer in `draw`, added slots at their defaults."""
    c, opts = dict(comp), dict(opts)
    if 'traversal' not in c and c.get('visibility') in KERNELS: c['traversal'] = c.pop('visibility')
    if 'draw' not in c and c.get('visibility') in DRAWS: c['draw'] = c.pop('visibility')
    if 'draw' in opts and 'draw' not in c: c['draw'] = opts.pop('draw')
    for k, v in ADDED.get(c.get('backend', 'gpu-compute'), {}).items():
        if not c.get(k): c[k] = v
    return c, opts
def graph_keys(records):
    """Graph keys for every record, computed by the binary (one call for all; a stored key may
    come from an older canonical form)."""
    need = [o for o in records if o.get('composition')]
    if need:
        exe = os.environ.get('MR_BIN', 'target/release/multirenderer')
        text = ''.join(json.dumps(o) + '\n' for o in need)
        keys = subprocess.run([exe, 'graph', '--keys'], input=text, capture_output=True, text=True, check=True).stdout.split()
        assert len(keys) == len(need), (len(keys), len(need))
        for o, k in zip(need, keys):
            if k != '-': o['graph_key'] = k
def load(d):
    records = []
    for f in glob.glob(d + '/*/observations.jsonl*'):
        op = gzip.open if f.endswith('.gz') else open
        for line in op(f, 'rt'):
            o = json.loads(line)
            if not o.get('composition'): continue
            o['composition'], o['options'] = canonical(o['composition'], o.get('options') or {})
            records.append(o)
    graph_keys(records)
    out = {}
    for o in records:
        # The composition stays in the key's display part (labels below); the graph key decides.
        k = (o['plan'], o['mode'], o.get('graph_key') or json.dumps(o['composition'], sort_keys=True), json.dumps(o['options'], sort_keys=True), json.dumps(o['composition'], sort_keys=True))
        out[k[:4]] = (o, k[4])
    return out
base, new = load(sys.argv[1]), load(sys.argv[2])
shown = {k: v[1] for d in (base, new) for k, v in d.items()}
base = {k: v[0] for k, v in base.items()}
new = {k: v[0] for k, v in new.items()}
common = [k for k in base if k in new]
print(f'base {len(base)} obs, new {len(new)} obs, common {len(common)}, only new {len(set(new)-set(base))}, only base {len(set(base)-set(new))}')
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
    if d < -0.5: print(f'  {d:+.2f} dB  {k[0]} {k[1]} {json.loads(shown[k])} {k[3]}  {a:.1f} -> {b:.1f}')
print('GPU ms worst 20:')
for r, _, k, a, b in sorted(worst, reverse=True)[:20]:
    print(f'  {r:.3f}  {k[0]:15s} {k[1]:10s} {a:7.3f} -> {b:7.3f}  {json.loads(shown[k])} {k[3] if k[3] != "{}" else ""}')
print('new failures:', len(fails))
for k, e in fails[:10]: print('  ', k[0], k[1], json.loads(shown[k]), e)
# per back end medians
byb = collections.defaultdict(list)
for r, _, k, a, b in worst: byb[json.loads(shown[k])['backend']].append(r)
print('GPU ms median ratio by back end:', {b: round(st.median(v), 4) for b, v in sorted(byb.items())})
