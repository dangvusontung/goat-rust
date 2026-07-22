//! Promotion & relegation between tiers (TASK-DESIGN-round2 A3), and — since Design round 5,
//! Slice 9 — the whole club-economy season-tick wiring (budgets, transfers, academy
//! investment, manager evaluation) that hooks into the same season boundary.
//!
//! Design: **replay-from-seed, zero new persisted state** (A3.1's recommended fork) —
//! league membership for season N is derived by replaying promotion/relegation for
//! seasons 1..N from genesis-static membership, purely from `(world_seed, season)`, the
//! same "seed is the universe, recompute the rest" pattern `History` already uses. An
//! in-memory `ReplayCache` (below) avoids re-replaying from season 1 on every call within
//! a session by extending incrementally as `season_number` advances (A3.1 point 2).
//!
//! Round 5's economy fields break the "zero new persisted state" purity a little:
//! `Club.budget`/`Club.academy_boost` are genuinely path-dependent (accumulated income
//! minus wages/spend across seasons) and `WorldGenesis` is regenerated fresh from
//! `world_seed` on every load, so those two `Club` fields only ever hold their genesis
//! *starting* values (see their doc comments in `world.rs`) — the real numbers live in
//! `ReplayCache::club_budgets`/`academy_boosts` below, this cache's own in-memory mirror of
//! what `goat_core::state::WorldState::club_budgets`/`academy_boosts` persists at the save
//! boundary. `ManagerPool` is the same story: `ManagerPool::genesis` is a pure function of
//! `world_seed` (identity/bias only), but *who* manages *which* club, current form, and
//! fire/rehire history are path-dependent and persisted (`goat-save` v14) exactly like the
//! budget fields — `ReplayCache::managers` is this slice's in-memory mirror of that.

use crate::academy::{decay_academy_boost, run_academy_investment_pass};
use crate::batch_tick::batch_tick_season_with_match_points;
use crate::economy::open_transfer_window;
use crate::manager::{hire_replacement, should_fire, Manager, ManagerId, ManagerPool};
use crate::population::{apply_youth_intake, Population};
use crate::season::Table;
use crate::transfers::{run_transfer_pass_with_log, TransferLane, TransferLogEntry};
use crate::world::{ClubId, LeagueId, WorldGenesis, PROMO_RELEGATION_N};

/// The per-club outcome of a season boundary — deliberately not a `bool` (A3.2's
/// refinement): a typed enum leaves room for a later round to add variants (playoff
/// promotion, administrative relegation, ...) without rewriting every call site that
/// currently pattern-matches a bool into an if/else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionType {
    DirectPromotion,
    DirectRelegation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromoRelegationEvent {
    pub club: ClubId,
    pub season: u32,
    pub from_league: LeagueId,
    pub to_league: LeagueId,
    pub transition: TransitionType,
}

/// Apply one season's promotion/relegation given that season's final tables (one `Table`
/// per league, indexed the same way as `world.leagues`). Mutates `membership` in place and
/// returns the ordered list of events. Top-`PROMO_RELEGATION_N` of a tier rise into the
/// tier above (within the same nation); bottom-N drop into the tier below. The top tier has
/// no promotion into it and the bottom tier has no relegation out of it — both are simply
/// no-ops at the pyramid's edges (only *adjacent*-tier pairs within a nation are visited).
pub fn apply_season_end(
    world: &WorldGenesis,
    membership: &mut [Vec<ClubId>],
    season: u32,
    tables: &[Table],
) -> Vec<PromoRelegationEvent> {
    let mut events = Vec::new();

    for nation in &world.nations {
        let mut nation_leagues: Vec<&crate::world::League> = world
            .leagues
            .iter()
            .filter(|l| l.nation == nation.id)
            .collect();
        nation_leagues.sort_by_key(|l| l.tier as usize);

        for pair in nation_leagues.windows(2) {
            let upper = pair[0];
            let lower = pair[1];

            let upper_sorted = tables[upper.id].sorted();
            let lower_sorted = tables[lower.id].sorted();

            let relegated: Vec<ClubId> = upper_sorted
                .iter()
                .rev()
                .take(PROMO_RELEGATION_N)
                .map(|e| e.club_id)
                .collect();
            let promoted: Vec<ClubId> = lower_sorted
                .iter()
                .take(PROMO_RELEGATION_N)
                .map(|e| e.club_id)
                .collect();

            for &club in &relegated {
                membership[upper.id].retain(|&c| c != club);
                membership[lower.id].push(club);
                events.push(PromoRelegationEvent {
                    club,
                    season,
                    from_league: upper.id,
                    to_league: lower.id,
                    transition: TransitionType::DirectRelegation,
                });
            }
            for &club in &promoted {
                membership[lower.id].retain(|&c| c != club);
                membership[upper.id].push(club);
                events.push(PromoRelegationEvent {
                    club,
                    season,
                    from_league: lower.id,
                    to_league: upper.id,
                    transition: TransitionType::DirectPromotion,
                });
            }
        }
    }

    events
}

/// `club_id -> league_id` for the current membership — built once per season-tick (stable
/// for the whole tick; only `apply_season_end` at the very end moves clubs between
/// leagues) rather than linear-scanning `membership` per club, per window, per club-economy
/// slice's own `squads_by_club` idiom (`transfers.rs`, `batch_tick.rs`) for the same reason:
/// O(clubs) once beats O(clubs × leagues) repeated.
fn league_of_club(membership: &[Vec<ClubId>], num_clubs: usize) -> Vec<LeagueId> {
    let mut league_of = vec![0usize; num_clubs];
    for (league_id, clubs) in membership.iter().enumerate() {
        for &club_id in clubs {
            league_of[club_id] = league_id;
        }
    }
    league_of
}

/// `club_id -> Vec<population index>` for the whole population, one pass — this module's
/// own copy of the same tiny helper `batch_tick.rs`/`transfers.rs` each already keep
/// privately (no shared "squads by club" util exists in this crate yet, per those modules'
/// own doc comments).
fn squads_by_club(pop: &Population, num_clubs: usize) -> Vec<Vec<usize>> {
    let mut squads: Vec<Vec<usize>> = vec![Vec::new(); num_clubs];
    for idx in 0..pop.len() {
        squads[pop.club[idx] as usize].push(idx);
    }
    squads
}

/// One transfer window's budget top-up for every club (§9.1 step 1/3): income in, wages
/// out, off each club's *current* tier (looked up via `league_of`, not a genesis-static
/// one — a promoted club earns its new tier's income from the very next window). Squads
/// are rebuilt fresh each call since the prior window's transfer passes can have moved
/// players between clubs.
fn open_transfer_windows(
    world: &mut WorldGenesis,
    pop: &Population,
    league_of: &[LeagueId],
    elapsed_weeks: u32,
) {
    let squads = squads_by_club(pop, world.clubs.len());
    for club_id in 0..world.clubs.len() {
        let tier = world.leagues[league_of[club_id]].tier;
        open_transfer_window(
            &mut world.clubs[club_id],
            pop,
            &squads[club_id],
            tier,
            elapsed_weeks,
        );
    }
}

/// In-memory (never persisted as a whole) replay cache: holds league membership and the
/// events that produced it, as of the highest season fully resolved so far. Extending it
/// one season at a time (rather than replaying from season 1 every call) is what keeps
/// per-season cost bounded as a career gets long (A3.1 point 2's flagged perf concern).
///
/// Since Design round 5 (Slice 9), also holds every other piece of path-dependent
/// club-economy state a season-tick threads through: the population (career accumulators,
/// club assignments), club budgets/academy boosts (this cache's mirror of
/// `WorldState::club_budgets`/`academy_boosts`), and the manager pool (this cache's mirror
/// of `WorldState::managers`/`club_manager`/`free_agents`).
pub struct ReplayCache {
    world_seed: u64,
    /// Highest season whose promotion/relegation has been resolved into `membership`.
    /// `0` means genesis-static membership (no season played yet).
    resolved_through: u32,
    membership: Vec<Vec<ClubId>>,
    /// Population reused across the whole replay — genesis identity/potential columns are
    /// pure functions of `world_seed`, so one `Population` serves the entire cache lifetime.
    pop: Population,
    /// Every club's running transfer/wage war-chest, £k, indexed by `ClubId` — the real,
    /// path-dependent value; `world.clubs[i].budget` only ever holds the genesis starting
    /// point outside of the brief window `advance_one_season` overlays this onto it (module
    /// doc). Seeded from that same starting point at `new()`; overridden by
    /// `overlay_persisted` when resuming a save.
    club_budgets: Vec<i64>,
    /// Every club's academy-boost lever, same "genesis starting point vs. real
    /// path-dependent value" story as `club_budgets`, indexed by `ClubId`.
    academy_boosts: Vec<u8>,
    /// Manager pool: identities are seed-derived (`ManagerPool::genesis`), but assignment,
    /// form, and fire/rehire history are path-dependent — this is the session's live
    /// mirror of `WorldState`'s persisted manager fields, same role `club_budgets` plays
    /// for `Club::budget`.
    managers: ManagerPool,
    /// Every transfer this cache's most recent `advance_one_season` call executed (both
    /// windows, both lanes), most recent call replacing the previous — telemetry/test
    /// observability, not consumed by the tick itself.
    last_transfers: Vec<TransferLogEntry>,
}

impl ReplayCache {
    pub fn new(world: &WorldGenesis, world_seed: u64) -> Self {
        Self {
            world_seed,
            resolved_through: 0,
            membership: world.static_league_clubs(),
            pop: crate::population::genesis(world_seed, world),
            club_budgets: world.clubs.iter().map(|c| c.budget).collect(),
            academy_boosts: world.clubs.iter().map(|c| c.academy_boost).collect(),
            managers: ManagerPool::genesis(world_seed, world),
            last_transfers: Vec::new(),
        }
    }

    /// Overlay path-dependent state loaded from an existing save
    /// (`WorldState::club_budgets`/`academy_boosts`/manager-pool fields) onto a
    /// freshly-`new()`-constructed cache, for resuming a career rather than starting one.
    /// Any argument left empty (an old/short save that predates that field, `goat-save`'s
    /// own `old_v11_save_without_club_budgets_defaults_to_empty`-style migration story)
    /// leaves this cache's genesis-derived default in place rather than clobbering it with
    /// nothing.
    pub fn overlay_persisted(
        &mut self,
        club_budgets: Vec<i64>,
        academy_boosts: Vec<u8>,
        managers: Vec<Manager>,
        club_manager: Vec<ManagerId>,
        free_agents: Vec<ManagerId>,
    ) {
        if !club_budgets.is_empty() {
            self.club_budgets = club_budgets;
        }
        if !academy_boosts.is_empty() {
            self.academy_boosts = academy_boosts;
        }
        if !managers.is_empty() {
            self.managers = ManagerPool {
                managers,
                club_manager,
                free_agents,
            };
        }
    }

    pub fn club_budgets(&self) -> &[i64] {
        &self.club_budgets
    }

    pub fn academy_boosts(&self) -> &[u8] {
        &self.academy_boosts
    }

    pub fn managers(&self) -> &ManagerPool {
        &self.managers
    }

    pub fn pop(&self) -> &Population {
        &self.pop
    }

    /// Every transfer the most recent `advance_one_season` call executed.
    pub fn last_transfers(&self) -> &[TransferLogEntry] {
        &self.last_transfers
    }

    /// League membership to use for `season` (i.e. as of the most recently resolved
    /// promotion/relegation *before* `season` kicks off). Extends the cache one season at a
    /// time until `season - 1` is resolved.
    pub fn membership_for_season(
        &mut self,
        world: &mut WorldGenesis,
        season: u32,
    ) -> &[Vec<ClubId>] {
        while self.resolved_through + 1 < season {
            self.advance_one_season(world);
        }
        &self.membership
    }

    /// Resolve one season end-to-end: promotion/relegation, and — since Design round 5,
    /// Slice 9 — the whole club-economy season tick (Doc A §9.1's ordering, this function's
    /// own construction, flagged for sign-off — see the task doc's "Decisions" §15):
    ///
    /// 1. Winter transfer window: budgets top up, then both buy-lanes run, then youth
    ///    investment — budgets must top up before any spending pass reads them.
    /// 2. The season's matches, capturing per-match points for manager form.
    /// 3. Summer transfer window: the same three passes again, off the post-match budget
    ///    state, so a club's summer spending reflects what its squad actually did this
    ///    season. Academy boost decays once, after both windows have had a chance to
    ///    reinvest.
    /// 4. Manager evaluation, once per season (not per-window) — after a full season of
    ///    form data, before next season's roster churn.
    /// 5. Existing round-3/round-2 machinery (youth intake, season-end promotion/
    ///    relegation), untouched.
    ///
    /// `world` gains `&mut` access it didn't need before this slice — budgets/academy_boost/
    /// tactical_identity all mutate now (module doc's "wider-than-usual ripple", propagated
    /// to every caller by this slice).
    pub fn advance_one_season(&mut self, world: &mut WorldGenesis) -> Vec<PromoRelegationEvent> {
        let season = self.resolved_through + 1;
        let elapsed_weeks = season * 52;

        // Overlay this cache's own path-dependent budget/academy state onto `world.clubs`
        // for the duration of this call (module doc: every existing Club-mutating function
        // in this round takes `&mut Club`, so this in/out overlay is the cheapest correct
        // fix, rather than rewriting five functions' signatures to thread bare `i64`/`u8`
        // refs through instead).
        for (i, club) in world.clubs.iter_mut().enumerate() {
            club.budget = self.club_budgets[i];
            club.academy_boost = self.academy_boosts[i];
        }

        let league_of = league_of_club(&self.membership, world.clubs.len());
        let mut transfers = Vec::new();

        // 1. Winter window: budgets top up, then both buy-lanes run, then youth investment.
        open_transfer_windows(world, &self.pop, &league_of, elapsed_weeks);
        transfers.extend(run_transfer_pass_with_log(
            &mut self.pop,
            world,
            self.world_seed,
            season,
            0,
            TransferLane::WeakestPosition,
        ));
        transfers.extend(run_transfer_pass_with_log(
            &mut self.pop,
            world,
            self.world_seed,
            season,
            0,
            TransferLane::GemHunt,
        ));
        run_academy_investment_pass(world); // Slice 6 §6.4's always-invest-the-cap policy

        // 2. The season's matches — captures per-match points for manager form (Slice 8.1).
        let (_results, tables, match_points) = batch_tick_season_with_match_points(
            &mut self.pop,
            world,
            &self.membership,
            self.world_seed,
            season,
            elapsed_weeks,
        );
        self.managers.record_match_points(&match_points);

        // 3. Summer window: same three passes again, off the post-season-matches budget
        // state.
        open_transfer_windows(world, &self.pop, &league_of, elapsed_weeks);
        transfers.extend(run_transfer_pass_with_log(
            &mut self.pop,
            world,
            self.world_seed,
            season,
            1,
            TransferLane::WeakestPosition,
        ));
        transfers.extend(run_transfer_pass_with_log(
            &mut self.pop,
            world,
            self.world_seed,
            season,
            1,
            TransferLane::GemHunt,
        ));
        run_academy_investment_pass(world);
        for club in &mut world.clubs {
            decay_academy_boost(club); // once per season, after both windows (Slice 6.3)
        }
        self.last_transfers = transfers;

        // 4. Manager evaluation — after a full season of form data, before next season's
        //    roster churn (round-3 Slice 4's youth intake) so a freshly-fired club's
        //    replacement manager is in place before intake reads `club.tactical_identity`
        //    at all (it doesn't today, but this ordering is future-proof against that
        //    changing).
        for club_id in 0..world.clubs.len() {
            let mgr_id = self.managers.club_manager[club_id];
            if should_fire(
                &self.managers.managers[mgr_id as usize],
                world.clubs[club_id].strength,
                season,
            ) {
                hire_replacement(
                    &mut self.managers,
                    &mut world.clubs[club_id],
                    club_id,
                    self.world_seed,
                    season,
                );
            }
        }

        // 5. Existing round-3/round-2 machinery, untouched.
        apply_youth_intake(&mut self.pop, world, self.world_seed, season); // round-3 §4.6
        let events = apply_season_end(world, &mut self.membership, season, &tables);
        self.resolved_through = season;

        // Sync the mutated Club fields back into this cache's own persisted-shape mirror —
        // `WorldState::club_budgets`/`academy_boosts` round-trip *this*, not WorldGenesis's
        // dead-end genesis-only starting fields.
        for (i, club) in world.clubs.iter().enumerate() {
            self.club_budgets[i] = club.budget;
            self.academy_boosts[i] = club.academy_boost;
        }

        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::WorldGenesis;

    #[test]
    fn promotion_and_relegation_are_symmetric_counts() {
        let mut world = WorldGenesis::generate(5);
        let mut cache = ReplayCache::new(&world, 5);
        let events = cache.advance_one_season(&mut world);
        let promotions = events
            .iter()
            .filter(|e| e.transition == TransitionType::DirectPromotion)
            .count();
        let relegations = events
            .iter()
            .filter(|e| e.transition == TransitionType::DirectRelegation)
            .count();
        assert_eq!(
            promotions, relegations,
            "every promotion pairs with a relegation"
        );
        assert!(promotions > 0, "some tier boundary must have moved clubs");
    }

    #[test]
    fn top_tier_never_relegated_from_above_bottom_tier_never_promoted_from_below() {
        let mut world = WorldGenesis::generate(5);
        let mut cache = ReplayCache::new(&world, 5);
        let events = cache.advance_one_season(&mut world);
        for e in &events {
            let from = &world.leagues[e.from_league];
            let to = &world.leagues[e.to_league];
            assert_eq!(from.nation, to.nation, "transitions stay within a nation");
            assert_ne!(from.tier, to.tier);
        }
    }

    #[test]
    fn league_sizes_stay_uniform_after_transitions() {
        let mut world = WorldGenesis::generate(9);
        let mut cache = ReplayCache::new(&world, 9);
        cache.advance_one_season(&mut world);
        cache.advance_one_season(&mut world);
        let membership = cache.membership_for_season(&mut world, 3);
        for (id, clubs) in membership.iter().enumerate() {
            assert_eq!(
                clubs.len(),
                world.leagues[id].max_clubs as usize,
                "league {id} size must stay uniform after transitions"
            );
        }
    }

    #[test]
    fn replay_is_deterministic() {
        let mut world = WorldGenesis::generate(21);
        let mut a = ReplayCache::new(&world, 21);
        let mut b = ReplayCache::new(&world, 21);
        for _ in 0..3 {
            let ea = a.advance_one_season(&mut world);
            let eb = b.advance_one_season(&mut world);
            assert_eq!(ea, eb);
        }
    }

    #[test]
    fn idempotent_view_does_not_double_apply() {
        // Viewing the same season's resolution twice (without advancing) must be a no-op —
        // membership_for_season only replays up to `season - 1`, never re-applies a season
        // already resolved.
        let mut world = WorldGenesis::generate(13);
        let mut cache = ReplayCache::new(&world, 13);
        cache.advance_one_season(&mut world);
        let first = cache.membership_for_season(&mut world, 2).to_vec();
        let second = cache.membership_for_season(&mut world, 2).to_vec();
        assert_eq!(first, second);
    }

    // ── Slice 9 TDD anchors ──────────────────────────────────────────────────────

    #[test]
    fn full_season_tick_is_deterministic() {
        let world_seed = 555;
        let mut world_a = WorldGenesis::generate(world_seed);
        let mut world_b = WorldGenesis::generate(world_seed);
        let mut cache_a = ReplayCache::new(&world_a, world_seed);
        let mut cache_b = ReplayCache::new(&world_b, world_seed);

        cache_a.advance_one_season(&mut world_a);
        cache_b.advance_one_season(&mut world_b);

        assert_eq!(
            world_a.clubs, world_b.clubs,
            "budget/academy_boost/tactical_identity must be byte-identical across two \
             identical (world_seed, season) runs"
        );
        assert_eq!(cache_a.club_budgets(), cache_b.club_budgets());
        assert_eq!(cache_a.academy_boosts(), cache_b.academy_boosts());
        assert_eq!(
            cache_a.pop().club,
            cache_b.pop().club,
            "self.pop.club assignments must be byte-identical"
        );
        assert_eq!(
            cache_a.managers().club_manager,
            cache_b.managers().club_manager
        );
        assert_eq!(
            cache_a.managers().free_agents,
            cache_b.managers().free_agents
        );
        assert_eq!(cache_a.managers().managers, cache_b.managers().managers);
    }

    #[test]
    fn total_system_budget_change_equals_total_income_minus_total_wages() {
        // Verified against `advance_one_season`'s own sequencing (§9.1), calling the exact
        // same production functions in the exact same order on one live `(world, pop)`
        // pair, so intermediate checkpoints can be captured directly — the alternative
        // (predicting a shadow run's expected budgets up front) breaks the moment a
        // transfer moves a player between clubs, since that changes the *next* window's
        // wage bill via squad composition. The two transfer-pass checkpoints below are the
        // load-bearing assertions: they confirm fee conservation holds at the whole
        // (~1,200-club) population scale, not just the hand-built 3-club scenario
        // `transfers.rs`'s own `budget_conservation_across_a_transfer` already checks.
        let world_seed = 777;
        let season = 1u32;
        let elapsed_weeks = season * 52;
        let mut world = WorldGenesis::generate(world_seed);
        let mut pop = crate::population::genesis(world_seed, &world);
        let league_of = league_of_club(&world.static_league_clubs(), world.clubs.len());

        let sum_budget = |w: &WorldGenesis| w.clubs.iter().map(|c| c.budget).sum::<i64>();
        let before = sum_budget(&world);

        // Winter window.
        let before_winter_topup = sum_budget(&world);
        open_transfer_windows(&mut world, &pop, &league_of, elapsed_weeks);
        let after_winter_topup = sum_budget(&world);
        let winter_income_minus_wage = after_winter_topup - before_winter_topup;

        run_transfer_pass_with_log(
            &mut pop,
            &mut world,
            world_seed,
            season,
            0,
            TransferLane::WeakestPosition,
        );
        run_transfer_pass_with_log(
            &mut pop,
            &mut world,
            world_seed,
            season,
            0,
            TransferLane::GemHunt,
        );
        let after_winter_transfers = sum_budget(&world);
        assert_eq!(
            after_winter_transfers, after_winter_topup,
            "transfer fees must net to zero system-wide across the whole population \
             (winter window)"
        );

        run_academy_investment_pass(&mut world);
        let after_winter_academy = sum_budget(&world);
        let winter_academy_spend = after_winter_transfers - after_winter_academy;
        assert!(
            winter_academy_spend >= 0,
            "academy investment only ever spends"
        );

        // Summer window (the season's matches don't touch budget, so skipping them here
        // doesn't affect this invariant — this test is scoped to the economy bookkeeping,
        // not the full tick).
        let before_summer_topup = sum_budget(&world);
        open_transfer_windows(&mut world, &pop, &league_of, elapsed_weeks);
        let after_summer_topup = sum_budget(&world);
        let summer_income_minus_wage = after_summer_topup - before_summer_topup;

        run_transfer_pass_with_log(
            &mut pop,
            &mut world,
            world_seed,
            season,
            1,
            TransferLane::WeakestPosition,
        );
        run_transfer_pass_with_log(
            &mut pop,
            &mut world,
            world_seed,
            season,
            1,
            TransferLane::GemHunt,
        );
        let after_summer_transfers = sum_budget(&world);
        assert_eq!(
            after_summer_transfers, after_summer_topup,
            "transfer fees must net to zero system-wide across the whole population \
             (summer window)"
        );

        run_academy_investment_pass(&mut world);
        let after_summer_academy = sum_budget(&world);
        let summer_academy_spend = after_summer_transfers - after_summer_academy;
        assert!(
            summer_academy_spend >= 0,
            "academy investment only ever spends"
        );

        let total_delta = after_summer_academy - before;
        let expected = winter_income_minus_wage + summer_income_minus_wage
            - winter_academy_spend
            - summer_academy_spend;
        assert_eq!(
            total_delta, expected,
            "whole-season budget delta must equal total_income - total_wages - academy \
             spend; transfer fees net to zero and drop out of the equation"
        );
    }

    #[test]
    fn manager_firing_reflects_the_full_season_not_just_one_window() {
        // There is no mid-season fire path (step 4 runs exactly once, after both windows
        // and the season's matches) — confirmed structurally: every manager's
        // `matches_played` after one season-tick is bounded by one season's worth of
        // matches (`ROUNDS_PER_SEASON`), never a partial half-season snapshot evaluated
        // early, and at least one manager actually accrued match points this tick.
        let world_seed = 61;
        let mut world = WorldGenesis::generate(world_seed);
        let mut cache = ReplayCache::new(&world, world_seed);

        cache.advance_one_season(&mut world);

        for &mgr_id in &cache.managers().club_manager {
            let mgr = &cache.managers().managers[mgr_id as usize];
            assert!(
                mgr.matches_played as usize <= crate::fixtures::ROUNDS_PER_SEASON,
                "one season-tick must never record more than one season's worth of \
                 matches for a manager evaluated only once, at season end"
            );
        }
        assert!(
            cache
                .managers()
                .club_manager
                .iter()
                .any(|&id| cache.managers().managers[id as usize].matches_played > 0),
            "at least one manager must have recorded this season's match points"
        );
    }

    // ── Whole-round integration (Definition of done #5) ─────────────────────────────

    #[test]
    fn full_bl3_loop_fires_a_manager_wins_a_contested_auction_and_gem_hunts_an_outlier() {
        // A fixed seed played through several seasons, at the real ~1,200-club scale,
        // asserting every headline BL3 mechanic actually fires end-to-end (not just at
        // the unit level): a manager gets fired and replaced, at least one contested
        // auction pays above valuation, and at least one gem-hunt target is a round-3
        // outlier-style prospect (young, low current OVR, high-ceiling potential — the
        // exact shape `scouting.rs`'s own `gem_hunt_prefers_outlier_style_prospects` test
        // uses, since no stored boolean flags a player as "the outlier roll" after the
        // fact).
        const OUTLIER_STYLE_POTENTIAL: u8 = 85;
        const OUTLIER_STYLE_MAX_AGE: u32 = 23;

        let world_seed = 909_090;
        let mut world = WorldGenesis::generate(world_seed);
        let mut cache = ReplayCache::new(&world, world_seed);

        let mut any_fired = false;
        let mut any_contested_above_valuation = false;
        let mut any_gem_hunt_outlier = false;

        for _ in 0..6u32 {
            let managers_before = cache.managers().club_manager.clone();
            cache.advance_one_season(&mut world);

            if !any_fired {
                any_fired = managers_before
                    .iter()
                    .zip(cache.managers().club_manager.iter())
                    .any(|(before, after)| before != after);
            }

            for &(player_idx, _winner, _seller, fee, valuation, lane) in cache.last_transfers() {
                if fee > valuation {
                    any_contested_above_valuation = true;
                }
                if lane == TransferLane::GemHunt {
                    let potential = cache.pop().potential_ovr[player_idx];
                    let age = cache
                        .pop()
                        .age_years_at(player_idx, cache_elapsed_weeks(&cache));
                    if potential >= OUTLIER_STYLE_POTENTIAL && age <= OUTLIER_STYLE_MAX_AGE {
                        any_gem_hunt_outlier = true;
                    }
                }
            }

            if any_fired && any_contested_above_valuation && any_gem_hunt_outlier {
                break;
            }
        }

        assert!(
            any_fired,
            "expected at least one manager fired/replaced across several seasons at full scale"
        );
        assert!(
            any_contested_above_valuation,
            "expected at least one contested auction to resolve above valuation"
        );
        assert!(
            any_gem_hunt_outlier,
            "expected at least one gem-hunt target to be a round-3-outlier-style prospect"
        );
    }

    /// Test-only helper: the elapsed-weeks value `advance_one_season` used for the season
    /// it most recently resolved — needed above to evaluate a transferred player's age at
    /// (approximately) the time of the transfer, since `last_transfers` doesn't itself
    /// carry the season/window it happened in.
    fn cache_elapsed_weeks(cache: &ReplayCache) -> u32 {
        cache.resolved_through * 52
    }
}
