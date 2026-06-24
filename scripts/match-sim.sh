#!/usr/bin/env bash
# scripts/match-sim.sh — beat-engine match simulation across N matches.
#
# Runs the goat-match beat engine for N auto-played matches against opponents spanning
# the strength spectrum, and analyses the distribution of the player's OUTPUT rating
# vs the TEAM RESULT — the Phase-4 design point that the two are decoupled
# ("a hat-trick in a 3-2 defeat" must be possible).
#
# Usage:
#   scripts/match-sim.sh             # 100 matches, default star striker (seed 7)
#   scripts/match-sim.sh 250         # 250 matches
#   scripts/match-sim.sh 100 11      # 100 matches with player seed 11
set -euo pipefail
cd "$(dirname "$0")/.."

N="${1:-100}"
SEED="${2:-7}"
BIN=target/debug/career-sim

cargo build --quiet -p goat-tui --bin career-sim

echo ""
echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║   BECOME THE GOAT — beat-engine match simulation (${N} matches)"
echo "╚══════════════════════════════════════════════════════════════════╝"
echo ""

out=$("$BIN" --match-sim "$N" "$SEED" 2>&1)

field() { sed -n "s/.*$1=\\([^ ]*\\).*/\\1/p" <<<"$2" | head -1; }

ml=$(grep '^MATCHSIM'  <<<"$out")
ol=$(grep '^OUTPUT '   <<<"$out")
rl=$(grep '^RESULTS'   <<<"$out")
gl=$(grep '^TEAMGOALS' <<<"$out")
bl=$(grep '^OUTPUT_BY_RESULT' <<<"$out")
dl=$(grep '^DECOUPLING' <<<"$out")

ovr=$(field player_ovr "$ml")
o_min=$(field min "$ol"); o_max=$(field max "$ol"); o_avg=$(field avg "$ol")
b1=$(field '<40' "$ol"); b2=$(field '40-59' "$ol"); b3=$(field '60-79' "$ol"); b4=$(field '80+' "$ol")
W=$(field W "$rl"); D=$(field D "$rl"); L=$(field L "$rl")
gf=$(field avg_for "$gl"); ga=$(field avg_against "$gl")
yel=$(field yellow "$gl"); red=$(field red "$gl")
ow=$(field W "$bl"); od=$(field D "$bl"); ol2=$(field L "$bl")
sid=$(field 'starred_in_defeat(out>=70 & L)' "$dl")
ctw=$(field 'carried_to_win(out<=45 & W)' "$dl")

printf "  Player: star striker (OVR %s)   |   own strength 70   |   opponents 45-90\n\n" "$ovr"
printf "  OUTPUT RATING   min %-3s  avg %-3s  max %-3s\n" "$o_min" "$o_avg" "$o_max"
printf "    distribution  <40:%-3s  40-59:%-3s  60-79:%-3s  80+:%-3s\n\n" "$b1" "$b2" "$b3" "$b4"
printf "  TEAM RESULTS    W %-3s  D %-3s  L %-3s   (avg goals %s-%s)   cards: %s yel, %s red\n\n" \
  "$W" "$D" "$L" "$gf" "$ga" "$yel" "$red"
printf "  AVG OUTPUT      in wins %-3s   in draws %-3s   in losses %-3s\n\n" "$ow" "$od" "$ol2"
echo   "  ── DECOUPLING (output rating ≠ team result) ──────────────────────"
printf "    starred in defeat (output ≥70 yet LOST) : %s / %s\n" "$sid" "$N"
printf "    carried to a win  (output ≤45 yet WON)  : %s / %s\n" "$ctw" "$N"
echo ""
echo "  Output tracks the player's own game; the scoreline tracks the team."
echo "  A high rating in a defeat is exactly the Phase-4 'hat-trick, lost 3-2'."
echo ""
