# TASK DESIGN ROUND 5, SLICE 3-4 — Weakest-position detection & gem-hunting target search

**Split-file note (read this first):** this file is 1 of 6 that together replace
`tasks/TASK-DESIGN-round5-club-economy.md` (now a short pointer doc). Sibling files:
`-slice1-2-foundation.md`, `-slice5-transfers.md`, `-slice6-academy.md`,
`-slice7-8-managers.md`, `-slice9-integration.md`. This file is fully self-contained —
implement it without reading the others or the original doc.

Prereq: **`TASK-DESIGN-round5-club-economy-slice1-2-foundation.md` must be landed and committed
first (hard prereq).** This file's two search lanes both call that slice's `market_valuation`
(signature: `fn market_valuation(current_ovr: u8, potential_ovr: u8, age: u32) -> i64`) to filter
candidates by an affordability ceiling (`lane_cap: i64`, a plain parameter here — the caller,
a later sibling slice, is responsible for computing it from `Club.budget`; this file does not
need to know how).

Read first: `crates/goat-world/src/population.rs` (`Population`, `position` field,
`current_ovr`, `is_retired`, `age_years_at`, `potential_ovr`); `crates/goat-core/src/derive.rs`
+ `crates/goat-core/src/roles.rs` (`role_rating`, `ROLE_WEIGHT_TABLE` — why these are *not*
reusable for population-wide transfer search, see "Verified").

## Ground rules for this file

- **"Weakest position," not "weakest role," for population-wide search — a real constraint,
  not a simplification of convenience.** See "Verified" for the concrete reason: the 14-role
  `role_rating`/`ROLE_WEIGHT_TABLE` system needs full per-attribute `current` values, which
  background (non-lazy-promoted) players do not carry. Searching the whole ~29,000-player
  background population at role granularity would force realizing (`Population::promote`)
  most of the population every window, defeating the entire "cheap identity, full realization
  on contact" principle (bible §9, `population.rs:1-9`) this codebase is built around. This
  file's target search uses the coarse 3-way `position` field (`population.rs:38-39`:
  Defender/Midfielder/Forward) plus `current_ovr`, both cheap SoA columns, for the whole-market
  scan; the 14-role machinery is reserved for what it already does (orbit-path role fit,
  Doc B's national-team call-ups).
- **"Generated but consistent."** Any RNG this file's siblings add on top of these search
  results must be its own forked stream (not applicable to this file directly — both functions
  below are deterministic, no RNG).

## Verified: current mechanics and constraints

- **Why population-wide role-granularity search is not viable.** `derive::role_rating`
  (`derive.rs:43-61`) requires `current: &[Fixed; NUM_ATTRS]` — the full 30-attribute vector.
  Background (non-realized) players only carry `potential_ovr: Vec<u8>` (`population.rs:44`);
  their full per-attribute `current` only exists after `Population::promote`
  (`population.rs:204-229`) lazy-realizes them — a per-player-seed call to the same full
  generation pipeline the PC's own creation uses. Running this for even "just each club's own
  squad" every transfer window, across 1,200 clubs × ~24 players, is ~28,800 realizations per
  window — roughly the *entire* background population, every window, forever. That is exactly
  the cost bible §9 lazy-promotion exists to avoid. `current_ovr` (`population.rs:189-193`), by
  contrast, is a cheap closed-form function of `potential_ovr` + age — no realization needed,
  and it's the same formula `batch_tick.rs`'s own `club_strength` already scans the whole
  population with every season. This file's target search reuses that existing,
  already-population-scale-proven cost profile.
- **`Population`'s cheap SoA columns this file reads**: `position: Vec<u8>` (0=Defender,
  1=Midfielder, 2=Forward, `population.rs:38-39`), `current_ovr(i, elapsed_weeks) -> u8`,
  `is_retired(i, elapsed_weeks) -> bool`, `age_years_at(i, elapsed_weeks) -> u32`,
  `potential_ovr: Vec<u8>` (`population.rs:44`), `club: Vec<u16>`.

## Slice 3 — Weakest-position detection & target search

### 3.1 — Detecting the gap: coarse `position`, not 14-role `RoleId` (per "Verified")

```rust
/// Which of the 3 coarse positions has this club's weakest *best* player, by current OVR.
/// Cheap: one pass over the (small, `~18-30`-player) squad, no realization.
fn weakest_position(pop: &Population, squad: &[usize], elapsed_weeks: u32) -> Option<u8> {
    (0u8..3).min_by_key(|&pos| {
        squad
            .iter()
            .filter(|&&i| pop.position[i] == pos && !pop.is_retired(i, elapsed_weeks))
            .map(|&i| pop.current_ovr(i, elapsed_weeks))
            .max()
            .unwrap_or(0) // an empty position group reads as maximally weak — correct: a
                           // club with *no* forward at all should prioritize buying one
    })
}
```

### 3.2 — Target search: one precomputed sorted list per position, per window, not per club

```rust
/// Built once per window (not once per club — 1,200 clubs re-scanning the whole population
/// each would be the same cost mistake `batch_tick.rs`'s own doc-comment on
/// `live_strength`/`live_strength_from_squad` already warns against, round-3 §3.2). Each
/// position's `Vec` is sorted descending by `current_ovr` once; every club's search below is
/// then a bounded prefix scan, not a rescan.
fn candidates_by_position(
    pop: &Population,
    elapsed_weeks: u32,
) -> [Vec<usize>; 3] {
    let mut out: [Vec<usize>; 3] = Default::default();
    for i in 0..pop.len() {
        if !pop.is_retired(i, elapsed_weeks) {
            out[pop.position[i] as usize].push(i);
        }
    }
    for list in &mut out {
        list.sort_by_key(|&i| std::cmp::Reverse(pop.current_ovr(i, elapsed_weeks)));
    }
    out
}

/// A club's single weakest-position target for this pass, or `None` if nothing affordable
/// beats what it already has. `lane_cap` is this club's weakest-position spending ceiling
/// this window (a sibling slice's `Club.budget`-derived value).
fn weakest_position_target(
    club_id: ClubId,
    pop: &Population,
    squad: &[usize],
    candidates: &[Vec<usize>; 3],
    lane_cap: i64,
    elapsed_weeks: u32,
) -> Option<usize> {
    let pos = weakest_position(pop, squad, elapsed_weeks)?;
    let own_best = squad
        .iter()
        .filter(|&&i| pop.position[i] == pos)
        .map(|&i| pop.current_ovr(i, elapsed_weeks))
        .max()
        .unwrap_or(0);
    // Bounded prefix scan (e.g. top 50 by current_ovr in this position) — a real upgrade
    // (current_ovr > own_best), not already at this club, and its market_valuation fits
    // within this lane's cap.
    candidates[pos as usize]
        .iter()
        .take(TARGET_SEARCH_PREFIX)
        .copied()
        .find(|&i| {
            pop.club[i] as usize != club_id
                && pop.current_ovr(i, elapsed_weeks) > own_best
                && market_valuation(
                    pop.current_ovr(i, elapsed_weeks),
                    pop.potential_ovr[i],
                    pop.age_years_at(i, elapsed_weeks),
                ) <= lane_cap
        })
}

const TARGET_SEARCH_PREFIX: usize = 50; // Design's own bound, see "Decisions"
```

### TDD anchor (Slice 3)

- `weakest_position_finds_the_real_gap`: a squad with a strong defense/midfield and zero
  forwards returns `Forward`, matching the "empty group reads maximally weak" comment.
- `target_search_never_returns_own_players_or_downgrades`: for any squad/candidate list, the
  returned target (if any) is never already at `club_id` and always has strictly higher
  `current_ovr` than `own_best`.
- `target_search_respects_lane_cap`: a target whose `market_valuation` exceeds `lane_cap` is
  never returned, even if it's the top-ranked candidate.
- `candidates_by_position_is_deterministic_and_sorted`: same population twice → identical
  sorted lists; every list is non-increasing in `current_ovr`.

## Slice 4 — Gem-hunting target search

### 4.1 — Scoring: the current/potential gap `market_valuation` deliberately underprices

```rust
const GEM_HUNT_MAX_AGE: u32 = 21; // only unproven-ceiling players are "gems" — a peaked
                                   // veteran has no unrealized potential left to buy cheap

fn gem_hunt_score(current_ovr: u8, potential_ovr: u8, age: u32) -> i64 {
    if age > GEM_HUNT_MAX_AGE {
        return 0;
    }
    (potential_ovr as i64 - current_ovr as i64).max(0)
}
```

This exploits the exact market inefficiency `TASK-DESIGN-round5-club-economy-slice1-2-
foundation.md`'s §2.3 documents: `market_valuation` prices mostly off *current* OVR, so a young
player with a high `potential_ovr` but still-low `current_ovr` (round-3's outlier roll,
`OUTLIER_CHANCE_PCT = 2`, can produce this at any club regardless of strength) is cheap by that
formula despite a high ceiling — this lane's whole job is to find those players before their
`current_ovr` catches up to their price.

### 4.2 — Target search: whole population, not position-gated (gem-hunting is proactive, not
gap-filling — per Tùng's explicit "not just reactive to squad gaps" requirement)

```rust
/// Reuses the same `candidates_by_position` lists (3.2) purely as a cheap, already-
/// built "who's out there" index — re-sorted here by `gem_hunt_score` instead of
/// `current_ovr`, since this lane's ranking question is different from the weakest-position
/// lane's. Built once per window, same cost discipline as 3.2.
fn gem_targets_by_position(
    pop: &Population,
    candidates: &[Vec<usize>; 3],
    elapsed_weeks: u32,
) -> [Vec<usize>; 3] {
    let mut out = candidates.clone();
    for list in &mut out {
        list.sort_by_key(|&i| {
            std::cmp::Reverse(gem_hunt_score(
                pop.current_ovr(i, elapsed_weeks),
                pop.potential_ovr[i],
                pop.age_years_at(i, elapsed_weeks),
            ))
        });
    }
    out
}

fn gem_hunt_target(
    club_id: ClubId,
    pop: &Population,
    gem_lists: &[Vec<usize>; 3],
    lane_cap: i64,
    elapsed_weeks: u32,
) -> Option<usize> {
    // Scan across all 3 position lists' top prefixes, pick the single highest-scoring
    // affordable candidate overall — gem-hunting doesn't care which position the gem plays.
    gem_lists
        .iter()
        .flat_map(|list| list.iter().take(TARGET_SEARCH_PREFIX).copied())
        .filter(|&i| {
            pop.club[i] as usize != club_id
                && gem_hunt_score(
                    pop.current_ovr(i, elapsed_weeks),
                    pop.potential_ovr[i],
                    pop.age_years_at(i, elapsed_weeks),
                ) > 0
                && market_valuation(
                    pop.current_ovr(i, elapsed_weeks),
                    pop.potential_ovr[i],
                    pop.age_years_at(i, elapsed_weeks),
                ) <= lane_cap
        })
        .max_by_key(|&i| {
            gem_hunt_score(
                pop.current_ovr(i, elapsed_weeks),
                pop.potential_ovr[i],
                pop.age_years_at(i, elapsed_weeks),
            )
        })
}
```

### TDD anchor (Slice 4)

- `gem_hunt_score_zero_past_max_age`: any player with `age > GEM_HUNT_MAX_AGE` scores exactly
  `0`, regardless of potential/current gap.
- `gem_hunt_prefers_outlier_style_prospects`: construct a population where one weak-club young
  player has a round-3-outlier-style high `potential_ovr` far above its low `current_ovr` —
  assert it's the top-ranked gem target ahead of ordinary anchor-formula players at the same
  club — the direct regression for the foundation slice's §2.3 claim.
- `gem_hunt_ignores_position_gaps`: a club with no squad weakness at all (every position
  already strong) can still return a non-`None` gem target, unlike Slice 3's
  `weakest_position_target`.

## Out of scope (this file)

- Computing `lane_cap` from `Club.budget`, running these searches across all clubs in a pass,
  the auction, and applying any resulting transfer — all `TASK-DESIGN-round5-club-economy-
  slice5-transfers.md`'s work. This file only builds the two target-search functions that
  slice calls.
- Youth-academy investment, managers, season-tick wiring — other sibling files.

## Decisions Design made as judgment calls — flag for Tùng's explicit sign-off

4. **Slice 3.2**: `TARGET_SEARCH_PREFIX = 50` — a perf/quality tradeoff bound, not measured
   against a real `--release` benchmark this round (same caveat round-2's A2.5/A3.1 items
   flagged for their own perf numbers).
5. **Slice 4.1**: `GEM_HUNT_MAX_AGE = 21` — Design's own cutoff, matching round-3's own choice
   to treat teenage-to-early-20s as the "prospect" band implicitly (round-3's genesis age range
   is `16..33`, `population.rs:129`).

(Numbering preserved from the original doc's full "Decisions" list, items 1-3 and 6-15 live in
sibling files.) These are first-pass numbers for a later `TASK-TUNE` pass once playtested — not
blocking items Tùng needs to approve before Dev starts.

## Definition of done (Slice 3-4)

1. `cargo test --workspace` green, including every TDD-anchor test listed above.
2. `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` clean.
3. No new dependencies. No floats in sim state/logic, no unsafe.
4. No new persisted fields in this file — no save-version bump needed here.
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
6. **Commit this slice before starting `-slice5-transfers.md`.**
