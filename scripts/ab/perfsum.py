# perfsum.py DIR: median GPU ms and fps per pass of interleaved before/after runs.
# AB_RENAME="old=new": the before side's labels with `old` replaced by `new` (a graph that
# gained nodes has a new label).
import os, re, sys, json, glob, statistics as st
T = sys.argv[1]; res = {}
old, _, new = os.environ.get('AB_RENAME', '').partition('=')
for b in ('before', 'after'):
    for f in glob.glob(f'{T}/{b}/*/*/report.json'):
        for p in json.load(open(f)):
            if p.get('error'): print('ERROR', b, p['label'], p['error']); continue
            g = p['gpu_ms']['mean'] if p.get('gpu_ms') else float('nan')
            label = p['label'].replace(old, new) if old and b == 'before' else p['label']
            res.setdefault((re.sub(r' \[(native|break-in)\]$', '', label), p['mode']), {}).setdefault(b, []).append((g, p['fps']))
w = max([len(k[0]) + 1 + len(k[1]) for k in res] + [4])
print(f"{'pass':{w}s} {'gpu before':>10s} {'after':>8s} {'Δ%':>6s} {'fps before':>10s} {'after':>8s} {'Δ%':>6s}")
for k, v in sorted(res.items()):
    if 'before' not in v or 'after' not in v: continue
    gb = st.median(x[0] for x in v['before']); ga = st.median(x[0] for x in v['after'])
    fb = st.median(x[1] for x in v['before']); fa = st.median(x[1] for x in v['after'])
    print(f"{k[0]+' '+k[1]:{w}s} {gb:10.3f} {ga:8.3f} {100*(ga/gb-1):+6.1f} {fb:10.1f} {fa:8.1f} {100*(fa/fb-1):+6.1f}")
