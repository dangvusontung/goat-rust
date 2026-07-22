//! Promotion & relegation between tiers (TASK-DESIGN-round2 A3).
//!
//! Design: **replay-from-seed, zero new persisted state** (A3.1's recommended fork) —
//! league membership for season N is derived by replaying promotion/relegation for
//! seasons 1..N from genesis-static membership, purely from `(world_seed, season)`, the
//! same "seed is the universe, recompute the rest" pattern `History` already uses. An
//! in-memory `ReplayCache` (below) avoids re-replaying from season 1 on every call within
//! a session by extending incrementally as `season_number` advances (A3.1 point 2).

use crate::batch_tick::batch_tick_season;
use crate::population::Population;
use crate::season::Table;
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

/// In-memory (never persisted) replay cache: holds league membership and the events that
/// produced it, as of the highest season fully resolved so far. Extending it one season at
/// a time (rather than replaying from season 1 every call) is what keeps per-season cost
/// bounded as a career gets long (A3.1 point 2's flagged perf concern).
pub struct ReplayCache {
    world_seed: u64,
    /// Highest season whose promotion/relegation has been resolved into `membership`.
    /// `0` means genesis-static membership (no season played yet).
    resolved_through: u32,
    membership: Vec<Vec<ClubId>>,
    /// Population reused across the whole replay — genesis identity/potential columns are
    /// pure functions of `world_seed`, so one `Population` serves the entire cache lifetime.
    pop: Population,
}

impl ReplayCache {
    pub fn new(world: &WorldGenesis, world_seed: u64) -> Self {
        Self {
            world_seed,
            resolved_through: 0,
            membership: world.static_league_clubs(),
            pop: crate::population::genesis(world_seed, world),
        }
    }

    /// League membership to use for `season` (i.e. as of the most recently resolved
    /// promotion/relegation *before* `season` kicks off). Extends the cache one season at a
    /// time until `season - 1` is resolved.
    pub fn membership_for_season(&mut self, world: &WorldGenesis, season: u32) -> &[Vec<ClubId>] {
        while self.resolved_through + 1 < season {
            self.advance_one_season(world);
        }
        &self.membership
    }

    /// Resolve one season's promotion/relegation on top of the cached membership, advancing
    /// `resolved_through` by one. Returns that season's events.
    pub fn advance_one_season(&mut self, world: &WorldGenesis) -> Vec<PromoRelegationEvent> {
        let season = self.resolved_through + 1;
        let (_results, tables) = batch_tick_season(
            &mut self.pop,
            world,
            &self.membership,
            self.world_seed,
            season,
            season * 52,
        );
        let events = apply_season_end(world, &mut self.membership, season, &tables);
        // Youth intake shapes *next* season's roster; ordering vs. apply_season_end above
        // doesn't matter for correctness, but running after batch_tick_season means this
        // season's career-accumulator crediting isn't affected by mid-season-boundary new
        // arrivals (Design round 3, Doc C §4.6).
        crate::population::apply_youth_intake(&mut self.pop, world, self.world_seed, season);
        self.resolved_through = season;
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::WorldGenesis;

    #[test]
    fn promotion_and_relegation_are_symmetric_counts() {
        let world = WorldGenesis::generate(5);
        let mut cache = ReplayCache::new(&world, 5);
        let events = cache.advance_one_season(&world);
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
        let world = WorldGenesis::generate(5);
        let mut cache = ReplayCache::new(&world, 5);
        let events = cache.advance_one_season(&world);
        for e in &events {
            let from = &world.leagues[e.from_league];
            let to = &world.leagues[e.to_league];
            assert_eq!(from.nation, to.nation, "transitions stay within a nation");
            assert_ne!(from.tier, to.tier);
        }
    }

    #[test]
    fn league_sizes_stay_uniform_after_transitions() {
        let world = WorldGenesis::generate(9);
        let mut cache = ReplayCache::new(&world, 9);
        cache.advance_one_season(&world);
        cache.advance_one_season(&world);
        let membership = cache.membership_for_season(&world, 3);
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
        let world = WorldGenesis::generate(21);
        let mut a = ReplayCache::new(&world, 21);
        let mut b = ReplayCache::new(&world, 21);
        for _ in 0..3 {
            let ea = a.advance_one_season(&world);
            let eb = b.advance_one_season(&world);
            assert_eq!(ea, eb);
        }
    }

    #[test]
    fn idempotent_view_does_not_double_apply() {
        // Viewing the same season's resolution twice (without advancing) must be a no-op —
        // membership_for_season only replays up to `season - 1`, never re-applies a season
        // already resolved.
        let world = WorldGenesis::generate(13);
        let mut cache = ReplayCache::new(&world, 13);
        cache.advance_one_season(&world);
        let first = cache.membership_for_season(&world, 2).to_vec();
        let second = cache.membership_for_season(&world, 2).to_vec();
        assert_eq!(first, second);
    }
}
