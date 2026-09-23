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
use crate::world::{div_clubs, ClubId, NUM_CLUBS, NUM_DIVISIONS};
use crate::worldgen::generate_world;
use goat_rng::{GoatRng, RngSource};
use std::collections::HashMap;

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

/// One division's resolved season, for records / world screens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeasonResult {
    pub division: usize,
    pub champion_club: ClubId,
    /// Population index of the division's top scorer this season.
    pub top_scorer_idx: usize,
    pub top_scorer_goals: u32,
}

/// Individual stats already credited by the deep-simmed orbit for one season
/// (PA2 M3). When a division the PC played in is batch-ticked, each player's
/// season apps/goals become the REMAINDER after his real orbit residue — no
/// double counting, and the division's top scorer is the man who actually
/// scored in the deep-simmed matches.
#[derive(Debug, Clone, Default)]
pub struct OrbitSeasonOverlay {
    /// Divisions the PC played in this season (normally one; two across a
    /// mid-season transfer).
    pub divs: Vec<usize>,
    /// pop_idx → orbit appearances this season (one per PC match started).
    pub apps: HashMap<u32, u32>,
    /// pop_idx → orbit goals this season.
    pub goals: HashMap<u32, u32>,
}

impl OrbitSeasonOverlay {
    fn covers(&self, div: usize) -> bool {
        self.divs.contains(&div)
    }
    fn apps_of(&self, idx: usize) -> u32 {
        self.apps.get(&(idx as u32)).copied().unwrap_or(0)
    }
    fn goals_of(&self, idx: usize) -> u32 {
        self.goals.get(&(idx as u32)).copied().unwrap_or(0)
    }
}

/// Build `club_id -> Vec<population index>` for the whole population (one pass).
fn squads_by_club(pop: &Population) -> Vec<Vec<usize>> {
    let mut squads: Vec<Vec<usize>> = vec![Vec::new(); NUM_CLUBS];
    for idx in 0..pop.len() {
        squads[pop.club[idx] as usize].push(idx);
    }
    squads
}

/// Mean current OVR of a club's squad at `elapsed_weeks` → team strength (1–99).
fn club_strength(pop: &Population, squad: &[usize], elapsed_weeks: u32) -> u8 {
    if squad.is_empty() {
        return 1;
    }
    let sum: u32 = squad
        .iter()
        .map(|&i| pop.current_ovr(i, elapsed_weeks) as u32)
        .sum();
    (sum / squad.len() as u32).clamp(1, 99) as u8
}

/// Youth intake (design B.1 NPC path — the cheap formula): at season end every
/// retired player is replaced by a fresh academy graduate at the SAME club —
/// new seed, age 16–18, potential re-anchored to club strength, career
/// accumulators reset. Squad sizes and position spread stay constant forever.
/// Deterministic in `(world_seed, season, idx)`. Returns the intake count.
fn youth_intake(pop: &mut Population, world_seed: u64, season: u32, elapsed_weeks: u32) -> usize {
    let world = generate_world(world_seed);
    let mut count = 0;
    for idx in 0..pop.len() {
        if !pop.is_retired(idx, elapsed_weeks) {
            continue;
        }
        let slot = idx % crate::population::SQUAD_SIZE;
        let club = pop.club[idx] as u64;
        let pseed =
            crate::population::player_seed_for_intake(world_seed, season, club, slot as u64);
        let mut rng = GoatRng::new(pseed);

        let age_years = rng.next_range_u32(16, 18);
        // Born after genesis → negative "age at genesis" (i64 column).
        pop.seed[idx] = pseed;
        pop.birth_age_weeks[idx] = age_years as i64 * 52 - elapsed_weeks as i64;

        let base = world.clubs[pop.club[idx] as usize].strength as i32;
        let variance = rng.next_range_u32(0, 30) as i32 - 15;
        pop.potential_ovr[idx] = (base + variance).clamp(30, 99) as u8;

        pop.career_goals[idx] = 0;
        pop.career_apps[idx] = 0;
        pop.career_titles[idx] = 0;
        pop.form[idx] = 50; // a fresh identity — any orbit residue died with the retiree
        count += 1;
    }
    count
}

/// Advance every non-orbit league one season. Mutates the population's career accumulators
/// and returns one `SeasonResult` per division. Deterministic in `(world_seed, season)`.
/// Retired players are replaced by youth intake at the end of the season.
pub fn batch_tick_season(
    pop: &mut Population,
    world_seed: u64,
    season: u32,
    elapsed_weeks: u32,
) -> Vec<SeasonResult> {
    batch_tick_season_orbit(pop, world_seed, season, elapsed_weeks, None)
}

/// Orbit-aware variant (PA2 M3): divisions listed in `orbit` credit each player
/// only the REMAINDER of his season apps/goal share after his real deep-simmed
/// residue, and the top-scorer race counts orbit goals — the division's scoring
/// chart reflects what actually happened in the PC's matches.
pub fn batch_tick_season_orbit(
    pop: &mut Population,
    world_seed: u64,
    season: u32,
    elapsed_weeks: u32,
    orbit: Option<&OrbitSeasonOverlay>,
) -> Vec<SeasonResult> {
    let squads = squads_by_club(pop);
    let strengths: Vec<u8> = (0..NUM_CLUBS)
        .map(|c| club_strength(pop, &squads[c], elapsed_weeks))
        .collect();

    let mut results = Vec::with_capacity(NUM_DIVISIONS);

    for div in 0..NUM_DIVISIONS {
        let orbit_div = orbit.is_some_and(|o| o.covers(div));
        let div_clubs = div_clubs(div);
        // Resolve the division season via the shared fixture + match machinery.
        let mut table = Table::new(div_clubs);
        let mut rng = GoatRng::new(world_seed ^ ((season as u64) << 20) ^ (div as u64));
        for round in 0..ROUNDS_PER_SEASON {
            for f in round_fixtures(world_seed, season, div, round) {
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

            // Appearances: top-OVR members start, the rest are fringe. Orbit
            // divisions credit only what the deep sim did NOT already record.
            let mut by_ovr: Vec<usize> = squad.clone();
            by_ovr.sort_by_key(|&i| std::cmp::Reverse(pop.current_ovr(i, elapsed_weeks)));
            for (rank, &idx) in by_ovr.iter().enumerate() {
                if pop.is_retired(idx, elapsed_weeks) {
                    continue;
                }
                let season_apps = if rank < STARTERS_PER_CLUB {
                    SEASON_APPS_STARTER
                } else {
                    SEASON_APPS_FRINGE
                };
                let remainder = if orbit_div {
                    season_apps.saturating_sub(orbit.map_or(0, |o| o.apps_of(idx)))
                } else {
                    season_apps
                };
                pop.career_apps[idx] += remainder;
                if club == champion_club {
                    pop.career_titles[idx] += 1;
                }
            }

            // Goals: share the club's GF across the squad by position weight × current OVR.
            // Orbit divisions scale the share down to the UNRECORDED matches only:
            // a player whose orbit apps already cover his whole season quota has
            // no phantom fixtures left to score in — his real goals stand alone.
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
            // Starter/fringe quota per player (same ranking as the apps pass).
            for &idx in squad {
                if pop.is_retired(idx, elapsed_weeks) {
                    continue;
                }
                let rank = by_ovr.iter().position(|&i| i == idx).unwrap_or(usize::MAX);
                let season_apps = if rank < STARTERS_PER_CLUB {
                    SEASON_APPS_STARTER
                } else {
                    SEASON_APPS_FRINGE
                };
                let w =
                    goal_weight_x10(pop.position[idx]) * pop.current_ovr(idx, elapsed_weeks) as u32;
                let share = entry.gf * w / total_w;
                let (remainder_apps, orbit_goals) = if orbit_div {
                    (
                        season_apps.saturating_sub(orbit.map_or(0, |o| o.apps_of(idx))),
                        orbit.map_or(0, |o| o.goals_of(idx)),
                    )
                } else {
                    (season_apps, 0)
                };
                let remainder_goals = share * remainder_apps / season_apps.max(1);
                pop.career_goals[idx] += remainder_goals;
                // The scoring chart sees the real total: orbit goals + remainder.
                let season_goals = orbit_goals + remainder_goals;
                if season_goals > div_top_goals {
                    div_top_goals = season_goals;
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
    }

    // Season end: retirees make way for the next academy class.
    youth_intake(pop, world_seed, season, elapsed_weeks);

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::population::genesis;

    #[test]
    fn batch_tick_returns_one_result_per_division() {
        let mut pop = genesis(7);
        let results = batch_tick_season(&mut pop, 7, 1, 52);
        assert_eq!(results.len(), NUM_DIVISIONS);
        for (div, r) in results.iter().enumerate() {
            assert_eq!(r.division, div);
            assert!(div_clubs(div).contains(&r.champion_club));
        }
    }

    #[test]
    fn career_totals_are_monotonic_across_seasons() {
        let mut pop = genesis(11);
        let sum = |p: &Population| -> (u64, u64, u64) {
            (
                p.career_goals.iter().map(|&x| x as u64).sum(),
                p.career_apps.iter().map(|&x| x as u64).sum(),
                p.career_titles.iter().map(|&x| x as u64).sum(),
            )
        };
        let mut prev = sum(&pop);
        for season in 1..=10u32 {
            batch_tick_season(&mut pop, 11, season, season * 52);
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
        let mut pop = genesis(3);
        let titles_before: u64 = pop.career_titles.iter().map(|&x| x as u64).sum();
        let results = batch_tick_season(&mut pop, 3, 1, 52);
        // Exactly NUM_DIVISIONS champions named.
        assert_eq!(results.len(), NUM_DIVISIONS);
        let titles_after: u64 = pop.career_titles.iter().map(|&x| x as u64).sum();
        // Titles added = sum of each champion's non-retired squad size — bounded & > 0.
        assert!(titles_after > titles_before);
    }

    #[test]
    fn youth_intake_replaces_every_retiree() {
        let mut pop = genesis(7);
        let size = pop.len();
        // Run long enough that the entire genesis generation has retired.
        for season in 1..=25u32 {
            batch_tick_season(&mut pop, 7, season, season * 52);
        }
        assert_eq!(pop.len(), size, "population size must stay constant");
        for idx in 0..pop.len() {
            assert!(
                !pop.is_retired(idx, 25 * 52),
                "idx {idx} still retired after intake"
            );
        }
    }

    #[test]
    fn youth_intake_is_deterministic() {
        let run = || {
            let mut pop = genesis(11);
            for season in 1..=10u32 {
                batch_tick_season(&mut pop, 11, season, season * 52);
            }
            pop.fingerprint()
        };
        assert_eq!(run(), run(), "intake must be deterministic");
    }

    #[test]
    fn intake_players_reset_career_and_stay_club_anchored() {
        let mut pop = genesis(5);
        for season in 1..=25u32 {
            batch_tick_season(&mut pop, 5, season, season * 52);
        }
        let world = generate_world(5);
        for idx in 0..pop.len() {
            // A post-intake player (career shorter than 25 seasons) must have a
            // potential anchored near his club's strength band.
            let age_weeks = pop.birth_age_weeks[idx] + 25 * 52;
            let career_seasons = age_weeks / 52 - 15; // rough: debuted ~15-16
            if career_seasons < 20 {
                let club_str = world.clubs[pop.club[idx] as usize].strength as i32;
                let pot = pop.potential_ovr[idx] as i32;
                assert!(
                    (pot - club_str).abs() <= 15 || pot == 30,
                    "idx {idx}: potential {pot} far from club str {club_str}"
                );
            }
        }
    }
}
