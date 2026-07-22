# TASK DESIGN ROUND 5, SLICE 6 — Youth-academy investment lane

**Split-file note (read this first):** this file is 1 of 6 that together replace
`tasks/TASK-DESIGN-round5-club-economy.md` (now a short pointer doc). Sibling files:
`-slice1-2-foundation.md`, `-slice3-4-scouting.md`, `-slice5-transfers.md`,
`-slice7-8-managers.md`, `-slice9-integration.md`. This file is fully self-contained — implement
it without reading the others or the original doc.

Prereq: **both `TASK-DESIGN-round5-club-economy-slice1-2-foundation.md` and
`TASK-DESIGN-round5-club-economy-slice5-transfers.md` must be landed and committed first (hard
prereq)** — this file spends `lane_cap(club.budget, LANE_YOUTH_INVESTMENT_PCT)`, both defined in
the transfers slice, against `Club.budget` from the foundation slice. It also composes directly
with `tasks/TASK-DESIGN-round3-player-driven-club-strength.md`'s Slice 4 (youth intake) and
Slice 2 (outlier roll) — **verified already implemented on disk**, see "Verified" below.

Read first: `tasks/TASK-DESIGN-round3-player-driven-club-strength.md` §4.2-4.6 (youth intake:
`apply_youth_intake`, `roll_potential_ovr`, `intake_week`) and §2.1 (`roll_potential_ovr`'s
outlier mechanism); `crates/goat-world/src/population.rs` (`Population`, intake machinery).

## Ground rules for this file

- **Composes with round-3's existing youth intake rather than replacing it.** This slice's only
  ripple into that code is the anchor value passed into `roll_potential_ovr` (6.4) — no change
  to `roll_potential_ovr`'s own signature, the outlier-roll mechanism, the intake-count formula,
  or the `intake_week` column.
- **Every invented number is flagged** — see "Decisions" below.

## Verified: grounding for this file

- Round-3 Slice 4's `apply_youth_intake` rolls each new player's `potential_ovr` via
  `roll_potential_ovr(rng, club.strength)` (`tasks/TASK-DESIGN-round3-player-driven-club-
  strength.md` §2.1/§4.6) — already implemented on disk as of 2026-07-22, in the same working
  tree (a concurrent Dev round shipped it). This file's Slice 6 hooks into it via a one-line
  anchor-value substitution at the existing call site (6.4).
- `lane_cap(club_budget: i64, pct: i64) -> i64` and `LANE_YOUTH_INVESTMENT_PCT: i64 = 20` are
  defined in `TASK-DESIGN-round5-club-economy-slice5-transfers.md` §5.1 — this file's academy
  spend for a window is exactly `lane_cap(club.budget, LANE_YOUTH_INVESTMENT_PCT)`, the third of
  the three lane shares that slice defines (weakest-position 50%, gem-hunt 30%, youth-investment
  20%, spent in that priority order against the club's live post-earlier-lanes budget).

## Slice 6 — Youth-academy investment lane

Distinct lever from the transfers slice: instead of buying from another club, a club spends its
remaining window budget on its **own** academy.

### 6.1 — Data shape

```rust
pub struct Club {
    // ...existing fields, plus the foundation slice's `budget`...
    /// How much this club's own academy currently out-punches its genesis `strength` for
    /// intake purposes. NEW. Decays without reinvestment (6.3) — an ongoing commitment, not
    /// a one-time purchase.
    pub academy_boost: u8,
}

const ACADEMY_BOOST_MAX: u8 = 20;
```

### 6.2 — Investment: diminishing returns

```rust
fn apply_academy_investment(club: &mut Club, spend: i64) {
    if spend <= 0 {
        return;
    }
    // Gets pricier per point the higher the existing boost — early investment is cheap,
    // pushing a boosted academy even further gets progressively harder.
    let cost_per_point = 1_000 + (club.academy_boost as i64) * 300;
    let gained = (spend / cost_per_point.max(1)) as u8;
    club.academy_boost = club.academy_boost.saturating_add(gained).min(ACADEMY_BOOST_MAX);
    club.budget -= spend; // real spend, not a loan
}
```

A club's youth-investment lane spend this window is simply `lane_cap(club.budget,
LANE_YOUTH_INVESTMENT_PCT)` (the transfers slice's §5.1) — no target search needed, the whole
lane amount goes straight into `apply_academy_investment` if the club chooses to invest (see 6.4
for the choice-gating).

### 6.3 — Decay: sustained investment required

```rust
const ACADEMY_BOOST_DECAY_PCT: i64 = 15; // per season, not per window

fn decay_academy_boost(club: &mut Club) {
    club.academy_boost = ((club.academy_boost as i64 * (100 - ACADEMY_BOOST_DECAY_PCT)) / 100) as u8;
}
```

Called once per season (the integration slice's wiring), after both windows' investment for
that season has landed — so a club gets up to two investment opportunities per season before
any decay bites.

### 6.4 — Hook into round-3's existing intake formula: one call-site change

Round-3 Slice 4's `apply_youth_intake` rolls each new player's `potential_ovr` via
`roll_potential_ovr(rng, club.strength)` (round-3 §2.1/§4.6). This slice's only ripple into
that existing code is the anchor value passed in:

```rust
// Round-3's existing call site, minimally changed:
let effective_strength = club.strength.saturating_add(club.academy_boost).min(99);
let potential_ovr = roll_potential_ovr(&mut rng, effective_strength); // was: club.strength
```

No change to `roll_potential_ovr`'s own signature, the outlier-roll mechanism, the intake-count
formula (round-3 §4.2/§4.3), or the `intake_week` column — this is exactly the "compose without
redesigning" instruction, a one-line anchor substitution at the one call site round-3 already
built.

**Whether to invest at all, this round:** a simple always-invest-the-full-lane-cap policy (no
club ever leaves this lane's money on the table) — Design's simplification, flagged below,
since Tùng's brief didn't specify a club-level "prefer buying over academy investment" decision
rule and a fixed policy is the smallest concrete choice that satisfies "clubs invest in their
own youth academy" without inventing a whole club-personality system.

### TDD anchor (Slice 6)

- `academy_investment_has_diminishing_returns`: equal `spend` at `academy_boost = 0` vs.
  `academy_boost = 10` gains fewer points at the higher starting boost.
- `academy_boost_never_exceeds_cap`: repeated large investments still clamp at
  `ACADEMY_BOOST_MAX`.
- `academy_boost_decays_without_reinvestment`: a club with `academy_boost > 0` and zero spend
  for several seasons trends toward `0`.
- `academy_boost_raises_effective_intake_anchor`: round-3's `youth_intake_uses_shared_outlier_
  formula`-style test, re-run with a nonzero `academy_boost`, shows a higher mean
  `potential_ovr` among that club's intake vs. an identical club with `academy_boost = 0` —
  the end-to-end regression tying this slice to round-3's existing mechanic.

## Out of scope (this file)

- **Sponsorship, matchday/ticket sales, shirt sales, prize-money income contributors** —
  unrelated to this lane, see the foundation slice.
- Weakest-position/gem-hunt search, the auction, managers, season-tick wiring — other sibling
  files.
- A club-level "buy vs. develop" preference system — the always-invest-full-lane-cap policy
  (6.4) is the deliberate simplification in place of this.

## Decisions Design made as judgment calls — flag for Tùng's explicit sign-off

9. **Slice 6**: `ACADEMY_BOOST_MAX = 20`, the `1,000 + boost×300` diminishing-returns cost
   curve, and `ACADEMY_BOOST_DECAY_PCT = 15` — Design's own numbers for a lever Tùng specified
   only at the concept level ("invest in own youth academy").
10. **Slice 6.4**: the "always invest the full youth-investment lane cap" policy — a
    simplification Design chose over building a per-club "buy vs. develop" preference system,
    flagged explicitly in 6.4's own text.

(Numbering preserved from the original doc's full "Decisions" list.) These are first-pass
numbers for a later `TASK-TUNE` pass once playtested — not blocking items Tùng needs to approve
before Dev starts.

## Definition of done (Slice 6)

1. `cargo test --workspace` green, including every TDD-anchor test listed above.
2. `goat-save::save::VERSION` bumped (if not already bumped by a landed sibling slice) to cover
   the new `Club.academy_boost` field, serialized/deserialized and round-tripped
   (`crates/goat-save/tests/save_roundtrip.rs` extended, mirroring how round-3 flagged
   `intake_week` for the same treatment).
3. `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` clean.
4. No new dependencies. No floats in sim state/logic, no unsafe.
5. No new failures beyond the **10 pre-existing `goat-tui` `smoke_stdin` failures** (verified
   2026-07-22, out of scope, unrelated to this work, caused by a `generate_club_name()` bug in
   `crates/goat-world/src/world.rs`): `confirm_screen_blank_enter_reprompts_instead_of_
   discarding_character`, `double_w_in_same_round_shows_message_not_silent_noop`,
   `game_sheet_and_player_sheet_boxes_close_for_short_and_long_club_names`, `key_moments_lines_
   close_with_ellipsis_not_ragged_cutoff`, `legacy_screen_notes_mid_season_batching`,
   `main_loop_unrecognized_command_messages_and_continues`, `player_sheet_explains_ovr_is_
   position_weighted`, `save_overwrite_requires_explicit_confirmation`, `save_to_empty_slot_
   succeeds_without_confirmation`, `status_header_shows_energy_percent_and_labeled_discipline_
   count`.
6. **Commit this slice before starting `-slice7-8-managers.md`.**
