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
    let (results, tables, _match_points) = batch_tick_season_with_match_points(
        pop,
        world,
        league_clubs,
        world_seed,
        season,
        elapsed_weeks,
    );
    (results, tables)
}

/// Points (3/1/0) awarded to each side of a match result, home first.
fn points_from_result(gf: u32, ga: u32) -> (u8, u8) {
    match gf.cmp(&ga) {
        std::cmp::Ordering::Greater => (3, 0),
        std::cmp::Ordering::Equal => (1, 1),
        std::cmp::Ordering::Less => (0, 3),
    }
}

/// Identical body to `batch_tick_season`, plus: inside the existing per-round loop,
/// immediately after each `table.apply_result(...)`, captures this match's points for both
/// sides into the returned `Vec` — a zero-ripple sibling (Design round 5, Slice 7-8 §8.1),
/// same pattern round-3 Slice 5 used for `generate_player`/`generate_player_biased`. Every
/// existing caller/test of `batch_tick_season` stays byte-identical.
pub fn batch_tick_season_with_match_points(
    pop: &mut Population,
    world: &WorldGenesis,
    league_clubs: &[Vec<ClubId>],
    world_seed: u64,
    season: u32,
    elapsed_weeks: u32,
) -> (Vec<SeasonResult>, Vec<Table>, Vec<(ClubId, u8)>) {
    let squads = squads_by_club(pop, world.clubs.len());
    let strengths: Vec<u8> = (0..world.clubs.len())
        .map(|c| pop.live_strength_from_squad(&squads[c], elapsed_weeks))
        .collect();

    let mut results = Vec::with_capacity(world.leagues.len());
    let mut tables = Vec::with_capacity(world.leagues.len());
    let mut match_points = Vec::new();

    for (div, div_clubs) in league_clubs.iter().enumerate() {
        // Resolve the division season via the shared fixture + match machinery.
        let mut table = Table::new(div_clubs);
        let mut rng = GoatRng::new(world_seed ^ ((season as u64) << 20) ^ (div as u64));
        for round in 0..ROUNDS_PER_SEASON {
            for f in round_fixtures(world_seed, season, div, div_clubs, round) {
                let (gf, ga) = sim_team_match(strengths[f.home], strengths[f.away], &mut rng);
                table.apply_result(f.home, f.away, gf, ga);
                let (home_pts, away_pts) = points_from_result(gf, ga);
                match_points.push((f.home, home_pts));
                match_points.push((f.away, away_pts));
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

    (results, tables, match_points)
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

    // ── Slice 8 TDD anchor (Design round 5, §8.1) ───────────────────────────────

    #[test]
    fn match_points_sum_matches_table_points() {
        use crate::manager::ManagerPool;

        let world = WorldGenesis::generate(41);
        let league_clubs = world.static_league_clubs();
        let mut pop = genesis(41, &world);
        let (_results, tables, match_points) =
            batch_tick_season_with_match_points(&mut pop, &world, &league_clubs, 41, 1, 52);

        // Sum every match's captured points per club directly — this is the "faithful
        // capture" claim: `match_points`' per-club total must equal the same season's
        // `Table`'s own points column, computed independently by `apply_result`.
        let mut summed: std::collections::HashMap<ClubId, u32> = std::collections::HashMap::new();
        for &(club_id, pts) in &match_points {
            *summed.entry(club_id).or_insert(0) += pts as u32;
        }
        for (div, div_clubs) in league_clubs.iter().enumerate() {
            for &club_id in div_clubs {
                let table_points = tables[div]
                    .entries
                    .iter()
                    .find(|e| e.club_id == club_id)
                    .unwrap()
                    .points();
                assert_eq!(
                    summed.get(&club_id).copied().unwrap_or(0),
                    table_points,
                    "club {club_id}: match_points' per-club total must match the table's own points column"
                );
            }
        }

        // Corroborate with `record_match_points` itself via the non-lossy `matches_played`
        // counter (unlike `recent_points`, a `MANAGER_FORM_WINDOW`-deep ring buffer, this
        // never truncates) — every club's manager must have recorded exactly one entry per
        // match played this season.
        let mut pool = ManagerPool::genesis(41, &world);
        pool.record_match_points(&match_points);
        for &club_id in league_clubs.iter().flatten() {
            let mgr_id = pool.club_manager[club_id];
            assert_eq!(
                pool.managers[mgr_id as usize].matches_played as usize, ROUNDS_PER_SEASON,
                "club {club_id}: every round's match must be recorded exactly once"
            );
        }
    }
}
