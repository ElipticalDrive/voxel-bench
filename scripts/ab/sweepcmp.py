# sweepcmp.py BASE_SWEEP NEW_SWEEP (two directories of runs, e.g. target/sweep/<time>; the baseline of 2026-10-11 names its pipelines in a form no longer read): compares two sweeps per (plan, mode, pipeline, options): GPU ms, fps, PSNR.
# Records are read and keyed by records.py (the graph key recomputed by the binary; see there).
import sys, os, statistics as st, collections
sys.path.insert(0, os.path.dirname(__file__))
from records import load
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
