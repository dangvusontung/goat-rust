# TASK — BL5.3: Clutch index (accumulating stat)

Prereq: BL5.2 v1+v2 (`16fa538`, `d0fe445`) — decisive-moment detection, the
importance×result weighting, and the season/career counter plumbing this task
mirrors.

Read first: `crates/goat-match/src/sim.rs` (`is_decisive`, `MomentSummary`'s
`goals_*_before` fields), `crates/goat-core/src/state.rs`
(`pc_season_decisive_moments`, `decisive_effective_importance_x10`,
`Intent::ApplyRoundResult`/`ApplySeasonEndLegacy`), `crates/goat-save/src/save.rs`
(v15/v16 tail-append precedent).

## Origin

BL5.2's doc parked "clutch index" as an unresolved design question (new
accumulating stat vs per-match display value). **Tùng chốt 2026-08-03: it is a
NEW accumulating stat**, accruing season/career exactly like
`pc_decisive_moments` — not a display-only per-match value.

## Decision (settled)

1. **Definition** (chosen so the stat is not a duplicate of
   `pc_decisive_moments`): a clutch moment is the high-leverage SUBSET of
   decisive moments — moments that actually moved the needle:
   - PC's side scores (GoalFor/AssistFor) while level or trailing going in
     (`goals_for_before <= goals_against_before`): an equalizer or go-ahead
     goal. An insurance goal while already ahead stays `decisive` but is NOT
     clutch.
   - A threatened concession is prevented (same rule as `is_decisive`): any
     late stop in a one-goal game keeps points/hope alive — always clutch.
   Implement as `pub fn is_clutch(&MomentSummary) -> bool` in
   `goat-match::sim` = `is_decisive(m)` && the leverage check above.
2. **Fields**: `pc_season_clutch_index` / `pc_career_clutch_index: u32`,
   parallel to the decisive counters in every way: live season counter,
   `StartSeason` reset, `ApplySeasonEndLegacy` fold (new
   `season_clutch_index` param).
3. **Weighting**: reuse the v2 formula verbatim — same
   `decisive_effective_importance_x10` (incl. table tension), same result
   multiplier (win 10 / draw 5 / loss 0, ×10 ints), same half-up rounding.
   `Intent::ApplyRoundResult` gains `pc_clutch_count: u32`; the reducer
   computes one shared contribution closure for both counters. No new tuning
   constants.
4. **Legacy**: NOT wired into `LegacyEvidence` (Tùng didn't ask) — one-line
   future-hook comment, same idiom as BL5.1's assists note.
5. **Save**: VERSION 16→17, tail-append both u32s, `.unwrap_or(0)` reads,
   backward-compat + round-trip tests (v15/v16 idiom).
6. **Display** (playtest harness only): `career-sim --match-beats` FULL TIME
   line and `--season-beats` SEASON SUMMARY gain the clutch count, mirroring
   decisive. Batch career path passes 0 (no moments), same as decisive.

## Explicitly out of scope

- No Legacy/school weighting of the new stat (future design round).
- No changes to `is_decisive`, the weighting formula, or any frozen golden.
- The 10 pre-existing `smoke_stdin` baseline failures stay untouched.

## Definition of done

1. `cargo test --workspace` green modulo the known smoke_stdin baseline.
2. `cargo fmt --check` / `clippy -D warnings` clean.
3. New tests: `is_clutch` unit tests (equalizer/go-ahead count, insurance
   doesn't, defensive stop counts, non-decisive doesn't) + a reducer test
   pinning the shared weighting on the clutch counter + save v17 tests.
4. Playable gate: `career-sim --match-beats 8` shows `clutch 1` (the 84'
   close-game goal), `--season-beats` shows a nonzero Clutch total.
5. No new deps, no floats, no unsafe, no I/O in core.
6. Commit summary restating that Legacy weighting is deliberately unwired.
