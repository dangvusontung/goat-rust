#!/usr/bin/env bash
# scripts/world-sim.sh — whole-world simulation across a batch of seeds.
#
# Genesis → batch-tick the full outer world N seasons → top scorer, most-titled club,
# backfilled pantheon GOAT, and the emergent-rival verdict, for every seed in the batch.
#
# The emergent rival is measured against a reference PC career (the "keep pace" bar). A
# GOAT-tier PC makes the weak-era asterisk reachable when a generation's cohort is thin.
#
# Usage:
#   scripts/world-sim.sh                 # 20 seeds × 20 seasons, PC bar 300g/8t
#   scripts/world-sim.sh 30              # 30 seeds
#   scripts/world-sim.sh 20 25           # 20 seeds × 25 seasons
#   scripts/world-sim.sh 20 20 240 5     # lower the PC bar → more rivals
set -euo pipefail
cd "$(dirname "$0")/.."

BATCHES="${1:-20}"
SEASONS="${2:-20}"
PC_GOALS="${3:-300}"   # reference-PC career goals (rival "keep pace" bar)
PC_TITLES="${4:-8}"    # reference-PC league titles
BIN=target/debug/career-sim

echo ""
echo "╔══════════════════════════════════════════════════════════════════════════╗"
echo "║   BECOME THE GOAT — whole-world simulation (${BATCHES} seeds × ${SEASONS} seasons)"
echo "║   Rival measured vs a reference GOAT-tier PC: ${PC_GOALS} goals / ${PC_TITLES} titles"
echo "╚══════════════════════════════════════════════════════════════════════════╝"
echo ""

# Build once, quietly.
cargo build --quiet -p goat-tui --bin career-sim

# Field extractor: pulls key="value" or key=value from the WORLD output.
field() { sed -n "s/.*${2}=\"\\([^\"]*\\)\".*/\\1/p;s/.*${2}=\\([^ ]*\\).*/\\1/p" <<<"$1" | head -1; }

printf "%-4s | %-18s %-5s | %-14s %-4s | %-16s %-4s | %s\n" \
  "Seed" "Top Scorer" "G" "Champion Club" "Tt" "Pantheon GOAT" "BdO" "Rival"
echo "─────┼────────────────────────────┼─────────────────────┼───────────────────────┼──────────────"

weak=0; rival=0; goals_sum=0
for s in $(seq 0 $((BATCHES - 1))); do
  out=$("$BIN" --world-sim "$s" "$SEASONS" "$PC_GOALS" "$PC_TITLES" 2>&1)
  ts_line=$(grep 'topscorer=' <<<"$out");  ts=$(field "$ts_line" topscorer);  tg=$(field "$ts_line" goals)
  cl_line=$(grep 'mosttitles_club=' <<<"$out"); cl=$(field "$cl_line" mosttitles_club); ct=$(field "$cl_line" titles)
  pg_line=$(grep 'pantheon_goat=' <<<"$out"); pg=$(field "$pg_line" pantheon_goat); pb=$(field "$pg_line" ballondors)
  rv_line=$(grep 'rival=' <<<"$out")
  if grep -q 'rival=WEAKERA' <<<"$rv_line"; then
    rverdict="— reigns alone (asterisk)"; weak=$((weak + 1))
  else
    rn=$(field "$rv_line" name); rg=$(field "$rv_line" goals)
    rverdict="${rn} (${rg}g)"; rival=$((rival + 1))
  fi
  goals_sum=$((goals_sum + ${tg:-0}))
  printf "%-4s | %-18s %-5s | %-14s %-4s | %-16s %-4s | %s\n" \
    "$s" "$ts" "$tg" "$cl" "$ct" "$pg" "$pb" "$rverdict"
done

echo "─────┴────────────────────────────┴─────────────────────┴───────────────────────┴──────────────"
echo ""
printf "  Batch summary: %d seeds | rivals crystallised: %d | weak-era (reign alone): %d | avg top-scorer goals: %d\n" \
  "$BATCHES" "$rival" "$weak" "$((goals_sum / BATCHES))"
echo ""
