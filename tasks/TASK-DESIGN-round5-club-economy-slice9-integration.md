# TASK DESIGN ROUND 5, SLICE 9 — Season-tick wiring: where the club economy hooks into `advance_one_season`

**Split-file note (read this first):** this file is 1 of 6 that together replace
`tasks/TASK-DESIGN-round5-club-economy.md` (now a short pointer doc). Sibling files:
`-slice1-2-foundation.md`, `-slice3-4-scouting.md`, `-slice5-transfers.md`,
`-slice6-academy.md`, `-slice7-8-managers.md`. This file is fully self-contained — implement it
without reading the others or the original doc, **but it is the integration pass and needs all
5 sibling files landed and committed first** (hard prereq) — it calls `open_transfer_window`,
`run_transfer_pass`, `run_academy_investment_pass`/`apply_academy_investment`+`decay_academy_
boost`, `should_fire`/`hire_replacement`, and `batch_tick_season_with_match_points`, all defined
across those 5 files.

Read first: `crates/goat-world/src/promotion.rs:137-149` (`ReplayCache::advance_one_season`,
the season-tick entry point every prior round's new season-boundary mechanic hooks into, and
this file's too); `crates/goat-core/src/calendar_loop.rs:32-56` (`standard_season()`, the
already-shipped `TransferWinter`/`TransferSummer` window layout this file's cadence borrows the
*name and count* from, not the day-level machinery); `tasks/TASK-DESIGN-round3-player-driven-
club-strength.md` §4.6 (`apply_youth_intake`, the existing call this file's wiring runs
alongside, untouched).

## Ground rules for this file

- **Season-granularity architecture stays intact — two transfer-window *passes* per
  season-tick, not real calendar-day events for background clubs.** `calendar_loop.rs`'s
  `TransferWinter`/`TransferSummer` windows already exist for the **orbit** (PC-facing,
  day-tick) calendar, but `batch_tick_season` (the background-league path this market lives in)
  simulates a whole season's matches in one call, with no sub-season day resolution. Splitting
  that call into two match-halves so a literal mid-season window could interleave is a bigger
  structural change than this round asked for. **Design's call, flagged for sign-off (see
  "Decisions"):** both windows run as two back-to-back passes at the same season-tick boundary
  (winter pass, then the season's matches, then summer pass, then manager evaluation) — see 9.1
  below for the exact ordering and why.
- **Every invented ordering decision is flagged** — see "Decisions" below.

## Verified: grounding for this file

- **Two transfer windows per year already exist — but only on the PC-facing orbit calendar.**
  `crates/goat-core/src/calendar_loop.rs:32-56`'s `standard_season()`: `TransferWinter` (days
  160–195, 35 days) and `TransferSummer` (days 330–364, wrapping the season boundary, 34 days),
  inside a 365-day season (`SEASON_DAYS`). This confirms "two windows/year" is already the
  game's calendar shape — this file's wiring borrows the *name and count* (winter, summer) but
  not the day-level machinery.
- **The season-tick hook point every prior round used is `ReplayCache::advance_one_season`**
  (`promotion.rs:137-149`): calls `batch_tick_season` (all of one season's matches), then
  `apply_season_end` (promotion/relegation). Round-3 Slice 4 (youth intake) already established
  the pattern of adding a new call inside this function. This file extends the same function
  further.
- **`world.clubs` gains `&mut` access it didn't need before** (budgets/academy_boost/
  tactical_identity all mutate now) — `WorldGenesis` today is treated as effectively immutable
  after genesis by every existing caller (`ReplayCache::advance_one_season`'s current signature
  takes `&WorldGenesis`, not `&mut`); this is a real, load-bearing signature change to
  `ReplayCache::advance_one_season` and every caller of it (`promotion.rs`'s own tests, and
  whatever orbit-path code the round-3 Slice 3.4 migration note anticipated) — flagged below as
  a wider-than-usual ripple for Dev to scope carefully, not a small change.

## Slice 9 — Season-tick wiring

### 9.1 — The wiring itself

```rust
pub fn advance_one_season(&mut self, world: &mut WorldGenesis) -> Vec<PromoRelegationEvent> {
    let season = self.resolved_through + 1;
    let elapsed_weeks = season * 52;

    // 1. Winter window: budgets top up, then both buy-lanes run, then youth investment.
    for club in &mut world.clubs {
        open_transfer_window(club, &self.pop, &squad_of(&self.pop, club.id), club.tier(world), elapsed_weeks);
    }
    run_transfer_pass(&mut self.pop, world, self.world_seed, season, 0, TransferLane::WeakestPosition);
    run_transfer_pass(&mut self.pop, world, self.world_seed, season, 0, TransferLane::GemHunt);
    run_academy_investment_pass(&mut self.pop, world, self.world_seed, season, 0); // Slice 6

    // 2. The season's matches — now captures per-match points for manager form (Slice 8.1).
    let (_results, tables, match_points) = batch_tick_season_with_match_points(
        &mut self.pop, world, &self.membership, self.world_seed, season, elapsed_weeks,
    );
    self.managers.record_match_points(&match_points);

    // 3. Summer window: same three passes again, off the post-season-matches budget state.
    for club in &mut world.clubs {
        open_transfer_window(club, &self.pop, &squad_of(&self.pop, club.id), club.tier(world), elapsed_weeks);
    }
    run_transfer_pass(&mut self.pop, world, self.world_seed, season, 1, TransferLane::WeakestPosition);
    run_transfer_pass(&mut self.pop, world, self.world_seed, season, 1, TransferLane::GemHunt);
    run_academy_investment_pass(&mut self.pop, world, self.world_seed, season, 1);
    for club in &mut world.clubs {
        decay_academy_boost(club); // once per season, after both windows (Slice 6.3)
    }

    // 4. Manager evaluation — after a full season of form data, before next season's roster
    //    churn (round-3 Slice 4's youth intake) so a freshly-fired club's replacement manager
    //    is in place before intake reads `club.tactical_identity` at all (it doesn't today,
    //    but this ordering is future-proof against that changing).
    for club_id in 0..world.clubs.len() {
        let mgr_id = self.managers.club_manager[club_id];
        if should_fire(&self.managers.managers[mgr_id as usize], world.clubs[club_id].strength, season) {
            hire_replacement(&mut self.managers, &mut world.clubs[club_id], club_id, self.world_seed, season);
        }
    }

    // 5. Existing round-3/round-2 machinery, untouched.
    apply_youth_intake(&mut self.pop, world, self.world_seed, season); // round-3 §4.6
    let events = apply_season_end(world, &mut self.membership, season, &tables);
    self.resolved_through = season;
    events
}
```

`run_academy_investment_pass(pop, world, world_seed, season, window)` is a thin wrapper this
slice adds: for each club, `apply_academy_investment(club, lane_cap(club.budget, LANE_YOUTH_
INVESTMENT_PCT))` (transfers slice §5.1's `lane_cap`, academy slice §6.2's
`apply_academy_investment`) — no target search, just the always-invest-full-cap policy academy
slice §6.4 specifies. `squad_of(pop, club_id) -> Vec<usize>` and `ReplayCache.managers:
ManagerPool` (threaded next to `self.pop`, initialized via `ManagerPool::genesis` at world
creation) are this slice's own small additions to `ReplayCache`, alongside the signature change
below.

**Why this exact ordering, spelled out**: budgets must top up before any spending pass reads
them (1 before the buy-lanes); the season's matches must run before manager form can be scored
off them (2 before 4); both transfer windows should bracket the season's matches, not both run
back-to-back before any match is played, so a club's summer-window spending reflects what its
squad actually did that season, not just its winter-window shape (1 and 3 on either side of 2);
manager evaluation happens once per season, after the summer window (not per-window), since
firing a manager mid-transfer-window would create an ordering dependency between two systems
this doc otherwise keeps independent. This whole ordering is **Design's own construction** —
Tùng specified the pieces (two windows, budget, auction, manager pressure) but not their
relative sequencing within one season-tick — flagged below.

### 9.2 — The signature-change ripple

`ReplayCache::advance_one_season`'s signature changes from `&WorldGenesis` to `&mut
WorldGenesis` — every existing caller of it must be updated (`promotion.rs`'s own tests at
minimum, and whatever orbit-path code the round-3 Slice 3.4 migration note anticipated). Dev
greps for all call sites before landing this; this file does not enumerate every one, since it
was not exhaustively audited against the whole codebase.

### TDD anchor (Slice 9)

- `full_season_tick_is_deterministic`: two identical `(world_seed, season)` runs through
  `advance_one_season` produce byte-identical `world.clubs` (`budget`, `academy_boost`,
  `tactical_identity`), `self.pop.club` assignments, and `self.managers` state — the
  end-to-end regression tying every prior slice together.
- `total_system_budget_change_equals_total_income_minus_total_wages`: summing every club's
  `budget` delta across one full season-tick (both windows) equals the sum of every club's
  `total_income` minus `window_wage_deduction` minus academy-investment spend — transfer fees
  net to zero system-wide (the transfers slice's conservation invariant, now checked at the
  whole-season scale, not just per-transfer).
- `manager_firing_reflects_the_full_season_not_just_one_window`: a manager whose form is bad
  in the winter half but recovers by the summer window is evaluated once, at season-end, off
  the full rolling window — not fired mid-season by this file's own machinery (there is no
  mid-season fire path; confirms step 4's "once per season" claim).

## Out of scope (this file)

- **PC-facing transfer-market participation** — the PC's own club is not yet a bidder in this
  market. Phase 8 (`state.rs`) stays exactly as-is.
- **PC Reputation as a factor in which clubs show transfer interest** — approved by Tùng
  (2026-07-22, "Ok" at the point where this was proposed) but not designed in this round;
  flagged here so the decision isn't lost. Tùng's framing: high Reputation (bible §8.2,
  `crates/goat-meta/src/reputation.rs`) should make big clubs notice/pursue the PC independent
  of raw attribute numbers, "ngon hơn FC career mode" (better than how EA FC's career mode does
  it, which is stats-only). The natural hook, once PC-facing transfer participation is
  designed, is to weight offer likelihood/quality by Reputation alongside OVR — not designed
  here, just recorded as a real requirement for whichever future round designs PC-facing
  transfer offers.
- **Sub-season (day-level) transfer window integration with the orbit calendar** — this file's
  two windows are season-tick passes, not literal calendar-day events for background clubs;
  wiring the *orbit* (PC-facing) calendar's already-existing `TransferWinter`/`TransferSummer`
  flashpoints (`calendar_loop.rs`) to actually open/close this market in lockstep with day-level
  time is a future integration, not designed here.
- **Round-4 competition-result feedback into finances or manager pressure** — two composition
  points are noted but not designed: (1) `total_income`'s future `prize_money_income`
  contributor naturally reads round-4's `Competition`/results machinery once that lands; (2)
  `expected_ppg`'s "expected performance for that club's tier/strength" could, in a future
  round, be raised for a club in continental-qualification or promotion-relegation contention
  (round-4's `FixtureImportance`, round-2's `PROMO_RELEGATION_N = 3`) rather than
  uniform-by-strength as it is today. Neither is built here — flagged as interaction points,
  not redesigns.
- **Round-2/round-3/round-4 decisions themselves** — not re-litigated. This file reads
  `Club.strength`, `TacticalIdentity`, `PROMO_RELEGATION_N`, and the youth-intake formula as
  fixed inputs.

## Decisions Design made as judgment calls — flag for Tùng's explicit sign-off

15. **Slice 9**: the entire season-tick ordering (windows bracket the season's matches;
    manager evaluation once, after both windows) and the `ReplayCache::advance_one_season`
    signature change from `&WorldGenesis` to `&mut WorldGenesis` — a real, wider-than-usual
    ripple Design flagged explicitly rather than glossing over; every existing caller of
    `advance_one_season` needs updating, not just this file's own new code.

(Numbering preserved from the original doc's full "Decisions" list; items 1-14 live in sibling
files.) This is a first-pass ordering for a later `TASK-TUNE`/review pass once playtested — not
a blocking item Tùng needs to approve before Dev starts, same framing round-3's own judgment
calls used.

## Definition of done (Slice 9 — and the whole round)

1. `cargo test --workspace` green, including every TDD-anchor test listed above and every TDD
   anchor from all 5 sibling files (this is the integration pass — nothing upstream should have
   regressed).
2. `goat-save::save::VERSION` bumped past `10`, with **every** new persisted field across the
   whole round (`Club.budget`, `Club.academy_boost`, `ManagerPool`'s three fields,
   `Manager.matches_played`) serialized/deserialized and round-tripped
   (`crates/goat-save/tests/save_roundtrip.rs` extended, mirroring how round-3 flagged
   `intake_week` for the same treatment) — if a sibling slice already bumped the version, this
   slice confirms the full set is covered, not just its own additions.
3. `ReplayCache::advance_one_season`'s signature change (`&WorldGenesis` → `&mut WorldGenesis`)
   is propagated to every existing caller — `promotion.rs`'s own tests at minimum; Dev greps
   for all call sites before landing, since this file did not enumerate every one.
4. A `--release` benchmark of one full season-tick (both windows + both transfer lanes + youth
   investment + manager evaluation, across all 1,200 clubs) is taken once implemented, to
   validate the scouting slice's `TARGET_SEARCH_PREFIX = 50` bound and the general
   O(population) per-window cost this round's "Verified" sections reasoned about but did not
   measure — unmeasured/growing per-season cost is exactly the same category of open item
   round-2's A2.5 flagged for genesis/replay time.
5. At least one integration test plays a fixed seed through several seasons and asserts:
   (a) at least one club's manager is fired and replaced (exercises the full Slice 8 path
   end-to-end, not just unit-level `should_fire`); (b) at least one contested auction resolves
   with `fee > valuation` (exercises Slice 5's competitive-bidding claim, not just the
   uncontested path); (c) at least one gem-hunting target is a round-3-outlier-style player
   (ties Slice 4 back to round-3 Slice 2's mechanic, the same style of cross-slice integration
   assertion round-3's own Definition of Done used).
6. `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` clean.
7. No new failures beyond the **10 pre-existing `goat-tui` `smoke_stdin` failures** (verified
   2026-07-22, out of scope, unrelated to this work, caused by a `generate_club_name()` bug in
   `crates/goat-world/src/world.rs`): `confirm_screen_blank_enter_reprompts_instead_of_
   discarding_character`, `double_w_in_same_round_shows_message_not_silent_noop`,
   `game_sheet_and_player_sheet_boxes_close_for_short_and_long_club_names`, `key_moments_lines_
   close_with_ellipsis_not_ragged_cutoff`, `legacy_screen_notes_mid_season_batching`,
   `main_loop_unrecognized_command_messages_and_continues`, `player_sheet_explains_ovr_is_
   position_weighted`, `save_overwrite_requires_explicit_confirmation`, `save_to_empty_slot_
   succeeds_without_confirmation`, `status_header_shows_energy_percent_and_labeled_discipline_
   count`.
8. **Commit this slice.** This is the last file in the round-5 split — landing it closes out
   backlog item #3 in full.
