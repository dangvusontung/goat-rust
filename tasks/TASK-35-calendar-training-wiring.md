# TASK 3.5 — Wire CalendarEngine + training into the live loop

Prereq: Phases 1–8 playable; lifestyle coupling (TASK-10A) landed. Read CLAUDE.md,
`docs/CALENDAR.md` (authoritative for the day-tick loop + subsystem ABI), and the
ROADMAP "Phase 3.5" section. The frozen golden rule is load-bearing here: **the live
growth/injury/decay math must not change**, so existing `golden_week` values stay
frozen.

## Architecture (decided)

- **Merge training into `goat-core`.** Delete the `goat-training` crate; the live loop
  has ONE training implementation. `goat-core` gains a dependency on `goat-calendar`
  (calendar deps only rng/fixed, so `goat-core → goat-calendar` is acyclic). This
  dissolves the dependency cycle by removing the split (roadmap blocker #1).
- **Batch 7 day-ticks per week.** `advance_week` keeps its public API and signature;
  internally it drives a `CalendarEngine` for 7 `tick_one_day()` calls. The golden
  anchor is exact equivalence: 7-day-batched `advance_week` == current `advance_week`,
  byte-for-byte (blocker #3).
- **Single ownership.** The in-core `TrainingSubsystem` operates on borrowed
  `WorldState`/`PlayerStore` data, NOT an owned `PlayerView` copy (blocker #2).

### Golden-safety design (the crux)

`goat-core::week.rs` already owns the canonical growth/injury/decay model with frozen
goldens. `goat-training` shipped a *separate* day-based model (`compute_growth`,
`trainability`). **The canonical model stays `week.rs`** — its goldens are sacred.
Therefore:

- The day-tick loop is for **calendar progression** (flashpoints, windows, fixtures).
  **Training growth still accrues once per week**, applied on the week-boundary tick
  using the EXISTING `week.rs` logic. 7 day-ticks → one week's growth, identical output.
- `goat-training`'s parallel growth model is **redundant**; absorb only what's uniquely
  useful (e.g. `TrainingEvent` taxonomy) and delete the rest. Do not adopt its day-based
  growth in the live path — that would move golden values (forbidden).

> If keeping `week.rs` as canonical conflicts with anything in CALENDAR.md, the doc
> wins — flag it, don't silently pick.

## TDD anchor (write first, must be RED then GREEN)

`crates/goat-core/tests/spec_phase35_calendar.rs`:
- `golden_week_equivalence_under_day_batch` — for a fixed seed + routine, the new
  day-batched `advance_week` reproduces the EXACT attribute/energy/injury state of the
  pre-refactor `advance_week` (assert against the same constants `golden_week` freezes).
- `seven_day_ticks_equal_one_week` — `CalendarEngine` advanced 7 days == one
  `advance_week`, same RNG sequence, same store.
- Property: a full headless season via the calendar loop keeps all invariants
  (`current ≤ potential`, attrs 1–99, no panic).

## Steps (golden-safe ordering)

### Step 1 — Add the seam without behavior change
- Add `goat-calendar` to `goat-core`'s `Cargo.toml`. Confirm workspace builds; all
  goldens green (nothing wired yet).

### Step 2 — In-core TrainingSubsystem over borrowed state
- Create `goat-core::calendar_loop` (or `training` module). Define a `TrainingSubsystem`
  implementing `goat_calendar::Subsystem`, driving the EXISTING `week.rs` growth at the
  week boundary, reading/writing the live `PlayerStore` (no owned copy).
- Unit test it in isolation before wiring.

### Step 3 — Route advance_week through the engine
- Rewrite `advance_week` body to run a `CalendarEngine` for 7 `tick_one_day()`s, with
  the `TrainingSubsystem` registered. Growth/injury/decay land exactly as today.
- Turn the equivalence spec GREEN. `golden_week` values UNCHANGED.

### Step 4 — Delete goat-training
- Remove the crate from the workspace + members. Migrate any kept types into `goat-core`.
- Update `docs`/module map references if needed (flag, don't silently diverge).

### Step 5 — Wire one calendar capability through to the TUI
- Surface `advance_until_flashpoint` (or day-windows) in the week loop so the calendar
  earns its keep: the gate below.

## ⏸ PAUSE — before Step 3
Show the user the equivalence test going green and confirm `golden_week` is untouched
before deleting `goat-training` (Step 4 is the irreversible one).

## Playable gate
`cargo run -p goat-tui`: `[W] Train` advances through the calendar loop (7 day-ticks),
and a calendar flashpoint (e.g. transfer window / fixture congestion) can interrupt the
week — proving the CalendarEngine is live, not bypassed.

## Definition of done
1. `cargo test --workspace` green incl. ALL pre-existing `golden_week` values unchanged.
2. Equivalence + season-invariant specs green.
3. `goat-training` crate deleted; no dangling deps; one training implementation.
4. `cargo fmt --check` + `clippy -D warnings` clean on touched crates.
5. No floats in sim, no unsafe, no I/O in core, no logic in TUI; determinism preserved.
6. Summary: what changed + which CALENDAR.md sections it implements.

## Out of scope
Day-tick rewrite of the core loop (we batch, not replace), match/transfer/media
subsystems beyond what training needs, season-boundary pipeline depth (own task).
