#!/usr/bin/env python3
"""One line per pass of report.json files (default: the newest under runs/ and target/).

  scripts/report.py [DIR|report.json]...

Columns: label mode fps p99 gpu-ms cpu-ms psnr bad% repeat; then the pass's stages:
name[nodes] g<gpu ms> c<cpu ms> (frames that ran it, if not all)."""
import glob
import json
import os
import sys

os.chdir(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
args = sys.argv[1:]
if not args:
    found = [f for g in ("runs/*/report.json", "target/*/*/report.json", "target/*/*/*/report.json") for f in glob.glob(g)]
    args = [max(found, key=os.path.getmtime)] if found else []
for a in args:
    f = a if a.endswith(".json") else os.path.join(a, "report.json")
    if not os.path.exists(f):
        c = sorted(glob.glob(a + "/*/report.json"), key=os.path.getmtime)
        f = c[-1] if c else f
    print("#", f)
    for r in json.load(open(f)):
        if r.get("error"):
            print(f"{r['label'][:44]:44} {r['mode']:6} ERROR {r['error'][:150]}")
            continue
        q = r.get("quality") or {}; g = r.get("gpu_ms") or {}; c = r.get("cpu_ms") or {}; fi = r.get("frame_interval_ms") or {}
        ps = q.get("psnr", {}).get("mean"); bad = q.get("bad_pct", {}).get("mean")
        print(f"{r['label'][:44]:44} {r['mode']:6} fps {r['fps']:7.1f} p99 {fi.get('p99', 0):6.2f} gpu {g.get('mean', 0) or 0:6.2f} cpu {c.get('mean', 0) or 0:6.2f}"
              + (f" psnr {ps:5.1f} bad {bad:5.2f}%" if ps is not None else "") + f" r{r.get('repeat', 0)}")
        st = r.get("stages") or []
        if st:
            def ms(x, k):
                v = (x.get(k) or {}).get("mean")
                return f"{k[0]}{v:.3f}" if v is not None else ""
            print("    stages: " + " | ".join(f"{x['name']}[{'+'.join(x['nodes'])}] " + " ".join(filter(None, [ms(x, "gpu_ms"), ms(x, "cpu_ms")])) + (f" ({x['frames']}f)" if x["frames"] < r["frames"] else "") for x in st))
