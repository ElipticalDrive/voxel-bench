#!/usr/bin/env python3
"""Runs every plan once into target/catalogue; summarise with scripts/summarise.py target/catalogue."""
import re
import subprocess
import sys
import time

sys.path.insert(0, __import__("os").path.dirname(__file__))
from mr import ROOT, SCENARIOS, release
if {"-h", "--help"} & set(sys.argv[1:(sys.argv + ["--"]).index("--")]):  # help, never a run
    sys.exit(print(__doc__))

for p in SCENARIOS:
    print(f"=== {p} {time.strftime('%H:%M')}", flush=True)
    out = subprocess.run([str(release()), "run", f"plans/{p}.toml", "-s", 'output.dir="target/catalogue"', "-s", "output.save.every=0"], cwd=ROOT, capture_output=True, text=True, errors="replace")
    print("\n".join([l for l in (out.stdout + out.stderr).splitlines() if re.search("FAILED|ERROR|uncleanly|panicked", l)][:5]))
print("SUITE-DONE")
