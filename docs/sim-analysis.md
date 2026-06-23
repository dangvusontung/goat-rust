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
