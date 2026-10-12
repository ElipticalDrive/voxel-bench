#!/bin/bash
# Saves the current release build as the "before" side of A/B checks (run before a change).
cd "$(dirname "$0")/../.."
cargo build --release -q || exit 1
mkdir -p target/ab-before && cp target/release/multirenderer target/ab-before/multirenderer && echo "saved target/ab-before/multirenderer"
