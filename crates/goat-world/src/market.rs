//! Dated registration windows shared by headless adapters. No wall clock or UI rules.
use goat_core::chronology::{Chronology, CivilDate};
pub(crate) const EVENTS_PER_SEASON: u32 = 4;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketEventKind {
    SummerOpen,
    SummerClose,
    WinterOpen,
    WinterClose,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarketEvent {
    pub day: u32,
    pub kind: MarketEventKind,
}
/// Generic federation policy: summer July 1–August 31, winter January 1–31.
pub fn events(c: Chronology, season: u32) -> [MarketEvent; 4] {
    let year = c.base_year + season - 1;
    [
        (year, 7, 1, MarketEventKind::SummerOpen),
        (year, 8, 31, MarketEventKind::SummerClose),
        (year + 1, 1, 1, MarketEventKind::WinterOpen),
        (year + 1, 1, 31, MarketEventKind::WinterClose),
    ]
    .map(|(year, month, day, kind)| MarketEvent {
        day: c.epoch_day(CivilDate { year, month, day }).unwrap(),
        kind,
    })
}
/// Registrations execute only while a window is open, with inclusive closing dates.
pub fn registration_open(c: Chronology, day: u32) -> bool {
    let d = c.date(day);
    matches!(d.month, 7 | 8 | 1)
}

/// Close from actual dated scores. Light games allocate only a cheap personal
/// projection to registrations on that date; detailed credits are already canonical.
#[allow(clippy::type_complexity)]
pub(crate) fn finish_dated_season(
    pop: &mut crate::population::Population,
    world: &crate::world::WorldGenesis,
    membership: &[Vec<usize>],
    cal: &goat_core::competitions::CompetitionCalendar,
    seed: u64,
    season: u32,
    records: &[goat_core::state::OrbitMatchRecord],
) -> (
    Vec<crate::batch_tick::SeasonResult>,
    Vec<crate::season::Table>,
    Vec<(usize, u8)>,
) {
    use goat_rng::{GoatRng, RngSource};
    let mut tables = membership
        .iter()
        .map(|clubs| crate::season::Table::new(clubs))
        .collect::<Vec<_>>();
    let mut points = Vec::new();
    let mut league_goals = std::collections::BTreeMap::<(usize, usize), u32>::new();
    for record in records
        .iter()
        .filter(|r| r.season == season && r.round & goat_core::competitions::EXTRA_ROUND == 0)
    {
        for credit in &record.credits {
            *league_goals
                .entry((record.div as usize, credit.pop_idx as usize))
                .or_default() += credit.goals as u32;
        }
    }
    // Indexed candidate union avoids scanning the population once per fixture.
    let mut club_candidates = vec![Vec::new(); world.clubs.len()];
    for idx in 0..pop.len() {
        let clubs = std::iter::once(pop.original_clubs[idx])
            .chain(pop.club_history(idx).iter().map(|&(_, club)| club))
            .collect::<std::collections::BTreeSet<_>>();
        for club in clubs {
            club_candidates[club as usize].push(idx);
        }
    }
    for result in cal.results.iter().filter(|r| r.fixture.season == season) {
        let f = &result.fixture;
        if f.competition == 1 {
            tables[f.region as usize].apply_result(
                f.home as usize,
                f.away as usize,
                result.goals[0],
                result.goals[1],
            );
            let pts = match result.goals[0].cmp(&result.goals[1]) {
                std::cmp::Ordering::Greater => [3, 0],
                std::cmp::Ordering::Less => [0, 3],
                _ => [1, 1],
            };
            points.extend([(f.home as usize, pts[0]), (f.away as usize, pts[1])]);
        }
        if result.detailed || !matches!(f.competition, 1..=5) {
            continue;
        }
        for (side, club) in [f.home, f.away].into_iter().enumerate() {
            let eligible = club_candidates[club as usize]
                .iter()
                .copied()
                .filter(|&idx| {
                    pop.club_at(idx, f.day) == Some(club as u16) && !pop.is_retired(idx, f.day / 7)
                })
                .collect::<Vec<_>>();
            let mut weights = Vec::new();
            for idx in eligible {
                // Same seeded planned light participation model; no invented observed minute journal.
                let plays = if f.competition == 1 {
                    pop.fixture_minutes_for_week(idx, f.day / 7)
                        .iter()
                        .any(|d| d.fixture_id == f.workload_id && d.minutes > 0)
                } else {
                    let regular = pop.seed[idx] % 25 < 11;
                    GoatRng::new(pop.seed[idx] ^ f.id ^ 0x4C49_4748_5441_5050).next_range_u32(0, 99)
                        < if regular { 79 } else { 21 }
                };
                if plays {
                    pop.career_apps[idx] += 1;
                }
                let position = match pop.position[idx] {
                    2 => 30,
                    1 => 10,
                    _ => 2,
                };
                let weight = if plays && !pop.goalkeeper[idx] {
                    position * pop.potential_ovr[idx] as u32
                } else {
                    0
                };
                if weight > 0 {
                    weights.push((idx, weight));
                }
            }
            let total: u32 = weights.iter().map(|&(_, w)| w).sum();
            if total == 0 {
                continue;
            }
            let mut rng = GoatRng::new(seed ^ f.id ^ ((side as u64) << 60) ^ 0x4C49_4748_5447_4F41);
            for _ in 0..result.goals[side] {
                let mut draw = rng.next_range_u32(0, total - 1);
                for &(idx, w) in &weights {
                    if draw < w {
                        pop.career_goals[idx] += 1;
                        if f.competition == 1 {
                            *league_goals.entry((f.region as usize, idx)).or_default() += 1;
                        }
                        break;
                    }
                    draw -= w;
                }
            }
        }
    }
    let c = pop.calendar_for_replay().unwrap();
    let mut results = Vec::new();
    for (div, table) in tables.iter().enumerate() {
        let champion = table.sorted()[0].club_id;
        let mut best = (0, 0);
        for &club in &membership[div] {
            for &idx in &club_candidates[club] {
                if club == champion
                    && pop.club_at(idx, c.frame(season).end_day) == Some(champion as u16)
                    && !pop.is_retired(idx, c.frame(season).end_day / 7)
                {
                    pop.career_titles[idx] += 1;
                }
                let goals = league_goals.get(&(div, idx)).copied().unwrap_or(0);
                if goals > best.1 {
                    best = (idx, goals);
                }
            }
        }
        results.push(crate::batch_tick::SeasonResult {
            division: div,
            champion_club: champion,
            top_scorer_idx: best.0,
            top_scorer_goals: best.1,
        });
    }
    (results, tables, points)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{population::genesis_dated, promotion::ReplayCache, world::WorldGenesis};
    fn small_world() -> WorldGenesis {
        let mut world = WorldGenesis::generate(42);
        world.clubs.truncate(3);
        world.leagues.truncate(1);
        world.leagues[0].clubs = vec![0, 1, 2];
        world
    }
    #[test]
    fn dated_window_dates_and_inclusive_edges_are_golden() {
        let c = Chronology::new(2023);
        assert_eq!(events(c, 1).map(|e| e.day), [0, 61, 184, 214]);
        assert_eq!(events(c, 2).map(|e| e.day), [366, 427, 550, 580]);
        for day in [0, 61, 184, 214] {
            assert!(registration_open(c, day));
        }
        for day in [62, 183, 215] {
            assert!(!registration_open(c, day));
        }
    }
    #[test]
    fn market_split_progression_reads_and_cold_replay_agree() {
        let world = small_world();
        let mut hot_world = world.clone();
        let mut cold_world = world.clone();
        let mut hot = ReplayCache::new_ranked(&world, 42, 2023);
        hot.enable_dated_market();
        let mut cold = ReplayCache::new_ranked(&world, 42, 2023);
        cold.enable_dated_market();
        hot.advance_market(&mut hot_world, 1, 0, None);
        let opening = hot.pop().fingerprint();
        hot.advance_market(&mut hot_world, 1, 60, None);
        assert_eq!(hot.pop().fingerprint(), opening);
        hot.advance_market(&mut hot_world, 1, 61, None);
        hot.advance_market(&mut hot_world, 1, 184, None);
        hot.advance_market(&mut hot_world, 1, 214, None);
        cold.advance_market(&mut cold_world, 1, 214, None);
        assert_eq!(hot.pop().fingerprint(), cold.pop().fingerprint());
        assert!(!hot.pop().affiliations.is_empty());
        assert_eq!(hot.pop().affiliations, cold.pop().affiliations);
        assert_eq!(hot.pop().contract_ends, cold.pop().contract_ends);
        assert_eq!(hot.club_budgets(), cold.club_budgets());
    }
    #[test]
    fn moving_registration_preserves_identity_career_and_medical_past() {
        let world = small_world();
        let mut pop = genesis_dated(42, &world, 2023);
        pop.enable_dated_rosters();
        let idx = 0;
        let before = pop.promote(idx, 20, "NPC", &world).unwrap();
        let genome = pop.seed[idx];
        let old = pop.club[idx];
        pop.career_apps[idx] = 7;
        pop.career_goals[idx] = 3;
        pop.move_club(idx, 1, 184);
        assert_eq!(pop.seed[idx], genome);
        assert_eq!((pop.career_apps[idx], pop.career_goals[idx]), (7, 3));
        assert_eq!(pop.club_at(idx, 183), Some(old));
        assert_eq!(pop.club_at(idx, 184), Some(1));
        assert_eq!(pop.club_history(idx), &[(184, 1)]);
        let after = pop.promote(idx, 20, "NPC", &world).unwrap();
        assert_eq!(before.current, after.current);
        assert_eq!(before.energy, after.energy);
        assert_eq!(before.injury_weeks, after.injury_weeks);
    }
    #[test]
    fn july_intake_keeps_indices_and_cannot_play_before_entry() {
        let mut world = small_world();
        for club in &mut world.clubs {
            club.squad_size = 60;
        }
        let mut pop = genesis_dated(42, &world, 2023);
        pop.enable_dated_rosters();
        let old = pop.seed.clone();
        let start = pop.len();
        let day = 366;
        let added =
            crate::population::apply_youth_intake_at(&mut pop, &world, 42, 1, day / 7, Some(day));
        assert!(added > 0);
        assert_eq!(&pop.seed[..start], old.as_slice());
        for idx in start..pop.len() {
            assert_eq!(pop.club_at(idx, day - 1), None);
            assert!(pop.club_at(idx, day).is_some());
            assert!(pop.contract_end_day(idx).unwrap() > day);
        }
    }
}

#[cfg(test)]
mod pc_tests {
    use super::*;
    use goat_core::state::{reduce, Intent, WorldState};
    use goat_rng::GoatRng;
    #[test]
    fn pc_registration_window_contract_and_deep_scope_use_live_world() {
        let world = crate::world::WorldGenesis::generate(42);
        let mut state = WorldState::new();
        state.world_seed = 42;
        state.dated_calendar = true;
        state.career_base_year = 2023;
        state.season_number = 1;
        state.pc_club_idx = 1180;
        state.pc_div_idx = 59;
        state = reduce(state, Intent::EnableDatedCompetitions, &mut GoatRng::new(0));
        let intent = || Intent::ExecuteTransfer {
            to_club_idx: 0,
            to_div_idx: 59,
            new_wage: 123,
            new_length: 2,
            new_club_name: "ignored renderer name".into(),
            facilities_mult: goat_fixed::Fixed::ZERO,
            staff_mods: goat_core::staff::StaffMods::NEUTRAL,
            fee_bonus: 0,
        };
        let mut session = crate::session::SimulationSession::new();
        let mut outside = state.clone();
        outside.pc_epoch_day = 62;
        assert_eq!(
            session
                .transfer_pc_on_date(outside, &world, intent())
                .unwrap_err(),
            crate::competitions::CalendarError::InvalidClock
        );
        let state = session
            .transfer_pc_on_date(state, &world, intent())
            .unwrap();
        assert_eq!(state.pc_club_idx, 0);
        assert_eq!(state.pc_div_idx, 0);
        assert_eq!(state.pc_club, world.clubs[0].name);
        let cal = state.competition_calendar.unwrap();
        assert_eq!(cal.pc_affiliations, vec![(0, 0)]);
        assert_eq!(
            cal.pc_contract_end,
            Some(Chronology::new(2023).frame(3).preparation_start)
        );
        let scope = state.deep_scopes.last().unwrap();
        assert_eq!(scope.pc_league, 0);
        assert!(scope.leagues.contains(&0));
        assert!(scope.leagues.len() <= 6);
    }
}

#[cfg(test)]
mod registration_load_tests {
    use goat_core::competitions::{CompetitionCalendar, DatedFixture};
    #[test]
    fn old_and_new_club_fixtures_keep_distinct_loads_after_transfer() {
        let mut world = crate::world::WorldGenesis::generate(42);
        world.clubs.truncate(3);
        let mut pop = crate::population::genesis_dated(42, &world, 2023);
        pop.enable_dated_rosters();
        let fixture = |id, day, home| DatedFixture {
            id,
            workload_id: id,
            season: 1,
            competition: 1,
            region: 0,
            round: 12,
            slot: 0,
            stage: 0,
            leg: 0,
            original_day: day,
            day,
            home,
            away: 2,
            priority: 1,
        };
        let cal = CompetitionCalendar {
            fixtures: vec![fixture(1001, 140, 0), fixture(1002, 150, 1)],
            market_enabled: true,
            ..Default::default()
        };
        pop.move_club(0, 1, 145);
        pop.apply_fixture_dates(&cal, 1);
        assert!(pop
            .fixture_minutes_for_week(0, 20)
            .iter()
            .any(|d| d.fixture_id == 1001));
        assert!(pop
            .fixture_minutes_for_week(0, 21)
            .iter()
            .any(|d| d.fixture_id == 1002));
        for (id, day) in [(1001, 140), (1002, 150)] {
            assert!(pop.record_match_load(goat_core::history::NpcMatchLoad {
                pop_idx: 0,
                competition_id: 1,
                fixture_id: id,
                epoch_day: day,
                minutes: 90
            }));
        }
        assert_eq!(pop.observed_match_loads(0).len(), 2);
    }
    #[test]
    fn rescheduling_invalidates_speculative_medical_views() {
        let mut world = crate::world::WorldGenesis::generate(42);
        world.clubs.truncate(3);
        let mut hot = crate::population::genesis_dated(42, &world, 2023);
        hot.enable_dated_rosters();
        let mut cold = hot.clone();
        let f = DatedFixture {
            id: 1001,
            workload_id: 1001,
            season: 1,
            competition: 1,
            region: 0,
            round: 12,
            slot: 0,
            stage: 0,
            leg: 0,
            original_day: 140,
            day: 140,
            home: 0,
            away: 2,
            priority: 1,
        };
        let mut cal = CompetitionCalendar {
            fixtures: vec![f],
            market_enabled: true,
            ..Default::default()
        };
        hot.apply_fixture_dates(&cal, 1);
        for idx in 0..20 {
            hot.promote(idx, 30, "NPC", &world).unwrap();
        }
        cal.fixtures[0].day = 147;
        hot.apply_fixture_dates(&cal, 1);
        cold.apply_fixture_dates(&cal, 1);
        for idx in 0..20 {
            let a = hot.promote(idx, 30, "NPC", &world).unwrap();
            let b = cold.promote(idx, 30, "NPC", &world).unwrap();
            assert_eq!(a.current, b.current);
            assert_eq!(a.energy, b.energy);
            assert_eq!(a.injury_weeks, b.injury_weeks);
        }
    }
    #[test]
    fn dated_market_league_workload_ids_are_world_unique() {
        use goat_core::state::{reduce, Intent, WorldState};
        let world = crate::world::WorldGenesis::generate(42);
        let mut state = WorldState::new();
        state.world_seed = 42;
        state.dated_calendar = true;
        state.career_base_year = 2023;
        state.season_number = 1;
        state = reduce(
            state,
            Intent::EnableDatedCompetitions,
            &mut goat_rng::GoatRng::new(0),
        );
        let mut session = crate::session::SimulationSession::new();
        let state = session.prepare_competitions(state, &world).unwrap();
        let cal = state.competition_calendar.unwrap();
        let ids = cal
            .fixtures
            .iter()
            .filter(|f| f.competition == 1)
            .map(|f| f.workload_id)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids.len(), 60 * 380);
    }
}
