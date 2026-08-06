# TASK DESIGN ROUND 9 — Per-player injury-proneness/durability trait (BL7)

Prereq: none. Self-contained within `goat-core` (`player.rs`, `week.rs`).

Read first: `crates/goat-core/src/week.rs:326-348` (`injury_prob` — the formula this doc adds
one coefficient to), `crates/goat-core/src/player.rs:20-42` (`PlayerStore` — the struct this
doc adds one column to), `crates/goat-core/src/tuning.rs:167-190` (existing injury-tuning
constants, incl. `lifestyle_injury_x10`, the closest analog to the new coefficient).

## Origin

Raised by Tùng 2026-07-22, clarified fully in `tasks/TASK-DESIGN-round2-world-genesis-scaleup.md`'s
"Parked for a future design round" section (BL7). This doc formalizes that already-settled
clarification into a proper spec — no new open questions were needed from Tùng tonight
(2026-07-23) beyond confirming "go write it."

## Verified: current state

`injury_prob(energy, intensity, age_years, lifestyle)` (`week.rs:326-348`) already multiplies
four `x10`-scaled coefficients — fatigue (from `energy`), training intensity, age band, and
lifestyle (`lifestyle_injury_x10`, `tuning.rs`) — then divides by `10_000` (four factors, each
contributing one power of 10) to get injury probability per 1000 weekly rolls. Called from
exactly one site (`week.rs:142`), the PC's own weekly tick — this mechanic applies to the
player-character only, not the background `Population` (which has no per-week individual
simulation; its injuries, if any, are handled by unrelated batch-tick aggregates). Today, two
PCs with identical energy/intensity/age/lifestyle have byte-identical injury risk — no player is
innately more fragile or durable than another.

## Decision (final, per Tùng's 2026-07-22 clarification, reconfirmed 2026-07-23)

**1. New coefficient, not a new subsystem.** Add one more per-player multiplier,
`durability_x10: u8` (or similar `x10`-scaled small int, matching this codebase's existing
fixed-point-multiplier idiom), rolled once at player creation and stored as a new
`PlayerStore` column (`Vec<u8>`, alongside `age_weeks`/`energy`/`injury_weeks`). `injury_prob`
gains a fifth factor: `... * durability_x10 / 100_000` (divisor bumped from `10_000` to
`100_000` for the fifth factor). A higher `durability_x10` value should mean a **lower** injury
risk (durable = fewer injuries) — i.e. the value should be inverted the same way
`lifestyle_injury_x10` already is (Pro < Balanced < Flashy in injury-multiplier terms), NOT a
direct multiply-durability-in — confirm this inversion in code review, since getting the sign
backwards would silently make "durable" players get hurt MORE.

**2. Roll range and distribution**: seed-derived once at PC creation (same
`GoatRng::new(seed_mix(...))` idiom as every other per-player generated trait), a moderate
spread around a neutral midpoint — e.g. roll uniform in a range whose midpoint reproduces
today's exact injury numbers unchanged (so this is purely additive variance, not a global
rebalance) and whose extremes give a noticeably more/less injury-prone player without being
absurd (a "never injured" or "constantly injured" player would break the mechanic's believability
and also make it too easy to reverse-engineer from observed outcomes). Concrete recommendation:
`durability_x10` uniform in `[7, 13]` (±30% around the neutral `10` = ×1.0), same shape as
`INJURY_LIFESTYLE_X10_*`'s existing spread in `tuning.rs` — Dev should read those constants'
actual values before picking the exact range so the new coefficient's spread is proportionate,
not guessed independently.

**3. Hidden from the player, always.** No UI surfaces the raw number, ever — matches OVR's own
position-weighted formula (also never shown raw) and this codebase's existing "some numbers are
observable through outcomes only" pattern. Only observable via actual injury frequency over a
career, never as a stat screen value.

**4. Fixed/innate, no club-medical-investment hook yet.** Confirmed explicitly: keep this round
small. A later round (BL3's club-economy budget system, or a fresh round) can multiply an
additional "club medical quality" coefficient into the same formula without redesigning
anything here — this doc's `durability_x10` stays a pure, unmodifiable-by-play constant for now.

**5. No persisted state beyond what's already persisted.** `durability_x10` is a pure function
of the PC's own creation seed — recomputed on demand exactly like `potential`, not a new
`SaveData::VERSION` bump, UNLESS this codebase's actual `PlayerStore`/save format already
persists other per-PC seed-derivable columns directly rather than re-deriving them (check
`goat-save`'s current `PlayerStore` (de)serialization before assuming — if `potential`/other
columns are persisted as raw data rather than re-derived from seed on load, `durability_x10`
should follow the same precedent for consistency, not fight it).

## TDD anchors

- `durability_is_deterministic_per_seed`: same creation seed → same `durability_x10`.
- `durability_varies_across_seeds`: different seeds produce different values within the
  designed range.
- `higher_durability_reduces_injury_probability`: two otherwise-identical `injury_prob` calls
  differing only in `durability_x10` — the higher-durability one must yield a strictly lower
  probability (catches an accidental sign inversion).
- `durability_neutral_value_reproduces_pre_existing_injury_numbers`: with `durability_x10` at
  its neutral midpoint, `injury_prob`'s output is byte-identical to its pre-this-doc value for
  the same energy/intensity/age/lifestyle inputs — confirms this is additive variance, not a
  silent global rebalance.
- `durability_never_serialized_or_displayed_raw`: grep-level check (or a UI-surface test if one
  exists) confirming no screen/DTO exposes the raw value.

## Playable gate

`cargo run -p goat-tui` → full-career playtest (or the existing career-sim harness) shows
injury frequency varying observably between different creation seeds at otherwise-identical
lifestyle/training choices — confirms the coefficient is live, not dead code.

## Out of scope

- Club medical-quality investment hook (explicitly deferred, see Decision 4).
- Applying this to the background `Population` (out of scope — no per-week individual
  simulation exists for them; would be a much larger, separately-scoped change if ever wanted).
- Any UI/display work beyond confirming the value stays hidden.
