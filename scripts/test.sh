#!/usr/bin/env bash
# scripts/test.sh — full GOAT workspace test runner
#
# Runs: fmt → clippy → cargo test → career-sim sanity run.
# Exit code 0 only when every step passes.

set -euo pipefail
cd "$(dirname "$0")/.."

PASS="✓"
FAIL="✗"
RESULTS=()

run_step() {
    local label="$1"; shift
    printf "  %-40s" "$label ..."
    if output=$("$@" 2>&1); then
        printf "%s\n" "$PASS"
        RESULTS+=("PASS: $label")
    else
        printf "%s\n" "$FAIL"
        echo "$output" | sed 's/^/    /'
        RESULTS+=("FAIL: $label")
        return 1
    fi
}

echo ""
echo "╔══════════════════════════════════════════════════════╗"
echo "║       BECOME THE GOAT — workspace test suite         ║"
echo "╚══════════════════════════════════════════════════════╝"
echo ""

FAILED=0

# ── Step 1: Format check ────────────────────────────────────────────────────────
run_step "cargo fmt --check" cargo fmt --check || FAILED=1

# ── Step 2: Clippy (deny all warnings) ─────────────────────────────────────────
run_step "cargo clippy -D warnings" \
    cargo clippy --workspace --all-targets -- -D warnings || FAILED=1

# ── Step 3: All workspace tests ─────────────────────────────────────────────────
printf "  %-40s\n" "cargo test --workspace ..."
if output=$(cargo test --workspace 2>&1); then
    # Print test summary line (the "test result:" line)
    echo "$output" | grep -E "^(test result:|running [0-9]+)" | sed 's/^/    /' || true
    printf "  %-40s%s\n" "" "$PASS"
    RESULTS+=("PASS: cargo test --workspace")
else
    echo "$output" | tail -30 | sed 's/^/    /'
    printf "  %-40s%s\n" "" "$FAIL"
    RESULTS+=("FAIL: cargo test --workspace")
    FAILED=1
fi

# ── Step 4: Build career-sim ────────────────────────────────────────────────────
run_step "cargo build -p goat-tui --bin career-sim" \
    cargo build -p goat-tui --bin career-sim 2>&1 || FAILED=1

# ── Step 5: career-sim seed-42 sanity run ──────────────────────────────────────
printf "  %-40s" "career-sim --seed 42 (invariants) ..."
if sim_out=$(cargo run --quiet -p goat-tui --bin career-sim -- --seed 42 2>&1); then
    if echo "$sim_out" | grep -q "VIOLATION DETECTED"; then
        printf "%s\n" "$FAIL"
        echo "  Invariant violation found in career-sim output:"
        echo "$sim_out" | grep -A2 "VIOLATION" | sed 's/^/    /'
        RESULTS+=("FAIL: career-sim invariants")
        FAILED=1
    else
        # Print the career verdict line
        echo "$sim_out" | grep -E "(Seasons played|Career apps|League titles|Peak OVR|Invariants OK)" \
            | sed 's/^/    /' || true
        printf "  %-40s%s\n" "" "$PASS"
        RESULTS+=("PASS: career-sim invariants")
    fi
else
    printf "%s\n" "$FAIL"
    echo "$sim_out" | tail -10 | sed 's/^/    /'
    RESULTS+=("FAIL: career-sim panicked or crashed")
    FAILED=1
fi

# ── Step 6: career-sim --scan (seed scanner) ───────────────────────────────────
run_step "career-sim --scan (seed scanner)" \
    cargo run --quiet -p goat-tui --bin career-sim -- --scan || FAILED=1

# ── Summary ─────────────────────────────────────────────────────────────────────
echo ""
echo "╔══════════════════════════════════════════════════════╗"
echo "║  SUMMARY                                             ║"
echo "╠══════════════════════════════════════════════════════╣"
for r in "${RESULTS[@]}"; do
    printf "║  %-52s║\n" "$r"
done
echo "╚══════════════════════════════════════════════════════╝"

if [ "$FAILED" -eq 0 ]; then
    echo ""
    echo "  ALL STEPS PASSED"
    echo ""
    exit 0
else
    echo ""
    echo "  ONE OR MORE STEPS FAILED — see output above"
    echo ""
    exit 1
fi
