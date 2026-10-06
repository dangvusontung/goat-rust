#!/usr/bin/env bash
# Run on the target machine; cross compilation is not a native measurement.
set -euo pipefail
cd "$(dirname "$0")/.."
output="${1:-/tmp/goat-capacity-native}"
mkdir -p "$output"
{
  uname -a
  rustc --version
  cargo --version
  if command -v lscpu >/dev/null; then lscpu; fi
} > "$output/environment.txt"
cargo build --locked --release -p goat-save --features capacity-bench --example bench_capacity
binary=target/release/examples/bench_capacity
for deep in 1 3 6; do
  python3 scripts/measure-native.py "$output/200k-deep-$deep-weeks.txt" "$binary" --players 200000 --deep "$deep" --weeks 16 --dir "$output"
  file="$output/players-200000-deep-$deep-year-1.capbench"
  for mode in checkpoint cold; do
    python3 scripts/measure-native.py "$output/200k-deep-$deep-$mode.txt" "$binary" --load "$file" --mode "$mode"
  done
done
