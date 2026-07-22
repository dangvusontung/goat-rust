# TASK DESIGN ROUND 5, SLICE 7-8 — Manager entity, appointment, performance & firing

**Split-file note (read this first):** this file is 1 of 6 that together replace
`tasks/TASK-DESIGN-round5-club-economy.md` (now a short pointer doc). Sibling files:
`-slice1-2-foundation.md`, `-slice3-4-scouting.md`, `-slice5-transfers.md`,
`-slice6-academy.md`, `-slice9-integration.md`. This file is fully self-contained — implement
it without reading the others or the original doc.

Prereq: **none from this round's other slices.** The manager subsystem (entity, appointment,
form tracking, firing/rehire) is independent of budgets/transfers/academy investment — only the
integration slice wires managers together with the rest of the economy. This file does depend
on already-shipped code from earlier rounds: `TacticalIdentity`
(`crates/goat-core/src/tactical_identity.rs`, round-2 Doc B,
`tasks/TASK-DESIGN-round2-national-team-tactical-identity.md`) and `batch_tick_season`
(`crates/goat-world/src/batch_tick.rs`).

Read first: `crates/goat-core/src/tactical_identity.rs` (`TacticalIdentity`, `NUM_ROLES`,
`role_weight`, `generate`, the sum-to-`NUM_ROLES` invariant); `crates/goat-world/src/
batch_tick.rs:73-167` (`batch_tick_season`, the per-round match-sim loop this file's Slice 8
adds a sibling function alongside); `crates/goat-world/src/history.rs:76-83`
(`name_from_seed`, the 16×16 name word bank, reused verbatim for manager names);
`crates/goat-world/src/world.rs:140-152` (`seed_mix`, `nation_seed`'s `0xA1`/`club_seed`'s
`0xB2` salts — the convention this file's `manager_seed` follows and the next-unused-byte guess
it makes).

## Ground rules for this file

- **Managers are a new, lightweight, non-SoA entity type.** There are only ~1,200 of them (one
  per club) plus a small reserve pool; this is `Club`-scale data (small `Vec` of a small
  struct), not `Population`-scale, so no column-oriented storage is needed.
- **"Generated but consistent."** Manager generation (7.2), identity-shift blending (7.3), and
  the fire/rehire draw (8.4) are each pure functions of `world_seed` (+ manager/club/season
  indices), on forked seed streams, never sharing state with match/transfer/injury RNG.
- **Every invented number is flagged** — see "Decisions" below.

## Verified: grounding for this file

- **The seed-mixing convention this file's deterministic manager draws follow.**
  `world.rs:140-144` (`seed_mix(world_seed, salt, idx)`) and `population.rs:109-113`/`461-467`
  (`player_seed`/`intake_player_seed`, the same idiom, each module keeping its own local copy —
  no shared "seed util" module exists in this codebase). This file's `manager_seed` (7.2) adds
  its own local copy, matching existing precedent.
- **Name generation for a new person-like entity is already solved.** `history::name_from_seed`
  (`history.rs:81-83`, public) wraps the existing 16×16 first/last-name word bank
  (`history.rs:76-79`) used for promoted/cohort players and pantheon greats. Slice 7 reuses it
  directly for manager names — no new word bank.
- **`batch_tick_season` (`batch_tick.rs:73-167`) already computes live per-club strength and
  iterates every match of every division, round by round** (`for round in 0..ROUNDS_PER_SEASON {
  for f in round_fixtures(...) { let (gf, ga) = sim_team_match(...); table.apply_result(...); }
  }`, `batch_tick.rs:93-97`). Per-match, per-club results (`gf`/`ga`, hence win/draw/loss) are
  computed transiently inside this loop but never captured per-club outside the aggregate
  `Table` — Slice 8 needs them (for manager rolling form) and adds a zero-ripple sibling
  function to capture them, same pattern round-3 Slice 5 used for
  `generate_player`/`generate_player_biased`.
- **`goat-save::save::VERSION` is currently `10`** (`save.rs:30`) — every new persisted field
  this file adds (the manager pool, per-club manager assignment) requires a version bump.

## Slice 7 — Manager entity, appointment, and tactical-identity shift

### 7.1 — Why a new lightweight entity, not a reuse of `Club` or a player

Managers aren't players (no attributes, no position, no `potential_ovr`, no retirement-by-age)
and aren't part of `Club`'s own identity (a club survives a managerial change; `strength`/
`tactical_identity`/`budget` all persist across a firing). A small, separate, non-SoA struct —
same weight class as `Club` itself (a handful of fields, ~1,200 of them, no per-attribute
columns) — is the natural fit; `Population`'s SoA discipline doesn't apply here because the
count is two orders of magnitude smaller and nothing about a manager needs per-attribute
storage.

```rust
pub type ManagerId = u32;

pub struct Manager {
    pub id: ManagerId,
    pub name: String, // reuses history::name_from_seed, "Verified" above
    /// The manager's own natural tactical bias — same type Doc B already built, reused
    /// verbatim, not extended.
    pub identity_bias: TacticalIdentity,
    /// Rolling points-per-match ring buffer (3/1/0), most recent overwrites oldest. Used by
    /// Slice 8's firing trigger.
    pub recent_points: [u8; MANAGER_FORM_WINDOW],
    pub recent_idx: u8,
    pub tenure_start_season: u32,
    /// Count of matches recorded into `recent_points` since this manager's current tenure
    /// began (reset to 0 on every hire, including rehire). Gates Slice 8.3's firing trigger
    /// against evaluating a manager off a not-yet-full ring buffer — added here, in this
    /// slice's own struct shape, specifically so Slice 8's Dev pass never needs to modify this
    /// already-shipped struct (see Slice 8.3 for why this field exists).
    pub matches_played: u16,
}

const MANAGER_FORM_WINDOW: usize = 10; // rolling 10-match form window, flagged in "Decisions"

/// The whole manager population: one per club, plus a reserve pool. A closed system — total
/// manager count is fixed at genesis (`NUM_CLUBS + MANAGER_POOL_SIZE`); firing returns a
/// manager to `free_agents`, hiring draws one out. Never grows, never runs out.
pub struct ManagerPool {
    pub managers: Vec<Manager>,       // index = ManagerId as usize
    pub club_manager: Vec<ManagerId>, // per-club current manager, index = ClubId
    pub free_agents: Vec<ManagerId>,  // currently unemployed, available to hire
}

const MANAGER_POOL_SIZE: usize = NUM_CLUBS / 4; // 300 — flagged in "Decisions"
```

Threaded through `ReplayCache` next to `pop` (`promotion.rs`'s existing `ReplayCache` struct
gains a `managers: ManagerPool` field, wired by the integration slice), same "new mutable,
persisted, replay-advanced state" treatment `pop` itself already gets.

### 7.2 — Generation: seeded, deterministic, following existing local-helper precedent

```rust
fn manager_seed(world_seed: u64, m: usize) -> u64 {
    world_seed
        ^ 0xD3u64.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (m as u64).rotate_left(23).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
}

impl Manager {
    fn generate(world_seed: u64, id: ManagerId) -> Self {
        let seed = manager_seed(world_seed, id as usize);
        Manager {
            id,
            name: history::name_from_seed(seed),
            identity_bias: TacticalIdentity::generate(seed ^ 1), // distinct sub-seed from the
                                                                  // name draw, same "^1" idiom
                                                                  // tactical_identity.rs's own
                                                                  // callers already use for
                                                                  // sibling-but-distinct rolls
            recent_points: [0; MANAGER_FORM_WINDOW],
            recent_idx: 0,
            tenure_start_season: 0,
            matches_played: 0,
        }
    }
}

impl ManagerPool {
    pub fn genesis(world_seed: u64, world: &WorldGenesis) -> Self {
        let total = world.clubs.len() + MANAGER_POOL_SIZE;
        let managers: Vec<Manager> = (0..total)
            .map(|m| Manager::generate(world_seed, m as ManagerId))
            .collect();
        let club_manager: Vec<ManagerId> = (0..world.clubs.len() as ManagerId).collect();
        let free_agents: Vec<ManagerId> =
            (world.clubs.len() as ManagerId..total as ManagerId).collect();
        Self { managers, club_manager, free_agents }
    }
}
```

This mirrors `world.rs`'s own local `seed_mix`-style helper rather than importing it (that
function is module-private to `world.rs`; `population.rs` already keeps its own separate copy
of the same idiom rather than exporting a shared one — existing precedent, not a new pattern).
Salt `0xD3` is Design's own next-unused-byte guess after `nation_seed`'s `0xA1`/`club_seed`'s
`0xB2` (`world.rs:146-152`) — Dev should grep for any other salt bytes in use before locking
this in, since Design did not exhaustively audit every salt constant in the codebase.

### 7.3 — Appointment: a blend, not a hard overwrite

```rust
const MANAGER_IDENTITY_BLEND_PCT: i64 = 700; // new manager's weight, out of 1000 (Fixed scale)

/// Called whenever a club gets a new manager (initial genesis assignment excluded — genesis
/// managers and genesis clubs start with independently-generated identities, no blend needed
/// at t=0). Shifts `club.tactical_identity` toward the incoming manager's own bias rather
/// than replacing it outright — "shift," per Tùng's own word, not "reset."
fn apply_manager_identity_shift(club: &mut Club, manager: &Manager) {
    for r in 0..NUM_ROLES {
        let old = club.tactical_identity.role_weight[r].to_raw() as i64;
        let new = manager.identity_bias.role_weight[r].to_raw() as i64;
        let blended = (old * (1000 - MANAGER_IDENTITY_BLEND_PCT) + new * MANAGER_IDENTITY_BLEND_PCT) / 1000;
        club.tactical_identity.role_weight[r] = Fixed::raw(blended as i32);
    }
    // Re-normalize so the weights still sum to NUM_ROLES * 1.0, the invariant
    // TacticalIdentity::generate itself maintains (tactical_identity.rs:45-49) — a blend of
    // two already-normalized vectors can drift slightly from integer rounding; Dev re-applies
    // the same sum-and-rescale step `generate` uses, not a new normalization routine.
}
```

70/30 (new manager dominant, old identity as inertia) is Design's own split — flagged below.

### TDD anchor (Slice 7)

- `manager_genesis_pool_is_closed`: `managers.len() == club_manager.len() + free_agents.len()`
  always, immediately after genesis and after any sequence of fire/hire operations (Slice 8) —
  the "never grows, never runs out" invariant.
- `manager_generation_is_deterministic`: same `(world_seed, id)` twice → identical `Manager`
  (name, `identity_bias`).
- `identity_shift_moves_toward_manager_not_past_it`: for any starting `club.tactical_identity`
  and any `manager.identity_bias`, every post-shift `role_weight[r]` lies strictly between the
  pre-shift value and the manager's value (never overshoots) — the "blend not override" bound.
- `identity_shift_preserves_the_sum_invariant`: post-shift `role_weight` sums to
  `NUM_ROLES * 1.0` within the same integer-rounding slack `identity_weights_sum_to_average_one`
  (`tactical_identity.rs:106-116`) already tolerates.

## Slice 8 — Manager performance tracking, firing trigger, pool-based rehire

### 8.1 — Capturing per-match results: a zero-ripple sibling to `batch_tick_season`

Same pattern round-3 Slice 5 used for `generate_player`/`generate_player_biased` — the existing
function keeps its exact signature and output for every current caller; a new sibling carries
the extra data this slice needs.

```rust
pub fn batch_tick_season(
    pop: &mut Population, world: &WorldGenesis, league_clubs: &[Vec<ClubId>],
    world_seed: u64, season: u32, elapsed_weeks: u32,
) -> (Vec<SeasonResult>, Vec<Table>) {
    let (results, tables, _match_points) =
        batch_tick_season_with_match_points(pop, world, league_clubs, world_seed, season, elapsed_weeks);
    (results, tables)
}

/// Identical body to today's `batch_tick_season`, plus: inside the existing per-round loop
/// (`batch_tick.rs:93-97`), immediately after each `table.apply_result(...)`, push this
/// match's points for both sides into the returned `Vec`. No other line changes.
pub fn batch_tick_season_with_match_points(
    pop: &mut Population, world: &WorldGenesis, league_clubs: &[Vec<ClubId>],
    world_seed: u64, season: u32, elapsed_weeks: u32,
) -> (Vec<SeasonResult>, Vec<Table>, Vec<(ClubId, u8)>) {
    // ...identical to today's body...
    let mut match_points = Vec::new();
    // inside the round loop:
    //   let (gf, ga) = sim_team_match(...);
    //   table.apply_result(f.home, f.away, gf, ga);
    //   let (home_pts, away_pts) = points_from_result(gf, ga); // 3/1/0, home/away
    //   match_points.push((f.home, home_pts));
    //   match_points.push((f.away, away_pts));
    // ...rest identical...
    (results, tables, match_points)
}

fn points_from_result(gf: u32, ga: u32) -> (u8, u8) {
    match gf.cmp(&ga) {
        std::cmp::Ordering::Greater => (3, 0),
        std::cmp::Ordering::Equal => (1, 1),
        std::cmp::Ordering::Less => (0, 3),
    }
}
```

The integration slice's `advance_one_season` switches to the `_with_match_points` variant and
feeds the result into `ManagerPool`; every other existing caller/test of `batch_tick_season`
(there are several across `goat-world`'s test suite) is byte-identical, zero test churn outside
the two new tests below.

### 8.2 — Updating manager form: ring-buffer push per match

```rust
impl ManagerPool {
    pub fn record_match_points(&mut self, match_points: &[(ClubId, u8)]) {
        for &(club_id, pts) in match_points {
            let mgr_id = self.club_manager[club_id];
            let mgr = &mut self.managers[mgr_id as usize];
            let idx = mgr.recent_idx as usize;
            mgr.recent_points[idx] = pts;
            mgr.recent_idx = ((idx + 1) % MANAGER_FORM_WINDOW) as u8;
            mgr.matches_played = mgr.matches_played.saturating_add(1);
        }
    }
}
```

The ring buffer is **not** reset at season boundaries — it persists across the season-tick the
same way `Manager` itself does, so a manager's form genuinely spans a rolling 10-match window
regardless of where in the season it falls (matches real "fired mid-slump," not "only
evaluated exactly at year-end"). `matches_played` likewise keeps accumulating across seasons
within the same tenure — it is only ever reset on a fresh hire (8.4), never on a season
boundary.

### 8.3 — Firing trigger: rolling actual points-per-game vs. expected, by strength

```rust
/// Expected points-per-game for a club of this strength, on the standard 3/1/0 scale.
/// Linear: strength 50 (dead average) → 1.5 ppg (a genuine mid-table rate); strength 99 →
/// ~2.24 ppg; strength 1 → ~0.76 ppg. Coefficients are Design's own fit, not derived from any
/// real points-per-strength data (none exists in this codebase) — flagged below.
fn expected_ppg(strength: u8) -> Fixed {
    Fixed::raw(1_500 + (strength as i32 - 50) * 15)
}

fn actual_ppg(recent_points: &[u8; MANAGER_FORM_WINDOW]) -> Fixed {
    let sum: i32 = recent_points.iter().map(|&p| p as i32).sum();
    Fixed::raw(sum * 1_000 / MANAGER_FORM_WINDOW as i32)
}

const FIRING_UNDERPERFORMANCE_PCT: i64 = 70; // actual ppg below 70% of expected → fired

fn should_fire(manager: &Manager, strength: u8, season: u32) -> bool {
    // Grace period: a manager needs a full rolling window of real results before being
    // eligible for dismissal — gated on `matches_played` (Slice 7.1's field), so a brand-new
    // manager with 9 zeroed ring-buffer slots and 1 real bad result never reads as
    // catastrophic underperformance from partial data.
    if (manager.matches_played as usize) < MANAGER_FORM_WINDOW {
        return false;
    }
    let actual = actual_ppg(&manager.recent_points);
    let threshold = expected_ppg(strength) * Fixed::raw(FIRING_UNDERPERFORMANCE_PCT * 10);
    actual < threshold
}
```

### 8.4 — Replacement: draw from the free-agent pool, deterministic

```rust
fn hire_replacement(pool: &mut ManagerPool, club: &mut Club, club_id: ClubId, world_seed: u64, season: u32) {
    let fired = pool.club_manager[club_id];
    pool.free_agents.push(fired);
    pool.managers[fired as usize].recent_points = [0; MANAGER_FORM_WINDOW]; // fresh start
    pool.managers[fired as usize].recent_idx = 0;
    pool.managers[fired as usize].matches_played = 0;

    let seed = manager_seed(world_seed, 0xE4) ^ (club_id as u64) ^ (season as u64);
    let mut rng = GoatRng::new(seed);
    let draw = rng.next_range_u32(0, pool.free_agents.len() as u32 - 1) as usize;
    let hired = pool.free_agents.remove(draw);

    pool.club_manager[club_id] = hired;
    pool.managers[hired as usize].tenure_start_season = season;
    pool.managers[hired as usize].matches_played = 0; // new tenure, fresh grace period
    apply_manager_identity_shift(club, &pool.managers[hired as usize]); // Slice 7.3
}
```

The fired manager going back into `free_agents` (rather than being discarded) is what keeps
`ManagerPool` closed (7.1's invariant) — a sacked manager is available to be hired elsewhere
later, same seed-derived draw mechanism, matching real-world managerial merry-go-rounds without
needing to model reputation/preference on the hiring side (every free agent is equally likely
to be drawn — Design's own simplification, flagged below). Resetting `matches_played` to `0` on
the *hired* manager (not just the fired one) matters just as much: a free agent pulled out of
the pool starts their new tenure's grace period from zero, exactly like a freshly-generated
manager would.

### TDD anchor (Slice 8)

- `match_points_sum_matches_table_points`: `record_match_points`'s total across a full season
  equals the same season's `Table`'s own points column, cross-checked against the existing
  `Table::apply_result` logic — confirms the sibling function's per-match capture is faithful
  to the aggregate it's derived alongside.
- `should_fire_false_before_grace_period_fills`: a manager with `matches_played < MANAGER_FORM_
  WINDOW` and all-zero `recent_points` returns `false` regardless of strength — the direct
  regression for 8.3's grace-period gate.
- `should_fire_true_for_sustained_underperformance`: a manager with `matches_played >=
  MANAGER_FORM_WINDOW` whose `recent_points` are all `0` at a `strength = 80` club (expected
  ~1.95 ppg) returns `true`.
- `should_fire_false_for_matching_or_exceeding_expectation`: a manager past the grace period
  whose `recent_points` average at/above `expected_ppg` for their club's strength returns
  `false`, even at strength extremes (1 and 99).
- `hire_replacement_keeps_pool_closed`: after any fire/hire cycle,
  `managers.len() == club_manager.len() + free_agents.len()` still holds (extends 7's
  invariant test to the fire/hire path specifically).
- `hire_replacement_resets_form_and_grace_period_but_keeps_identity`: the rehired-later fired
  manager's `recent_points` are zeroed and `matches_played == 0` at their new club, but
  `identity_bias` is byte-identical to what it was before firing.
- `identity_shift_applies_on_every_hire_not_just_genesis`: a club's `tactical_identity` after
  `hire_replacement` differs from its pre-hire value whenever the new manager's `identity_bias`
  differs from the old one.

## Out of scope (this file)

- Club budgets, transfers, youth-academy investment — other sibling files; this file's managers
  don't read or affect `Club.budget`.
- **Manager reputation/preference-based hiring** — every free agent is equally likely to be
  drawn (8.4); no "bigger clubs attract better/more famous managers" weighting.
- Season-tick wiring (when `should_fire`/`hire_replacement` actually get called each season) —
  the integration slice's work; this file only builds the mechanism.

## Decisions Design made as judgment calls — flag for Tùng's explicit sign-off

11. **Slice 7.1**: `MANAGER_POOL_SIZE = 300` (`NUM_CLUBS / 4`) and `MANAGER_FORM_WINDOW = 10`
    — both Design's own picks; Tùng's brief left pool size and window length as open "you
    decide" items explicitly.
12. **Slice 7.2**: salt byte `0xD3` for `manager_seed` — Design's own next-unused-byte guess,
    **not exhaustively verified against every salt constant in the codebase** (only `world.rs`'s
    two were checked). Dev must grep before landing this.
13. **Slice 7.3**: `MANAGER_IDENTITY_BLEND_PCT = 700` (70% new manager / 30% club legacy) — a
    concrete answer to "shift the tactical identity," Design's own split.
14. **Slice 8.3**: the entire `expected_ppg`/`should_fire` firing trigger — `expected_ppg`'s
    linear coefficients, and `FIRING_UNDERPERFORMANCE_PCT = 70` — is Design's concrete
    instantiation of Tùng's explicit "you decide and justify a concrete formula" instruction.
    (The `matches_played` grace-period gap Design found while first drafting this trigger has
    already been folded into Slice 7.1's struct shape above, as part of this file's split —
    Dev does not need to modify Slice 7's struct after the fact.)

(Numbering preserved from the original doc's full "Decisions" list; item 15 lives in the
integration slice.) These are first-pass numbers for a later `TASK-TUNE` pass once playtested —
not blocking items Tùng needs to approve before Dev starts.

## Definition of done (Slice 7-8)

1. `cargo test --workspace` green, including every TDD-anchor test listed above.
2. `goat-save::save::VERSION` bumped (if not already bumped by a landed sibling slice) to cover
   the new `ManagerPool` (three fields) and `Manager` (including `matches_played`), serialized/
   deserialized and round-tripped (`crates/goat-save/tests/save_roundtrip.rs` extended,
   mirroring how round-3 flagged `intake_week` for the same treatment).
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
6. **Commit this slice before starting `-slice9-integration.md`.**
