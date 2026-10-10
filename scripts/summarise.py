import json,glob,os,sys
base=sys.argv[1]
for d in sorted(glob.glob(base+'/*/'),key=os.path.getmtime):
    rp=d+'report.json'
    if not os.path.exists(rp): continue
    name=os.path.basename(d.rstrip('/')).split('-',2)[2]
    print(f"\n## {name}")
    for r in json.load(open(rp)):
        if r['label'].endswith('(ref)'): continue
        q=r.get('quality') or {}
        g=r.get('gpu_ms') or {}
        fi=r.get('frame_interval_ms') or {}
        if r.get('error'):
            print(f"{r['label']:22} {str(r['mode']):6} FAILED {r['error'][:100]}")
            continue
        print(f"{r['label']:22} {r['mode']:6} fps {r['fps']:7.1f}  p99 {fi.get('p99',0):7.2f}ms  gpu {g.get('mean',0):5.2f}ms  psnr {q.get('psnr',{}).get('mean',0):5.1f}  bad {q.get('bad_pct',{}).get('mean',0):5.2f}%  vram {r['memory']['gpu_bytes']/2**20:6.0f}MB")
