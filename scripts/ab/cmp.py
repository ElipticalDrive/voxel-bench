# cmp.py BEFORE_DIR AFTER_DIR: compares saved frames of two runs pixel by pixel.
import sys, os, glob
from PIL import Image
import numpy as np
a, b = sys.argv[1:3]
fa = sorted(f for f in glob.glob(a + '/*/frames/*/*') if not f.endswith('-diff.png'))
same = 0
for f in fa:
    rel = f.split('/frames/')[1]
    g = glob.glob(b + '/*/frames/' + rel)
    if not g: print('missing', rel); continue
    x = np.asarray(Image.open(f).convert('RGB')).astype(int); y = np.asarray(Image.open(g[0]).convert('RGB')).astype(int)
    d = np.abs(x - y); n = (d.max(axis=2) > 0).sum()
    if n == 0: same += 1; continue
    print(f'{rel:70s} diff px {n:8d}  max {d.max()}')
print(f'{same} of {len(fa)} frames identical')
