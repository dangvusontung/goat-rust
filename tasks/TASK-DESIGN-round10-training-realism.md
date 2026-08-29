# TASK — Design round 10: training realism (position-specific + periodization)

Prereq: none blocking. **One open decision (D2) should be settled before Step 2** — where
season-phase derivation lives. Step 1 can start immediately.

Read first: `crates/goat-core/src/week.rs` (`advance_week`, `attr_growth_rate`,
`injury_prob`), `crates/goat-core/src/state.rs` (`tick_one_week` ~line 1370 — where
`effective_mult` is assembled), `crates/goat-core/src/roles.rs` (`ROLE_WEIGHT_TABLE`),
`crates/goat-core/src/tuning.rs` (`W_KEY`/`W_IMP`/`W_BAS`/`W_ZERO`),
`crates/goat-core/tests/golden_week.rs` (the frozen values this task threatens),
`crates/goat-world/src/calendar.rs` (`PRE_SEASON_WEEKS`, `WEEK_MATCH_COUNTS`,
`is_break_week`), `crates/goat-tui/src/main.rs` (~line 402, `in_pre_season`),
`tasks/TASK-35-calendar-training-wiring.md` (why `goat-training` is NOT the target).

## Origin

Raised by Tùng 2026-08-30: "make this simulation as real as possible", training picked as
the first system to deepen. Scoped down from four candidate features to two after checking
the live code — see "Already built" below.

## Verified: current state (2026-08-30)

**Target the live crate, not the shelved one.** `crates/goat-training` is a standalone
Phase-1 prototype that nothing calls, and `TASK-35-calendar-training-wiring.md` has already
decided it gets **deleted** ("`goat-training`'s parallel growth model is redundant; absorb
only what's uniquely useful and delete the rest"). The canonical, live training math is
`goat-core::week.rs`. **All work in this task targets `week.rs`.** Do not extend
`goat-training` — it is a dead end.

**Already built — dropped from scope (do NOT rebuild):**
- *Overtraining → injury* is live. `week.rs::injury_prob` already factors energy ×
  intensity × age × lifestyle × durability (BL7), rolled every `advance_week`, with
  injury weeks set and training skipped while injured.
- *Facility / coach quality* is live and richer than a single multiplier:
  `state.rs::tick_one_week` composes `pc_facilities_mult` (club, from `goat-world`) ×
  lifestyle mult × `DEV_INVEST_MULT[pc_dev_invest_level]` into one `effective_mult` fed
  to `advance_week`.

**Genuinely missing — this task's scope:**
1. **Position-specific training.** `attr_growth_rate(archetype, age_years)` keys ONLY on
   the attribute's age-curve archetype and the player's age. A Target Forward and a Centre
   Back training Finishing at the same intensity/age/energy get **identical** growth.
   Position never enters the growth path. (`ROLE_WEIGHT_TABLE` already encodes exactly the
   needed per-role/per-attr relevance and is already used by `update_familiarity` — reuse
   it; do not author a second table.)
2. **Periodization.** `advance_week`'s signature is
   `(players, pc_id, routine, facilities_mult, lifestyle, rng)` — it has **no idea what
   week of the season it is**. Training cannot taper, base-build, or ease off in-season
   because the season phase is not an input.

## Notes and one open decision

### D1 — Golden values WILL move here, and that is now allowed

`golden_week.rs::golden_52_weeks_forward` asserts `CloseControl = 63.173` and
`Vision = 24.292` for a striker training CloseControl/BallControl/Vision/Finishing over 52
weeks. Both position relevance and phase modifiers change those numbers.

**Per CLAUDE.md (updated 2026-08-30), golden values are no longer frozen** — update them
and note what moved them, following the BL7 re-freeze comment already in that file. This
task therefore builds the real behavior directly; no inert-seam staging is required.

Still be deliberate about it: when a golden moves, confirm it moved *for the reason you
expect* and by roughly the magnitude you expect, rather than accepting whatever makes the
suite green. A `*_neutral_reproduces_pre_existing_growth` test (constants at ×1.0 reproduce
today's output byte-identically, matching
`week.rs::durability_neutral_value_reproduces_pre_existing_injury_numbers`) is a cheap way
to prove the *formula* is sound independently of the tuning — worth keeping even though
it's no longer mandatory.

### D2 — Season phase is not reachable from `goat-core`  `[DECISION NEEDED]`

`in_pre_season()` lives in **`goat-tui/src/main.rs:402`**, and `PRE_SEASON_WEEKS` /
`WEEK_MATCH_COUNTS` / `is_break_week` live in **`goat-world::calendar`**. `goat-core` does
not depend on `goat-world`, so neither is reachable from `week.rs` today. Worth noting
`in_pre_season` in the TUI is a mild "renderer is dumb" smell already — it derives sim-
relevant state renderer-side.

Good news: it derives purely from `state.season_number`, `state.season_round`, and
`season_week(state)` — **all already in `WorldState`** — so `goat-core` can compute the
phase itself with no new dependency.

**Recommended:** add a `SeasonPhase` enum + derivation to `goat-core`, compute it in
`state.rs::tick_one_week` (which already holds `state`), and pass it into `advance_week`.
Have the TUI's `in_pre_season` delegate to the core function rather than keeping its own
copy. Do NOT invert the core→world dependency; do NOT duplicate the week constants.

`[DECISION NEEDED]` Confirm phase derivation belongs in `goat-core` (recommended), and
whether the pre-season week count should move/mirror into `goat-core` or be passed in.

## Data model

```rust
// goat-core — NEW
pub enum SeasonPhase { PreSeason, InSeason, BreakWeek, OffSeason }

// goat-core::week — extended signature
pub fn advance_week(
    players: &mut PlayerStore, pc_id: PlayerId, routine: &Routine,
    facilities_mult: Fixed, lifestyle: u8,
    phase: SeasonPhase,        // NEW (D2)
    rng: &mut impl RngSource,
) -> Vec<DevelopmentEvent>;

// goat-core::tuning — NEW. TUNABLE placeholders per bible §11; pick defensible
// starting values (shape matters, exact numbers don't — final tuning is deferred).
pub const POSITION_RELEVANCE_KEY:  Fixed; // W_KEY attr for the role — trains fastest
pub const POSITION_RELEVANCE_IMP:  Fixed;
pub const POSITION_RELEVANCE_BAS:  Fixed;
pub const POSITION_RELEVANCE_ZERO: Fixed; // never 0 — off-position must stay trainable
pub const PHASE_GROWTH_PRESEASON:  Fixed; // base-building block trains hardest
pub const PHASE_GROWTH_INSEASON:   Fixed;
pub const PHASE_GROWTH_BREAK:      Fixed;
```

`[DECISION NEEDED]` Which role drives relevance? A player has 14 familiarity tiers, not one
role. Options: `PrimaryPosition` (coarse, stable), or the player's best-familiarity role
(granular, but shifts as familiarity grows — and a shifting multiplier is a determinism
footgun worth thinking through). Recommend `PrimaryPosition` for v1.

## Steps

### Step 1 — Position-specific growth
- Relevance lookup from `ROLE_WEIGHT_TABLE` + the four TUNABLE constants above, applied in
  `advance_week`'s per-attr growth alongside `growth_rate`/`intensity_mult`/`energy_factor`.
- Tests: an on-position attr out-gains an off-position one at equal age/intensity/energy,
  and off-position growth is still > 0 (no attribute is unreachable by position). Keep a
  neutral-reproduces-existing test (constants at ×1.0 ⇒ today's output byte-identical) as
  proof the formula itself is sound.
- Update `golden_52_weeks_forward` with a comment saying this change moved it.

### Step 2 — SeasonPhase + periodization (settle D2 first)
- `SeasonPhase` + derivation in `goat-core`; computed in `tick_one_week`, passed to
  `advance_week`; phase multipliers applied to growth.
- Point the TUI's `in_pre_season` at the core function (delete the renderer-side copy).
- Tests: derivation matches the TUI's current pre-season behavior for the same states;
  pre-season weeks grow faster than in-season at equal routine/energy.
- Update goldens again with a note.

### ⏸ PAUSE — after Step 1
Show which golden values moved, by how much, and confirm the movement matches expectations
before layering Step 2's phase modifiers on top. Two tuning changes landing at once makes
an unexpected shift much harder to attribute.

### Step 3 — Whole-sim sanity
- `./scripts/test.sh`, plus `career-sim --scan` and `./scripts/world-sim.sh` — confirm
  career arcs still look sane across seeds (peak ages, growth pace) rather than only that
  the suite is green. This is the real check that "more realistic" actually landed.

## Definition of done
1. `cargo test --workspace` green. Golden values that moved are updated with a comment
   naming the change that moved them, and the summary (#6) lists them.
2. `cargo fmt --check` + `clippy -D warnings` clean.
3. New behavior covered by golden-seed tests; neutral-reproduces-existing test present.
4. No new deps, no floats in sim, no unsafe, no I/O in core, **no sim logic left in the TUI**
   (`in_pre_season` delegates to core).
5. `goat-core` still does not depend on `goat-world`.
6. Summary: what changed + which bible/TRAINING.md sections it implements.

## Out of scope
- `crates/goat-training` — untouched; it is deleted by TASK-35, not extended here.
- Overtraining→injury and facility/coach quality — **already built** (see above).
- Wiring the CalendarEngine into the live loop — that is TASK-35, a separate task.
- Per-session/daily training micromanagement. Periodization here is phase-level modifiers,
  not a day-by-day planner; the manage-by-exception pillar (bible §2.2) still holds.
- Save format: only bump `VERSION` (currently 19) if a persisted periodization *choice* is
  added. The phase itself is derived, not stored — per CLAUDE.md's tiny-saves rule.
