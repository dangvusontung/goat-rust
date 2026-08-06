#!/usr/bin/env bash
# Overnight core-logic fuzz: runs career-sim across a wide range of seeds and
# positions, checking for panics and invariant violations (cur <= pot, cur >= min)
# per CLAUDE.md's "Long-horizon sanity" test-discipline rule. Read-only against
# the repo; writes only to /tmp.
set -uo pipefail

BIN="/home/tungdvs/projects/goat-rust/target/release/career-sim"
SEEDS_FROM="${1:-1}"
SEEDS_TO="${2:-3000}"
OUT_DIR="/tmp/goat_fuzz_$(date +%Y%m%d_%H%M%S 2>/dev/null || echo run)"
mkdir -p "$OUT_DIR"
SUMMARY="$OUT_DIR/summary.txt"
FAILURES="$OUT_DIR/failures.log"

total=0
fail=0
no_invariant_line=0

for pos in forward midfielder defender; do
  for seed in $(seq "$SEEDS_FROM" "$SEEDS_TO"); do
    total=$((total + 1))
    out=$("$BIN" --seed "$seed" --position "$pos" 2>&1)
    code=$?
    if [ "$code" -ne 0 ]; then
      fail=$((fail + 1))
      {
        echo "=== FAIL (exit $code) seed=$seed position=$pos ==="
        echo "$out"
        echo
      } >> "$FAILURES"
      continue
    fi
    if ! echo "$out" | grep -q "Invariants OK  : YES"; then
      if echo "$out" | grep -q "Invariants OK"; then
        fail=$((fail + 1))
        {
          echo "=== INVARIANT VIOLATION seed=$seed position=$pos ==="
          echo "$out" | grep -A2 "Invariants OK"
          echo
        } >> "$FAILURES"
      else
        no_invariant_line=$((no_invariant_line + 1))
        {
          echo "=== NO INVARIANT LINE FOUND seed=$seed position=$pos (unexpected output shape) ==="
          echo "$out" | tail -20
          echo
        } >> "$FAILURES"
      fi
    fi
  done
done

{
  echo "Goat-rust overnight invariant fuzz — $(date 2>/dev/null || echo 'no-date')"
  echo "Seed range: $SEEDS_FROM..$SEEDS_TO x 3 positions"
  echo "Total runs: $total"
  echo "Failures (panic/exit!=0): see failures.log"
  echo "Runs with no invariant line found: $no_invariant_line"
  echo "Total flagged: $fail"
  if [ "$fail" -eq 0 ] && [ "$no_invariant_line" -eq 0 ]; then
    echo "RESULT: ALL CLEAN"
  else
    echo "RESULT: ISSUES FOUND — see $FAILURES"
  fi
} > "$SUMMARY"

cat "$SUMMARY"
