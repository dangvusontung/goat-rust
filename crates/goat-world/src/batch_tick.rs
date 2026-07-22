//! Outer-world season batch-tick (TASK-09A Slice 9A.3, bible §244/§247, CALENDAR.md §7.1).
//!
//! Non-orbit leagues advance at **season granularity** — not match-by-match. Each season,
//! every division is resolved (table, champion, top scorer) and the path-dependent residue
//! (career goals/apps/titles) is accumulated into the population's columns. This is the
//! cheap tier: club strength is the mean current OVR of the squad (derived on demand), and
//! results never touch the deep-simmed orbit.

use crate::fixtures::{round_fixtures, ROUNDS_PER_SEASON};
use crate::population::Population;
use crate::season::Table;
use crate::sim_team_match;
use crate::world::{ClubId, WorldGenesis};
use goat_rng::GoatRng;

/// League appearances credited to a regular (top-OVR) squad member per season.
const SEASON_APPS_STARTER: u32 = 30;
/// League appearances credited to a fringe squad member per season.
const SEASON_APPS_FRINGE: u32 = 8;
/// Starters per club (the rest are fringe for appearance purposes).
const STARTERS_PER_CLUB: usize = 11;

/// Goal-attribution weight (×10) by position when sharing a club's season goals out among
/// its squad: forwards score most, defenders least.
fn goal_weight_x10(position: u8) -> u32 {
    match position {
        2 => 30, // Forward
        1 => 10, // Midfielder
        _ => 2,  // Defender
    }
}

/// One league's resolved season, for records / world screens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeasonResult {
    pub division: usize,
    pub champion_club: ClubId,
    /// Population index of the division's top scorer this season.
    pub top_scorer_idx: usize,
    pub top_scorer_goals: u32,
}

/// Build `club_id -> Vec<population index>` for the whole population (one pass).
fn squads_by_club(pop: &Population, num_clubs: usize) -> Vec<Vec<usize>> {
    let mut squads: Vec<Vec<usize>> = vec![Vec::new(); num_clubs];
    for idx in 0..pop.len() {
        squads[pop.club[idx] as usize].push(idx);
    }
    squads
}

/// Advance every non-orbit league one season. Mutates the population's career accumulators
/// and returns one `SeasonResult` per league. Deterministic in `(world_seed, season)` given
/// a fixed `league_clubs` membership (the caller resolves promotion/relegation membership
/// for this season before calling — this function just simulates it).
///
/// `league_clubs[league_id]` must line up with `world.leagues[league_id]` (same length,
/// same id space) — pass `world.static_league_clubs()` for genesis-static membership (fine
/// for flavor/replay screens that don't need real promotion/relegation drift), or a
/// promotion/relegation-advanced membership for the PC's real season-end pipeline.
pub fn batch_tick_season(
    pop: &mut Population,
    world: &WorldGenesis,
    league_clubs: &[Vec<ClubId>],
    world_seed: u64,
    season: u32,
    elapsed_weeks: u32,
) -> (Vec<SeasonResult>, Vec<Table>) {
    let squads = squads_by_club(pop, world.clubs.len());
    let strengths: Vec<u8> = (0..world.clubs.len())
        .map(|c| pop.live_strength_from_squad(&squads[c], elapsed_weeks))
        .collect();

    let mut results = Vec::with_capacity(world.leagues.len());
    let mut tables = Vec::with_capacity(world.leagues.len());

    for (div, div_clubs) in league_clubs.iter().enumerate() {
        // Resolve the division season via the shared fixture + match machinery.
        let mut table = Table::new(div_clubs);
        let mut rng = GoatRng::new(world_seed ^ ((season as u64) << 20) ^ (div as u64));
        for round in 0..ROUNDS_PER_SEASON {
            for f in round_fixtures(world_seed, season, div, div_clubs, round) {
                let (gf, ga) = sim_team_match(strengths[f.home], strengths[f.away], &mut rng);
                table.apply_result(f.home, f.away, gf, ga);
            }
        }

        let sorted = table.sorted();
        let champion_club = sorted[0].club_id;

        // Per club: appearances, titles, and a share of the club's season goals.
        let mut div_top_idx = squads[div_clubs[0]].first().copied().unwrap_or(0);
        let mut div_top_goals = 0u32;

        for entry in &sorted {
            let club = entry.club_id;
            let squad = &squads[club];
            if squad.is_empty() {
                continue;
            }

            // Appearances: top-OVR members start, the rest are fringe.
            let mut by_ovr: Vec<usize> = squad.clone();
            by_ovr.sort_by_key(|&i| std::cmp::Reverse(pop.current_ovr(i, elapsed_weeks)));
            for (rank, &idx) in by_ovr.iter().enumerate() {
                if pop.is_retired(idx, elapsed_weeks) {
                    continue;
                }
                pop.career_apps[idx] += if rank < STARTERS_PER_CLUB {
                    SEASON_APPS_STARTER
                } else {
                    SEASON_APPS_FRINGE
                };
                if club == champion_club {
                    pop.career_titles[idx] += 1;
                }
            }

            // Goals: share the club's GF across the squad by position weight × current OVR.
            let total_w: u32 = squad
                .iter()
                .filter(|&&i| !pop.is_retired(i, elapsed_weeks))
                .map(|&i| {
                    goal_weight_x10(pop.position[i]) * pop.current_ovr(i, elapsed_weeks) as u32
                })
                .sum();
            if total_w == 0 {
                continue;
            }
            for &idx in squad {
                if pop.is_retired(idx, elapsed_weeks) {
                    continue;
                }
                let w =
                    goal_weight_x10(pop.position[idx]) * pop.current_ovr(idx, elapsed_weeks) as u32;
                let goals = entry.gf * w / total_w;
                pop.career_goals[idx] += goals;
                if goals > div_top_goals {
                    div_top_goals = goals;
                    div_top_idx = idx;
                }
            }
        }

        results.push(SeasonResult {
            division: div,
            champion_club,
            top_scorer_idx: div_top_idx,
            top_scorer_goals: div_top_goals,
        });
        tables.push(table);
    }

    (results, tables)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::population::genesis;
    use crate::world::WorldGenesis;

    #[test]
    fn batch_tick_returns_one_result_per_division() {
        let world = WorldGenesis::generate(7);
        let league_clubs = world.static_league_clubs();
        let mut pop = genesis(7, &world);
        let (results, _tables) = batch_tick_season(&mut pop, &world, &league_clubs, 7, 1, 52);
        assert_eq!(results.len(), world.leagues.len());
        for (div, r) in results.iter().enumerate() {
            assert_eq!(r.division, div);
            assert!(world.leagues[div].clubs.contains(&r.champion_club));
        }
    }

    #[test]
    fn career_totals_are_monotonic_across_seasons() {
        let world = WorldGenesis::generate(11);
        let league_clubs = world.static_league_clubs();
        let mut pop = genesis(11, &world);
        let sum = |p: &Population| -> (u64, u64, u64) {
            (
                p.career_goals.iter().map(|&x| x as u64).sum(),
                p.career_apps.iter().map(|&x| x as u64).sum(),
                p.career_titles.iter().map(|&x| x as u64).sum(),
            )
        };
        let mut prev = sum(&pop);
        for season in 1..=10u32 {
            batch_tick_season(&mut pop, &world, &league_clubs, 11, season, season * 52);
            let now = sum(&pop);
            assert!(now.0 >= prev.0, "goals decreased");
            assert!(now.1 >= prev.1, "apps decreased");
            assert!(now.2 >= prev.2, "titles decreased");
            prev = now;
        }
        // After 10 seasons, real activity has accumulated.
        assert!(prev.0 > 0 && prev.1 > 0 && prev.2 > 0);
    }

    #[test]
    fn one_champion_per_division_per_season() {
        let world = WorldGenesis::generate(3);
        let league_clubs = world.static_league_clubs();
        let mut pop = genesis(3, &world);
        let titles_before: u64 = pop.career_titles.iter().map(|&x| x as u64).sum();
        let (results, _tables) = batch_tick_season(&mut pop, &world, &league_clubs, 3, 1, 52);
        // Exactly one champion named per league.
        assert_eq!(results.len(), world.leagues.len());
        let titles_after: u64 = pop.career_titles.iter().map(|&x| x as u64).sum();
        // Titles added = sum of each champion's non-retired squad size — bounded & > 0.
        assert!(titles_after > titles_before);
    }
}
