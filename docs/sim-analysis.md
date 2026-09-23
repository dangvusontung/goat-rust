# Simulation Analysis — Weird Numbers

Batch of 20 seeds (0–19), 20 seasons each, Forward position.
Run: `cargo run -p goat-tui --bin career-sim -- --seed N`

---

## Issues Found

### 1. OVR flatlines for 10+ years
Seeds 0, 1, 3, 4, 5, 7 hit their OVR ceiling at age 20–22 and stay completely flat until decay at ~31.
- Seed 0: OVR=56 from age 20 to age 31, zero change for 11 years.
- Once Fin/Dri/Pas hit their ceilings, nothing left to train. OVR never improves mid-career.
- Real players add tactical awareness, leadership, role mastery mid-career.

### 2. Form collapses and never recovers
- Seed 0: form drops to 12–21 from age 29 onwards and stays there.
- Form EMA is self-referential (`output = form ± variance`) — once it drifts low it feeds itself.
- A veteran playing 30 games/season with form=12 is unrealistic.

### 3. Seasonal goal variance is extreme
- Seed 4 (Fin 96): 20 goals at age 31 ★, 18 at age 34 ▼, but only 4 at age 19 ◆.
- Peak Fin=96 should mean consistently high output, not 20 one year and 4 the next.
- Root cause: per-team-goal roll creates high noise over 30 games.

### 4. OVR ceiling too low — best is 68 across 20 seeds
- "Become the GOAT" probably needs OVR 80+ to be achievable.
- Weighted-average role rating dragged down hard by any zero-weight or baseline attrs.
- Role weights may over-penalise mismatched attrs.

### 5. Age 23 always skipped in the season log
- 60 training weeks/season = 1.154 years. After season 6 player is 22.9, after season 7 they're 24.1.
- Age 23 never appears at a snapshot boundary — every player jumps 22 → 24.
- Cosmetic display oddity; easy to fix by tracking age at season start instead of end.

### 6. Titles vs player quality are completely decoupled
- Seed 15 (OVR 40, worst player in batch): **7 titles** — most of any seed.
- Seed 3 (OVR 61, top 3 in batch): only 2 titles.
- Team strength is the only driver of league position. Player quality has zero influence on team results.

### 7. Pac hits ceiling in season 1 and freezes until decay
- Physical attrs start at 85% of potential and fill the remaining 15% in the first ~60 training weeks.
- Every player's Pac is at ceiling by the first season snapshot (age 17).
- Stays frozen for 10–15 seasons, then decays. No training arc for Pac at all.

### 8. OVR vs Fin disconnect is jarring
- Seed 11 (Fin 97, highest possible): OVR only 57.
- Seed 4 (Fin 96, Pas 93): OVR only 48 — the best finisher in the batch is nearly invisible in OVR.
- Role rating weights over-penalise low attrs like Dri even when role is a poacher archetype.

---

## Priority

| # | Issue | Severity |
|---|-------|----------|
| 1 | OVR plateau — no mid-career growth | High |
| 6 | Player quality has no impact on team results | High |
| 4 | OVR ceiling too low to feel legendary | High |
| 3 | Seasonal goal variance too extreme | Medium |
| 2 | Form collapse self-reinforcing spiral | Medium |
| 8 | OVR/Fin disconnect — role weights too punishing | Medium |
| 7 | Pac freezes at ceiling from age 17 | Low |
| 5 | Age 23 skipped in season log | Low |

---

## Batch Summary (seeds 0–19, post-fix)

```
Seed │ PeakOVR │ PeakAge │ Apps │ Goals │ Titles │ Ceilings Reached
─────┼─────────┼─────────┼──────┼───────┼────────┼──────────────────────────
  0  │   56    │   28    │  598 │  165  │   2    │ Fin 84  Dri 41  Pas 50
  1  │   59    │   28    │  599 │  147  │   6    │ Fin 73  Dri 48  Pas 43
  2  │   54    │   29    │  540 │   99  │   2    │ Fin 57  Dri 87  Pas 70
  3  │   61    │   27    │  600 │  171  │   2    │ Fin 87  Dri 93  Pas 75
  4  │   48    │   29    │  510 │  167  │   3    │ Fin 96  Dri 54  Pas 93
  5  │   57    │   29    │  570 │  159  │   3    │ Fin 80  Dri 46  Pas 50
  6  │   46    │   27    │  510 │   73  │   4    │ Fin 47  Dri 60  Pas 91
  7  │   56    │   28    │  600 │  158  │   4    │ Fin 80  Dri 48  Pas 55
  8  │   54    │   28    │  570 │  147  │   3    │ Fin 79  Dri 47  Pas 60
  9  │   46    │   31    │  510 │   92  │   5    │ Fin 49  Dri 49  Pas 97
 10  │   60    │   31    │  600 │  167  │   2    │ Fin 88  Dri 95  Pas 91
 11  │   57    │   29    │  600 │  190  │   2    │ Fin 97  Dri 43  Pas 54
 12  │   68    │   29    │  598 │  156  │   2    │ Fin 80  Dri 97  Pas 85
 13  │   59    │   28    │  597 │  170  │   3    │ Fin 86  Dri 89  Pas 60
 14  │   58    │   29    │  600 │  160  │   4    │ Fin 79  Dri 47  Pas 48
 15  │   40    │   28    │  510 │   71  │   7    │ Fin 46  Dri 45  Pas 59
 16  │   64    │   29    │  600 │  137  │   3    │ Fin 73  Dri 95  Pas 78
 17  │   65    │   29    │  600 │  188  │   4    │ Fin 93  Dri 84  Pas 77
 18  │   67    │   28    │  600 │  176  │   5    │ Fin 91  Dri 93  Pas 89
 19  │   66    │   28    │  600 │  165  │   4    │ Fin 86  Dri 94  Pas 68
```

---

## Changes Already Applied

- `NONE_POT_ABS_LOW` raised 20 → 40 (`crates/goat-core/src/tuning.rs`)
  - Zero-weight attrs (e.g. Strength for a flair forward) now floor at 40 instead of 20.
  - Golden week tests re-frozen with new expected values.
- Goal attribution formula (`crates/goat-tui/src/career_sim.rs`)
  - Was: `pc_gf / 4` (flat quarter of team goals, ignores Finishing).
  - Now: per-team-goal Finishing roll — `fin/400` chance per goal. Fin 90 ≈ 22% per team goal.
  - Career totals went from ~9 to ~150–190 goals over 20 seasons.

---

# Match Flow — Realism Tuning Log

Harnesses:
- **match-batch** (`cargo run --release -p goat-tui --bin match-batch -- 100000`): 100k matches,
  random fresh-gen PCs across 5 roles, own str 75 vs opp 60–95. From round 3 on, 1-in-4 matches is
  played by a "star injection" PC (attr floor 75, role rating 65–79) so star-player effects are
  visible — fresh-gen players cap at role rating ~60 and used to wash them out of the averages.
- **match-sim** (`career-sim --match-sim N 7`): fixed star striker (OVR ~64), own str 70,
  opp 45–90 — the actual career-game scenario. `starred_in_defeat = output ≥ 70 & L`.

Targets: W/D/L plausible, goals/match ~2.7, starred-in-defeat 2–4%, rating tail thin at 90–100.

## Round 2 baseline (commit 177dc98, pre-star-injection harness)

100k, master seed 0xBA7C4 — W 36.8 / D 23.8 / L 39.5 · goals 2.91 (1.45–1.46) · clean sheets 15.8% ·
PC 2+ goals & L: 2.10% · rating 90–100 bucket: 2.1% · mean rating 56.8.

But the two open issues hid in the population average:
- match-sim star striker (N=2000): starred-in-defeat 1.85% (seed 7), 1.30% (seed 11), 1.65% (seed 23)
  — the "~1%" that motivated round 3. 80+ output bucket 11–13%.
- Star PCs pile at rating exactly 100 (soft-cap asymptote was at 200, +1 minimum delta per success).

## Round 3 baseline (star-injection harness, same engine)

100k, master seed 0xBA7C4 — W 39.3 / D 23.3 / L 37.4 · goals 2.98 (1.52–1.46) · clean sheets 15.9% ·
PC 2+ goals & L: 2.38% · rating≥70 & L: 3.02% (8.1% of losses) · mean rating 59.1.
Star band (role rating ≥ 65, n=25k): 90–100 bucket 8.8% — of which rating **exactly 100: 1.96%**
(a clamp pile: 22% of the top bucket sits on the rail) · starred-in-defeat 4.94% (15.9% of losses).

## Round 3a — response surge + star funnel + weaker momentum→contest coupling

Changes (`crates/goat-match/src/sim.rs`):
- **Response surge**: for 2 ticks after a goal, the conceding side gets +8 pp possession and
  ×1.40 goal chance (`RESPONSE_TICKS/SHARE_PCT/GOAL_BOOST`) — real matches cluster goals right
  after a goal, and a PC goal inviting a reply decouples his output from the result.
- **Momentum→contest coupling halved** (`MOMENTUM_CONTEST_DIV` 10 → 20): match momentum is a team
  state; letting it dominate the PC's contest roll coupled his rating to the scoreline.
- **Star funnel when trailing**: +20 pp involvement, +15 pp zone pull when behind
  (`TRAIL_INVOLVE_BONUS`, `TRAIL_ZONE_PULL_BONUS`) — chasing teams play through their star.
- `AUTO_GOAL_SCALE` 36 → 33 to pay for the surge/funnel goal inflation (2.98 → 3.14 → 3.01).

Golden seed 42 unchanged (output 54, 2-2) — all new mechanics are score-state-conditional
and consume no extra RNG on the golden path.

| Metric (match-batch 100k, seed 0xBA7C4) | Before | After |
|---|---|---|
| W / D / L % | 39.3 / 23.3 / 37.4 | 39.7 / 25.4 / 34.9 |
| Goals/match (for–against) | 2.98 (1.52–1.46) | 3.01 (1.55–1.46) |
| Clean sheets | 15.9% | 17.1% |
| PC 2+ goals & L | 2.38% | **3.24%** |
| Starred-in-defeat (rating≥70 & L) | 3.02% | **3.72%** |
| Mean rating | 59.1 | 60.2 |
| Star band 90–100 / pile at 100 | 8.8% / 1.96% | 9.2% / 2.11% |

match-sim star striker (N=5000, seeds 7/11/23): starred-in-defeat **1.64/1.42/1.84% →
2.00/1.90/2.26%** — now inside the 2–4% target on both harnesses.

## Round 3b — rating taper with the asymptote at the rails

Change (`crates/goat-match/src/sim.rs`, `apply_output_delta`): positive deltas now scale by
`(100 − output) × 7/400` (0.875 at output 50, 0 at 100) and negative deltas by
`(output + 50) × 3/400`; the ±1 minimum delta only applies in the 10–90 mid band. The old curve
(asymptote 200 + unconditional ±1 floor) let strong PCs grind onto the 100 clamp.

Golden seed 42: scoreline/moments/cards identical (2-2, 19 moments), only output moved 54 → 52 —
the flow is untouched, only rating arithmetic. Golden re-frozen for the output value alone.

| Metric (match-batch 100k, seed 0xBA7C4) | After 3a | After 3b |
|---|---|---|
| W / D / L % | 39.7 / 25.4 / 34.9 | 39.7 / 25.4 / 34.9 (flow untouched) |
| Goals/match | 3.01 | 3.01 |
| Clean sheets | 17.1% | 17.1% |
| PC 2+ goals & L | 3.24% | 3.24% |
| Starred-in-defeat (rating≥70 & L) | 3.72% | 4.37% (scale shift: ≥70 is reached more often) |
| Rating 90–100 bucket (all / star band) | 4.0% / 9.2% | **0.9% / 2.3%** |
| Rating exactly 100 (star band) | 2.11% | **0.00%** |
| Mean rating | 60.2 | 61.2 |

match-sim star striker: output max 100 → 93–97, 80+ bucket 11–13% → 9.7–11.2%,
starred-in-defeat **2.0–2.3% → 2.4/2.2/2.5%** — mid-band on the fixed scale.
Star band (role rating ≥ 65, n=25k): 90–100 bucket 8.8% — of which rating **exactly 100: 1.96%**
(a clamp pile: 22% of the top bucket sits on the rail) · starred-in-defeat 4.94% (15.9% of losses).

## PA2 M1+M1.5 — engine no-op verification (roster profiles + manager/selection)

M1 (commit `b9361ed`) and M1.5 (`f1ef256`) changed WHERE the live game gets its
`TacticalProfile`s (real squads, formation, manager trust/favor, PC benching) — all in
goat-core/goat-world/goat-tui's live loop. The engine (`goat-match`) was not touched, and
the harnesses build their own controlled profiles (`TacticalProfile::derive(75, 1000,
seed)`), so this round must be a bit-for-bit no-op — and is:

| Metric (match-batch 100k, seed 0xBA7C4) | After 3b | After M1+M1.5 |
|---|---|---|
| W / D / L % | 39.7 / 25.4 / 34.9 | 39.7 / 25.4 / 34.9 (identical) |
| Goals/match (for–against) | 3.01 (1.55–1.46) | 3.01 (1.55–1.46) |
| Clean sheets | 17.1% | 17.1% |
| PC 2+ goals & L | 3.24% | 3.24% |
| Starred-in-defeat (≥70 & L / ≥80 & L) | 4.37% / 0.68% | 4.37% / 0.68% |
| Carried-to-win (≤45 & W) | 1.97% | 1.97% |
| Rating 90–100 / pile at 100 (star band) | 2.3% / 0.00% | 2.3% / 0.00% |
| Mean rating | 61.2 | 61.2 |

match-sim star striker (N=5000, seeds 7/11/23): starred-in-defeat **2.40/2.18/2.52%** —
identical to 3b (2.4/2.2/2.5).

**Caveat (measurement gap):** match-batch/career-sim cannot see M1/M1.5's actual gameplay
effect — which profiles the live loop feeds the engine, how often the PC is benched, or how
trust drifts. Those live in `run_next_round` (goat-tui), which no batch harness exercises.
Smoke-run observations (TUI, seed 7): fresh 64-OVR 16yo benched 8/8 at a 5★ club under a
Strict manager; starts at a Third Division club until skipped training erodes trust. A
live-loop season harness (drive `run_next_round` headless, tally W/D/L + bench rate + trust
trajectory) is the missing measurement tool — candidate follow-up before M2.

## PA2 M2 — individuals in the contest + real-name commentary (engine change)

M2 brought the 22 real individuals into the match (commit after this entry):
`SquadSheet` (names + full attrs) on `MatchSetup`, a specific matchup opponent per
PC beat whose real counter-attrs blend 50/50 with the team line into difficulty
(MATCH.md A.5), and `{scorer}/{opponent}/{assist}` template slots in beats.json
filled from the sheets. Harnesses use `SquadSheet::stub` centred on their controlled
strengths, so this batch measures the A.5 difficulty change (not the live loop's
real squads, which no batch harness exercises — same caveat as M1/M1.5).

**Golden seed 42 re-frozen (52/2-2 → 57/1-1, 25 moments):** intentional — matchup
draws + name picks consume match RNG and the 50/50 blend shifts contest math.
Flow rules (possession/zone/momentum/auto-goal/mercy/response) untouched. The
12 non-value golden behaviour tests (chains cap, role zones, stronger-team,
headspace bounds, ref/aggression…) all pass unchanged.

| Metric (match-batch 100k, seed 0xBA7C4) | Pre-M2 (post-M1.5) | M2 |
|---|---|---|
| W / D / L % | 39.7 / 25.4 / 34.9 | 39.5 / 25.4 / 35.1 |
| Goals/match (for–against) | 3.01 (1.55–1.46) | 3.02 (1.55–1.47) |
| Clean sheets | 17.1% | 16.6% |
| PC 2+ goals & L | 3.24% | 3.39% |
| Starred-in-defeat (≥70 & L / ≥80 & L) | 4.37% / 0.68% | 4.68% / 0.76% |
| Carried-to-win (≤45 & W) | 1.97% | 1.95% |
| Rating 90–100 / pile at 100 (star band) | 2.3% / 0.00% | 2.2% / 0.00% |
| Mean rating | 61.2 | 61.3 |

match-sim star striker (N=5000, seeds 7/11/23): starred-in-defeat **2.04/2.40/2.72%**
(was 2.40/2.18/2.52) — still mid-band on the 2–4% target.

Read: the 50/50 individual blend adds per-beat difficulty variance (starred-in-defeat
+0.3pp, 2+-goals-and-lost +0.15pp — slightly more decoupling, in the right direction)
without touching team-level balance. No new rail pile-ups; per-position splits stable.

Live-game smoke (TUI, real population squads): commentary names real players both
directions — "Isolated out wide: just you and Viktor Adeyemi.", "You receive with
your back to goal, Viktor Adeyemi tight on you.", and in career-sim's stub-sheet
feed "A lightning break — L. Ferreira applies the finishing touch. GOAL!",
"J. Hartley beats you clean — and it ends up in your net."
