# sweepcmp.py BASE_SWEEP NEW_SWEEP (e.g. baselines/sweep-20261011 target/sweep/<time>): compares two sweeps per (plan, mode, mesh, composition): GPU ms, fps, PSNR.
import sys, json, glob, gzip, statistics as st, collections
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
def load(d):
    out = {}
    for f in glob.glob(d + '/*/observations.jsonl*'):
        op = gzip.open if f.endswith('.gz') else open
        for line in op(f, 'rt'):
            o = json.loads(line)
            c, opts = canonical(o['composition'], o.get('options') or {})
            k = (o['plan'], o['mode'], json.dumps(c, sort_keys=True), json.dumps(opts, sort_keys=True))
            out[k] = o
    return out
base, new = load(sys.argv[1]), load(sys.argv[2])
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
    if d < -0.5: print(f'  {d:+.2f} dB  {k[0]} {k[1]} {json.loads(k[2])} {k[3]}  {a:.1f} -> {b:.1f}')
print('GPU ms worst 20:')
for r, _, k, a, b in sorted(worst, reverse=True)[:20]:
    print(f'  {r:.3f}  {k[0]:15s} {k[1]:10s} {a:7.3f} -> {b:7.3f}  {json.loads(k[2])} {k[3] if k[3] != "{}" else ""}')
print('new failures:', len(fails))
for k, e in fails[:10]: print('  ', k[0], k[1], json.loads(k[2]), e)
# per back end medians
byb = collections.defaultdict(list)
for r, _, k, a, b in worst: byb[json.loads(k[2])['backend']].append(r)
print('GPU ms median ratio by back end:', {b: round(st.median(v), 4) for b, v in sorted(byb.items())})
