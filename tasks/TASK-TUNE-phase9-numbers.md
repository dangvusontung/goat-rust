# TASK TUNE — Balance the Phase-9 placeholder numbers (deferred)

Prereq: Phase 9 done. Read CLAUDE.md (tuning lives in `tuning` modules / named constants;
bible numbers are placeholders illustrating shape). Every Phase-9 number shipped as a
first-guess pending your approval — this task tunes them against the headless harnesses,
then **re-freezes the goldens with explicit sign-off**.

## Tools (already built, headless)
- `scripts/world-sim.sh [seeds] [seasons] [pc_goals] [pc_titles]` — whole-world batch:
  top scorers, champion distribution, pantheon GOAT, rival/weak-era rate.
- `career-sim --genesis|--history|--rival|--world-sim` — single-seed probes.
- `career-sim --seed N [--position …] [--lifestyle …] [--intensity …]` — the PC career.

## The numbers (all named constants — no magic inline)
- `goat-world::population`: `SQUAD_SIZE`, genesis potential band (`strength ± 15`, clamp
  30–99), `development_fraction` curve, `RETIRE_AGE_YEARS`.
- `goat-world::batch_tick`: `SEASON_APPS_STARTER/FRINGE`, `STARTERS_PER_CLUB`,
  `goal_weight_x10` (FW/MF/DF).
- `goat-world::history`: `NUM_GREATS`, career-length band, `peak_ovr` band, champion noise.
- `goat-world::rival`: `TITLE_WEIGHT`, `KEEP_PACE_PCT`, `MIN_RIVAL_APPS`,
  `COHORT_HALF_WIDTH_WEEKS`.

## Targets to dial in (decide the intended feel, then tune to it)
- **Top-scorer output**: a 20-season run currently averages ~257 goals (≈13/season).
  Is that the GOAT-tier you want, or too inflated? Set a target band.
- **Title competitiveness**: champion clubs recur (Man City / Flamengo dynasties). Decide
  how dynastic vs. open the leagues should be (champion noise vs. strength weighting).
- **Rival rate**: at PC 300g/8t the weak-era rate is ~60%. Pick the intended frequency of
  "you reign alone" vs. "a rival emerges" for a *typical* great career.
- **Development curve**: confirm the age peak (25–31) and decline feel right vs. the bible.

## Process
1. Sweep with `world-sim` across ≥20 seeds; record distributions (goals, titles, rival rate).
2. Adjust one constant family at a time; re-sweep; compare.
3. When a number is settled, **re-freeze** the affected golden(s) and get user approval:
   - genesis fingerprint, batch-tick career fingerprint, history fingerprint, rival mask.
   - Note: these WILL move when the numbers change — that's expected for this task only.

## ⏸ Pause
Before re-freezing ANY golden, show the user the before/after distributions and the new
frozen values. Goldens freeze only on explicit approval (CLAUDE.md).

## Definition of done
1. Each tuned constant has a one-line rationale in its `tuning`/module doc.
2. Distributions match the agreed targets (state measured numbers).
3. All Phase-9 goldens re-frozen to the new values, user-approved.
4. `cargo test --workspace` green; `fmt` + `clippy -D warnings` clean.

## Out of scope
New systems (this is tuning only), Phase-1–8 goldens (frozen — must NOT move), Phase 10.
