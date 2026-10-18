#!/usr/bin/env python3
"""Saves the current release build as the "before" side of A/B checks (run before a change)."""
import os
import shutil
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from mr import ROOT, build_release, release

if not build_release():
    sys.exit(1)
dst = ROOT / "target" / "ab-before" / release().name
dst.parent.mkdir(parents=True, exist_ok=True)
shutil.copy2(release(), dst)
print(f"saved {dst.relative_to(ROOT)}")
