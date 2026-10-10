#!/bin/bash
# Runs every plan once into target/catalogue; summarise with scripts/summarise.py target/catalogue.
cd "$(dirname "$0")/.."
for p in showcase terrain far edits menger random room many-lights streaming; do
  echo "=== $p $(date +%H:%M)"
  ./target/release/multirenderer run plans/$p.toml -s output.dir=\"target/catalogue\" -s output.save.every=0 2>&1 | grep -E "FAILED|ERROR|uncleanly|panicked" | head -5
done
echo SUITE-DONE
