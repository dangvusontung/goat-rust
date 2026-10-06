//! Five completed seasons of normalized continental performance, pure and replayable.
use crate::{
    continental::{
        continental_slots_for_nation, simulate_continental_with_qualified, ContinentalSeasonResult,
        ContinentalTier,
    },
    deep::{initial_scores, LeagueScore},
    season::Table,
    world::{ClubId, DivLevel, WorldGenesis},
};
use std::collections::VecDeque;
pub const WINDOW: usize = 5;
pub const POINT_SCALE: u32 = 1000;
pub const WIN_POINTS: u32 = 2;
pub const DRAW_POINTS: u32 = 1;
pub const GROUP_BONUS: [u32; 3] = [4, 2, 1];
pub const ADVANCE_BONUS: u32 = 1;
const TIER_PENALTY: u32 = 20_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnualCoefficient {
    pub season: u32,
    pub points: Vec<u32>,
    pub entrants: Vec<u32>,
}
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct RankingHistory {
    years: VecDeque<AnnualCoefficient>,
}
impl RankingHistory {
    /// Consecutive completed seasons only; reject a duplicate or reordered year.
    pub fn record(&mut self, annual: AnnualCoefficient) -> bool {
        let next = self.years.back().map_or(1, |last| last.season + 1);
        if annual.season != next
            || annual.points.len() != annual.entrants.len()
            || self
                .years
                .back()
                .is_some_and(|last| last.points.len() != annual.points.len())
        {
            return false;
        }
        self.years.push_back(annual);
        if self.years.len() > WINDOW {
            self.years.pop_front();
        }
        true
    }
    pub fn years(&self) -> &VecDeque<AnnualCoefficient> {
        &self.years
    }
    pub fn scores(&self, world: &WorldGenesis) -> Vec<LeagueScore> {
        if self.years.is_empty() {
            return initial_scores(world);
        }
        world
            .leagues
            .iter()
            .map(|league| {
                let history = self
                    .years
                    .iter()
                    .map(|year| year.points[league.nation] as u64)
                    .sum::<u64>();
                let prior = world.nations[league.nation].stature as u64
                    * POINT_SCALE as u64
                    * (WINDOW - self.years.len()) as u64
                    / WINDOW as u64;
                LeagueScore {
                    league: league.id,
                    score: (history + prior)
                        .saturating_sub(league.tier as u64 * TIER_PENALTY as u64)
                        .min(u32::MAX as u64) as u32,
                }
            })
            .collect()
    }
}

/// Offset each tier's domestic positions, so no club simultaneously plays three cups.
pub fn qualified(world: &WorldGenesis, tables: &[Table], tier: ContinentalTier) -> Vec<ClubId> {
    let mut out = Vec::new();
    for nation in &world.nations {
        let slots = continental_slots_for_nation(world, nation.id);
        let offset = match tier {
            ContinentalTier::Tier1 => 0,
            ContinentalTier::Tier2 => slots.tier1,
            ContinentalTier::Tier3 => slots.tier1 + slots.tier2,
        } as usize;
        let league = world
            .leagues
            .iter()
            .find(|l| l.nation == nation.id && l.tier == DivLevel::Top)
            .unwrap();
        out.extend(
            tables[league.id]
                .sorted()
                .iter()
                .skip(offset)
                .take(slots.for_tier(tier) as usize)
                .map(|e| e.club_id),
        );
    }
    out
}

/// Count each played leg separately; shootouts do not turn a drawn leg into a win.
pub fn annual(
    world: &WorldGenesis,
    season: u32,
    results: &[ContinentalSeasonResult],
) -> AnnualCoefficient {
    let mut points = vec![0u64; world.nations.len()];
    let mut entrants = vec![0u32; world.nations.len()];
    for result in results {
        let bonus = GROUP_BONUS[match result.tier {
            ContinentalTier::Tier1 => 0,
            ContinentalTier::Tier2 => 1,
            ContinentalTier::Tier3 => 2,
        }];
        for group in &result.groups {
            for row in group {
                let nation = world.clubs[row.club].nation;
                entrants[nation] += 1;
                points[nation] += (row.w * WIN_POINTS + row.d * DRAW_POINTS + bonus) as u64;
            }
        }
        for round in &result.knockout {
            for tie in &round.ties {
                let a = world.clubs[tie.club_a].nation;
                let b = world.clubs[tie.club_b].nation;
                for (ga, gb) in std::iter::once((tie.leg1_a_goals, tie.leg1_b_goals))
                    .chain(tie.leg2_a_goals.zip(tie.leg2_b_goals))
                {
                    if ga == gb {
                        points[a] += DRAW_POINTS as u64;
                        points[b] += DRAW_POINTS as u64;
                    } else {
                        points[if ga > gb { a } else { b }] += WIN_POINTS as u64;
                    }
                }
                points[world.clubs[tie.winner].nation] += ADVANCE_BONUS as u64;
            }
        }
    }
    AnnualCoefficient {
        season,
        points: points
            .into_iter()
            .zip(&entrants)
            .map(|(points, &count)| {
                (points * POINT_SCALE as u64 / count.max(1) as u64).min(u32::MAX as u64) as u32
            })
            .collect(),
        entrants,
    }
}

pub fn simulate_annual(
    world: &WorldGenesis,
    tables: &[Table],
    seed: u64,
    season: u32,
) -> AnnualCoefficient {
    let results = ContinentalTier::ALL
        .into_iter()
        .map(|tier| {
            simulate_continental_with_qualified(
                world,
                seed,
                season,
                tier,
                &qualified(world, tables, tier),
            )
        })
        .collect::<Vec<_>>();
    annual(world, season, &results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::continental::{GroupStanding, KnockoutRoundResult, KnockoutTie};
    fn tables(world: &WorldGenesis) -> Vec<Table> {
        world.leagues.iter().map(|l| Table::new(&l.clubs)).collect()
    }
    #[test]
    fn distinct_entries_and_deterministic_normalized_results() {
        let world = WorldGenesis::generate(42);
        let tables = tables(&world);
        let entries: Vec<_> = ContinentalTier::ALL
            .into_iter()
            .flat_map(|tier| qualified(&world, &tables, tier))
            .collect();
        assert_eq!(entries.len(), 144);
        assert_eq!(
            entries
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            144
        );
        let first = simulate_annual(&world, &tables, 42, 1);
        assert_eq!(first, simulate_annual(&world, &tables, 42, 1));
        assert_eq!(first.entrants.iter().sum::<u32>(), 144);
        assert!(first
            .points
            .iter()
            .zip(&first.entrants)
            .all(|(&p, &n)| (p > 0) == (n > 0)));
        let mut history = RankingHistory::default();
        assert!(history.record(first));
        assert!(!history.record(simulate_annual(&world, &tables, 42, 1)));
        assert!(!history.record(simulate_annual(&world, &tables, 42, 3)));
    }
    #[test]
    fn drawn_shootout_and_two_legs_count_before_entry_normalization() {
        let world = WorldGenesis::generate(42);
        let clubs: Vec<_> = world
            .nations
            .iter()
            .take(4)
            .map(|n| world.clubs.iter().find(|c| c.nation == n.id).unwrap().id)
            .collect();
        let groups = vec![std::array::from_fn(|i| GroupStanding {
            club: clubs[i],
            w: 1,
            d: 1,
            l: 1,
            gf: 3,
            ga: 3,
        })];
        let result = ContinentalSeasonResult {
            tier: ContinentalTier::Tier1,
            groups,
            knockout: vec![KnockoutRoundResult {
                round: 0,
                bye: None,
                ties: vec![KnockoutTie {
                    club_a: clubs[0],
                    club_b: clubs[1],
                    leg1_a_goals: 1,
                    leg1_b_goals: 1,
                    leg2_a_goals: Some(0),
                    leg2_b_goals: Some(0),
                    winner: clubs[0],
                }],
            }],
            champion: clubs[0],
        };
        let year = annual(&world, 1, std::slice::from_ref(&result));
        assert_eq!(&year.points[..4], &[10_000, 9_000, 7_000, 7_000]);
        assert_eq!(&year.entrants[..4], &[1, 1, 1, 1]);
        let mut two_entries = result;
        two_entries.groups.push(two_entries.groups[0]);
        let normalized = annual(&world, 1, &[two_entries]);
        assert_eq!(&normalized.entrants[..4], &[2, 2, 2, 2]);
        assert_eq!(&normalized.points[..4], &[8500, 8000, 7000, 7000]);
    }
    #[test]
    fn five_year_window_removes_prior_and_expires_old_results() {
        let world = WorldGenesis::generate(42);
        let mut history = RankingHistory::default();
        assert_eq!(history.scores(&world), initial_scores(&world));
        for season in 1..=6 {
            assert!(history.record(AnnualCoefficient {
                season,
                points: vec![season * 1000; world.nations.len()],
                entrants: vec![1; world.nations.len()],
            }));
            if season == 1 {
                assert_eq!(
                    history.scores(&world)[0].score,
                    1000 + world.nations[0].stature as u32 * 800
                );
            }
        }
        assert_eq!(history.years().len(), 5);
        assert_eq!(history.years().front().unwrap().season, 2);
        for score in history.scores(&world) {
            assert_eq!(
                score.score,
                20_000u32.saturating_sub(world.leagues[score.league].tier as u32 * 20_000)
            );
        }
    }
    #[test]
    fn results_override_stature_and_keep_pc_league_with_stable_ties() {
        let world = WorldGenesis::generate(42);
        let mut history = RankingHistory::default();
        for season in 1..=5 {
            let mut points = vec![6000; world.nations.len()];
            points[19] = 8000;
            assert!(history.record(AnnualCoefficient {
                season,
                points,
                entrants: vec![1; world.nations.len()]
            }));
        }
        let scores = history.scores(&world);
        let top = world
            .leagues
            .iter()
            .find(|l| l.nation == 19 && l.tier == DivLevel::Top)
            .unwrap()
            .id;
        let pc = world
            .leagues
            .iter()
            .find(|l| l.nation == 18 && l.tier == DivLevel::Third)
            .unwrap()
            .id;
        let selected = crate::deep::select_leagues(&world, pc, 5, &scores);
        assert!(selected.contains(&top));
        assert!(selected.contains(&pc));
        let first_four: Vec<_> = world
            .leagues
            .iter()
            .filter(|l| l.tier == DivLevel::Top && l.nation != 19)
            .take(4)
            .map(|l| l.id)
            .collect();
        assert!(first_four.iter().all(|l| selected.contains(l)));
    }
    #[test]
    fn session_switches_models_and_reconstructs_season_opening_scores() {
        let mut state = goat_core::state::WorldState::new();
        state.world_seed = 42;
        state.dated_calendar = true;
        state.career_base_year = 2023;
        state.season_number = 1;
        let mut session = crate::session::SimulationSession::new();
        session.population_dated(42, 2023, 1, &[], &[]);
        assert_eq!(
            session.league_scores(&state),
            initial_scores(&WorldGenesis::generate(42))
        );
        assert_eq!(session.rebuild_count(), 2);
        state.season_number = 2;
        let opening = session.league_scores(&state);
        let mut cold = crate::session::SimulationSession::new();
        assert_eq!(opening, cold.league_scores(&state));
        assert_eq!(opening, session.league_scores(&state));
        assert_eq!(session.rebuild_count(), 2);
    }
    #[test]
    fn ranked_replay_keeps_history_and_legacy_factory_separate() {
        let mut world = WorldGenesis::generate(42);
        let mut cache = crate::promotion::ReplayCache::new_ranked(&world, 42, 2023);
        cache.advance_one_season(&mut world);
        assert_eq!(cache.ranking_history().unwrap().years().len(), 1);
        let scores = cache.league_scores(&world);
        let mut fresh_world = WorldGenesis::generate(42);
        let mut fresh = crate::promotion::ReplayCache::new_ranked(&fresh_world, 42, 2023);
        fresh.advance_one_season(&mut fresh_world);
        assert_eq!(scores, fresh.league_scores(&fresh_world));
        assert_eq!(cache.ranking_history(), fresh.ranking_history());
        assert!(crate::promotion::ReplayCache::new_dated(&world, 42, 2023)
            .ranking_history()
            .is_none());
    }
}

crate::checkpoint::fields!(RankingHistory { years });
crate::checkpoint::fields!(AnnualCoefficient {
    season,
    points,
    entrants
});
