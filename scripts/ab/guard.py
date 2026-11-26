#!/usr/bin/env python3
"""guard.py [--base DIR] NEW [--speed PCT] [--abs-ms MS] [--fps PCT] [--db DB] [--floor DB] [--all]

The regression guard: compares a sweep (NEW, a directory of runs such as target/sweep/<time>
written by sweep.py, or target/turn/<time> by turn-sweep.py) with the baseline sweep (either
kind), pass by pass (plan, mode, pipeline by graph key, knobs), and prints only what moved
beyond noise:

- speed: GPU ms where both sides measured it, else fps (CPU pipelines). Beyond noise means
  slower by more than --speed % (default 10) and --abs-ms (default 0.01 ms, timer granularity
  for very fast passes) for GPU ms, --fps % (default 10) for fps. Sweeps run each pass once,
  and one pass varies by several percent between sweeps (the median pass by under 1 %, the
  tail by far more), so a pass's verdict is a candidate to confirm with perf.py (interleaved
  runs), not a measurement. Where both sides repeated a pass (run.repeats > 1), the threshold
  is at least three times the passes' own spread.
- pixels: the score the catalogue's floor uses (psnr_settled, else psnr_like, else psnr)
  dropping by more than --db dB (default 0.5), or crossing the --floor (default 35 dB);
  bad pixels growing by more than 1 percentage point.
- failures: passes that failed in NEW but not in the baseline (and the reverse, as fixes).

Then the totals and the median speed ratio over all passes (a shift of the whole machine
shows there, not in one pipeline). The robust check is per component: the median speed ratio
of the passes whose pipeline has it, against the median over all; a component moving more
than --shift % (default 2) is listed, with the median of the passes without it (a feature that
costs pipelines without it moves those). Also the components most common among the slowed
pipelines. Passes in only one side are counted and, with --all, listed. Performance data is background
work (the idle queue), so a pass may lack speed or a score on a side: it is still compared on
what both sides have, and the passes left uncompared are counted.

Metrics are an open map with a method version each (`multirenderer metrics`): a metric is compared only
where both sides measured it the same way; a changed method is printed as such ("measured another way"),
never as slower or faster. Every other metric with a better direction is compared by its median ratio
over the common passes and listed when it moves more than --shift %; metrics the binary does not know
are named, not compared. A pass slower or faster beyond noise shows the machine's conditions
that differ by more than 10 % between the sides (GPU clock, performance state, power, mains, CPU
clock; recorded per pass since observation version 7), what could explain the jump.

The baseline is the newest `baselines/sweep-*` unless --base names one. Exit 0 when nothing
regressed, 1 when something did, 2 when nothing could be compared. Records are read by
records.py (MR_BIN names the binary that recomputes graph keys): it prints the observation
versions read (newer than the binary: refused by name), the rename pairs of
baselines/renames.toml it applied, and the records it left out with the reason.
"""
import argparse, collections, glob, os, statistics as st, sys

sys.path.insert(0, os.path.dirname(__file__))
from records import load_all, registry

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def default_base():
    dirs = sorted(glob.glob(os.path.join(ROOT, 'baselines', 'sweep-*')))
    return dirs[-1] if dirs else None


def score(o):
    """The score the catalogue's floor applies, and its bad-pixel share."""
    m = o['metrics']
    for s, b in (('psnr_settled', 'bad_pct_settled'), ('psnr_like', 'bad_pct_like'), ('psnr', 'bad_pct')):
        if m.get(s) is not None:
            return s, m[s], m.get(b)
    return None, None, None


def spread(rs, field):
    """Relative spread (stdev / mean) of a metric over a pass's repeats, None with fewer than two."""
    v = [o['metrics'][field] for o in rs if o['metrics'].get(field)]
    return st.stdev(v) / st.mean(v) if len(v) >= 2 else None


def mean(rs, field):
    v = [o['metrics'][field] for o in rs if o['metrics'].get(field) is not None]
    return st.mean(v) if v else None


def method(rs, field):
    """The method versions a pass's records measured a metric by (a set; empty: not measured)."""
    return {(o.get('methods') or {}).get(field, 1) for o in rs if o['metrics'].get(field) is not None}


def same_method(b, n, field):
    """Both sides measured the metric, the same way: comparable."""
    mb, mn = method(b, field), method(n, field)
    return bool(mb) and mb == mn


CONDITIONS = (('gpu_sm_mhz', 'GPU MHz'), ('gpu_pstate', 'P'), ('gpu_power_w', 'W'), ('on_ac', 'mains'), ('cpu_mhz', 'CPU MHz'))


def conditions(b, n):
    """The machine's conditions that differ between the sides' last records (clocks, power state,
    power source): what could explain a speed change. Empty when neither recorded them."""
    mb, mn = b[-1]['metrics'], n[-1]['metrics']
    diff = [f'{label} {mb[f]:g} -> {mn[f]:g}' for f, label in CONDITIONS
            if mb.get(f) is not None and mn.get(f) is not None and abs(mn[f] - mb[f]) > 0.1 * max(abs(mb[f]), 1)]
    return f'  [{"; ".join(diff)}]' if diff else ''


def components(o):
    return {c for s in o.get('stages') or [] for c in s.get('components', [])}


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[1], formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('new')
    ap.add_argument('--base', default=default_base())
    ap.add_argument('--speed', type=float, default=10.0, help='GPU ms threshold, percent')
    ap.add_argument('--abs-ms', type=float, default=0.01, help='GPU ms threshold, absolute')
    ap.add_argument('--fps', type=float, default=10.0, help='fps threshold, percent (no GPU ms)')
    ap.add_argument('--db', type=float, default=0.5, help='score drop threshold, dB')
    ap.add_argument('--floor', type=float, default=35.0, help='the catalogue floor, dB')
    ap.add_argument('--shift', type=float, default=2.0, help='per-component median shift, percent')
    ap.add_argument('--all', action='store_true', help='also list passes in one side only')
    a = ap.parse_args()
    if not a.base:
        sys.exit('no baseline: baselines/sweep-* is empty; name one with --base')
    print(f'base {os.path.relpath(a.base)}  new {os.path.relpath(a.new)}')
    base, bu, bv = load_all(a.base)
    new, nu, nv = load_all(a.new)
    versions = lambda v: ', '.join(f'v{k}: {n}' for k, n in v.items()) or 'none'
    print(f'observation versions: base {versions(bv)}; new {versions(nv)}')
    common = sorted(k for k in base if k in new)
    print(f'passes: base {len(base)}, new {len(new)}, common {len(common)}, only base {len(set(base) - set(new))}, only new {len(set(new) - set(base))}')
    if not common:
        print('nothing in common to compare')
        return 2
    slow, fast, worse, better, failed, fixed, ratios = [], [], [], [], [], [], []
    label, unmeasured, changed = {}, collections.Counter(), collections.Counter()
    reg, other = registry(), collections.defaultdict(list)
    unknown = sorted({f for side in (base, new) for k in common for o in side[k] for f in o['metrics'] if f not in reg})
    for k in common:
        b, n = base[k], new[k]
        label[k] = n[-1]['label']
        be, ne = any(o.get('error') for o in b), any(o.get('error') for o in n)
        if ne and not be: failed.append((k, next(o['error'] for o in n if o.get('error'))))
        if be and not ne: fixed.append((k, ''))
        if be or ne: continue
        gb, gn = mean(b, 'gpu_ms'), mean(n, 'gpu_ms')
        fb, fn = mean(b, 'fps'), mean(n, 'fps')
        r = None
        for f in ('gpu_ms', 'fps'):
            if method(b, f) and method(n, f) and not same_method(b, n, f):
                changed[f] += 1  # measured another way: not a speed change
                if f == 'gpu_ms': gb = gn = None
                else: fb = fn = None
        for f, m in reg.items():
            if f in ('gpu_ms', 'fps') or m['better'] == 'neither' or not same_method(b, n, f): continue
            vb, vn = mean(b, f), mean(n, f)
            if vb and vn and vb > 0 and vn > 0:
                other[f].append(vn / vb if m['better'] == 'lower' else vb / vn)  # above 1 is worse
        if gb and gn:
            r, rel, what = gn / gb, a.speed / 100, 'GPU ms'
            noise = max(rel, a.abs_ms / gb, 3 * max(spread(b, 'gpu_ms') or 0, spread(n, 'gpu_ms') or 0))
            vb, vn = gb, gn
        elif fb and fn:
            r, what = fb / fn, 'fps'  # as a time ratio: above 1 is slower
            noise = max(a.fps / 100, 3 * max(spread(b, 'fps') or 0, spread(n, 'fps') or 0))
            vb, vn = fb, fn
        else:
            unmeasured['speed'] += 1  # performance data is background work: a side may lack it
        if r is not None:
            ratios.append((r, k))
            if r > 1 + noise: slow.append((r, k, what, vb, vn))
            elif r < 1 / (1 + noise): fast.append((r, k, what, vb, vn))
        s, sb, bb = score(b[-1])
        _, sn, bn = score(n[-1])
        if sb is None or sn is None:
            unmeasured['pixels'] += 1
        else:
            d = sn - sb
            crossed = sb >= a.floor > sn
            grew = bb is not None and bn is not None and bn - bb > 1.0
            if d < -a.db or crossed or grew: worse.append((d, k, s, sb, sn, bb, bn, crossed))
            elif d > a.db or sn >= a.floor > sb: better.append((d, k, s, sb, sn, bb, bn, False))

    def name(k):
        opts = '' if k[3] == '{}' else ' ' + k[3]
        return f'{k[0]} {k[1]} {label[k]}{opts}'

    def speed(rows, title):
        if rows:
            print(f'{title}: {len(rows)}')
            for r, k, what, vb, vn in sorted(rows, reverse=(title.startswith('slower'))):
                print(f'  {100 * (r - 1):+6.1f}%  {what} {vb:.3f} -> {vn:.3f}  {name(k)}{conditions(base[k], new[k])}')

    def pixels(rows, title):
        if rows:
            print(f'{title}: {len(rows)}')
            for d, k, s, sb, sn, bb, bn, crossed in sorted(rows, reverse=title.startswith('better')):
                bad = f'  bad {bb:.2f} -> {bn:.2f}%' if bb is not None and bn is not None else ''
                print(f'  {d:+6.2f} dB  {s} {sb:.1f} -> {sn:.1f}{bad}{"  below the floor" if crossed else ""}  {name(k)}')

    speed(slow, 'slower beyond noise')
    pixels(worse, 'pixels worse')
    if failed:
        print(f'failing now: {len(failed)}')
        for k, e in failed: print(f'  {name(k)}: {e[:160]}')
    speed(fast, 'faster beyond noise')
    pixels(better, 'pixels better')
    if fixed:
        print(f'passing now: {len(fixed)}')
        for k, _ in fixed: print(f'  {name(k)}')
    if a.all:
        for side, mine, other in (('base', base, new), ('new', new, base)):
            for k in sorted(set(mine) - set(other)):
                print(f'  only {side}: {k[0]} {k[1]} {mine[k][-1]["label"]}{"" if k[3] == "{}" else " " + k[3]}')
    if ratios:
        overall = st.median(r for r, _ in ratios)
        print(f'speed new/base over {len(ratios)} passes: median {overall:.3f} (above 1 is slower)')
        by = collections.defaultdict(list)
        for r, k in ratios:
            for c in components(new[k][-1]): by[c].append(r)
        moved = []
        for c, v in by.items():
            m = st.median(v)
            if len(v) >= 5 and abs(m / overall - 1) > a.shift / 100:
                rest = [r for r, k in ratios if c not in components(new[k][-1])]
                moved.append((m, c, len(v), st.median(rest) if rest else float('nan')))
        if moved:
            print('components moving the speed (median with it, passes, median without it):')
            for m, c, n_, rest in sorted(moved, reverse=True):
                print(f'  {c:28s} {m:.3f}  {n_:5d}  {rest:.3f}')
    if slow:
        have = collections.Counter(c for k in common for c in components(new[k][-1]))
        hit = collections.Counter(c for _, k, *_ in slow for c in components(new[k][-1]))
        top = sorted(hit, key=lambda c: (-hit[c] / len(slow), c))[:8]
        print('components of the slowed pipelines (slowed with it / pipelines with it):',
              ', '.join(f'{c} {hit[c]}/{have[c]}' for c in top))
    moved = sorted((st.median(v), f, len(v)) for f, v in other.items() if len(v) >= 5 and abs(st.median(v) - 1) > a.shift / 100)
    if moved:
        print('other metrics moving (median worse/better ratio, above 1 is worse; passes):')
        for m, f, n_ in sorted(moved, reverse=True):
            print(f'  {f:28s} {m:.3f}  {n_:5d}  {reg[f]["unit"]}')
    if changed:
        print('measured another way (method version changed; not compared as speed): '
              + ', '.join(f'{f} {n_} passes' for f, n_ in sorted(changed.items())))
    if unknown:
        print(f'metrics this build does not know (not compared): {", ".join(unknown)}')
    if unmeasured:
        print('not compared (measured on one side only or neither): '
              + ', '.join(f'{what} {n_} passes' for what, n_ in sorted(unmeasured.items())))
    regressions = len(slow) + len(worse) + len(failed)
    print(f'regressions {regressions} (slower {len(slow)}, pixels {len(worse)}, failing {len(failed)}); '
          f'improvements {len(fast) + len(better) + len(fixed)}')
    return 1 if regressions else 0


if __name__ == '__main__':
    sys.exit(main())
