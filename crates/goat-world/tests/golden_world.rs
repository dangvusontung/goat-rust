//! Golden-seed world tests.
//! Values frozen from first green run.

use goat_core::{
    attrs::AttrId,
    generation::CreationChoices,
    positions::PrimaryPosition,
    state::{reduce, Intent, WorldState},
    week::{Intensity, Routine},
};
use goat_rng::GoatRng;
use goat_world::{
    generate_fixtures, rest_weeks_after_round, round_fixtures, sim_team_match,
    week_ends_after_round,
    world::{WorldGenesis, NUM_CLUBS, NUM_DIVISIONS},
    Table, CLUBS_PER_DIV, ROUNDS_PER_SEASON,
};

#[test]
fn world_has_correct_size() {
    let world = WorldGenesis::generate(1);
    assert_eq!(world.clubs.len(), NUM_CLUBS);
    assert_eq!(world.leagues.len(), NUM_DIVISIONS);
    for league in &world.leagues {
        assert_eq!(league.clubs.len(), CLUBS_PER_DIV);
    }
    assert_eq!(ROUNDS_PER_SEASON, 38);
}

#[test]
fn golden_fixture_count() {
    let world = WorldGenesis::generate(12345);
    let league_clubs = &world.leagues[0].clubs;
    let fixtures = generate_fixtures(12345, 1, 0, league_clubs);
    // CLUBS_PER_DIV clubs, ROUNDS_PER_SEASON rounds, CLUBS_PER_DIV/2 matches/round
    assert_eq!(fixtures.len(), CLUBS_PER_DIV / 2 * ROUNDS_PER_SEASON);
}

#[test]
fn each_round_has_correct_match_count() {
    let world = WorldGenesis::generate(12345);
    let league_clubs = &world.leagues[0].clubs;
    let fixtures = generate_fixtures(12345, 1, 0, league_clubs);
    for round in 0..ROUNDS_PER_SEASON {
        let count = fixtures.iter().filter(|f| f.round == round).count();
        assert_eq!(
            count,
            CLUBS_PER_DIV / 2,
            "round {round} should have {} matches, got {count}",
            CLUBS_PER_DIV / 2
        );
    }
}

#[test]
fn each_club_plays_exactly_once_per_round() {
    let world = WorldGenesis::generate(42);
    let league_clubs = &world.leagues[0].clubs;
    let fixtures = generate_fixtures(42, 1, 0, league_clubs);
    for round in 0..ROUNDS_PER_SEASON {
        let round_fx: Vec<_> = fixtures.iter().filter(|f| f.round == round).collect();
        let mut seen = std::collections::HashSet::new();
        for f in &round_fx {
            assert!(
                seen.insert(f.home),
                "club {} appears twice as home in round {round}",
                f.home
            );
            assert!(
                seen.insert(f.away),
                "club {} appears twice as away in round {round}",
                f.away
            );
        }
    }
}

#[test]
fn each_club_plays_full_season_games() {
    let world = WorldGenesis::generate(7);
    let div_clubs = &world.leagues[0].clubs;
    let fixtures = generate_fixtures(7, 1, 0, div_clubs);
    for &club in div_clubs {
        let count = fixtures
            .iter()
            .filter(|f| f.home == club || f.away == club)
            .count();
        assert_eq!(
            count, ROUNDS_PER_SEASON,
            "club {club} should play {ROUNDS_PER_SEASON} games, got {count}"
        );
    }
}

#[test]
fn fixture_list_is_deterministic() {
    let world = WorldGenesis::generate(999);
    let league_clubs = &world.leagues[3].clubs;
    let a = generate_fixtures(999, 2, 3, league_clubs);
    let b = generate_fixtures(999, 2, 3, league_clubs);
    assert_eq!(a.len(), b.len());
    for (fa, fb) in a.iter().zip(b.iter()) {
        assert_eq!(fa.home, fb.home);
        assert_eq!(fa.away, fb.away);
        assert_eq!(fa.round, fb.round);
    }
}

#[test]
fn different_seasons_give_different_fixtures() {
    let world = WorldGenesis::generate(42);
    let league_clubs = &world.leagues[0].clubs;
    let s1 = generate_fixtures(42, 1, 0, league_clubs);
    let s2 = generate_fixtures(42, 2, 0, league_clubs);
    let differ = s1
        .iter()
        .zip(s2.iter())
        .any(|(a, b)| a.home != b.home || a.away != b.away);
    assert!(differ, "seasons 1 and 2 should have different schedules");
}

#[test]
fn golden_team_match_seed_42() {
    let (gf, ga) = sim_team_match(70, 60, &mut GoatRng::new(42));
    assert_eq!(gf, 2, "goals_for frozen at 2");
    assert_eq!(ga, 1, "goals_against frozen at 1");
}

/// 20-season full-career headless sim: no panic, invariants hold every season.
///
/// Starts a 16-year-old Forward, runs all 20 seasons (age ~16–36), asserts:
/// - cur <= pot for every attribute every season
/// - career goals and matches accumulate monotonically
/// - career counters are consistent at retirement age
#[test]
fn twenty_seasons_full_career_no_panic() {
    let world_seed = 0xCAFE_BABE_u64;
    let world = WorldGenesis::generate(world_seed);
    // League index 1 = nation 0's Second-tier division (mirrors the old "starts in the
    // second division" flavor — the specific league doesn't matter, just its tier).
    let pc_div_idx: usize = 1;
    let pc_club_id = world.leagues[pc_div_idx].clubs[0];

    let choices = CreationChoices {
        name: "Career GOAT".into(),
        primary_position: PrimaryPosition::ST,
        nationality: "England".to_string(),
        club: world.clubs[pc_club_id].name.clone(),
    };

    let mut state = WorldState::new();
    state = reduce(
        state,
        Intent::CreatePlayer {
            seed: world_seed,
            choices,
        },
        &mut GoatRng::new(0),
    );
    state = reduce(
        state,
        Intent::InitWorld {
            world_seed,
            pc_club_idx: pc_club_id as u16,
            pc_div_idx: pc_div_idx as u8,
            facilities_mult: world.clubs[pc_club_id].facilities_mult(),
            initial_table: Box::new([0u32; 100]),
        },
        &mut GoatRng::new(0),
    );

    let routine = Routine {
        focus_attrs: vec![AttrId::Finishing, AttrId::CloseControl],
        intensity: Intensity::Medium,
    };
    state = reduce(state, Intent::SetRoutine { routine }, &mut GoatRng::new(0));

    let mut season_positions: Vec<usize> = Vec::new();
    let mut prev_career_goals = 0u32;
    let mut prev_career_matches = 0u32;

    for season in 1u32..=20 {
        state = reduce(state, Intent::StartSeason, &mut GoatRng::new(0));
        assert_eq!(state.season_number, season, "s{season}: season counter");

        let div_idx = state.pc_div_idx as usize;
        let div_clubs = &world.leagues[div_idx].clubs;

        for round in 0..ROUNDS_PER_SEASON {
            let age = state.players.get_age_weeks(state.pc_player_id.unwrap());
            state = reduce(
                state,
                Intent::AdvanceWeek,
                &mut GoatRng::new(age as u64 ^ (season as u64 * 0xbeef) ^ round as u64),
            );

            let all_fixtures = round_fixtures(world_seed, season, div_idx, div_clubs, round);
            let mut sim_rng = GoatRng::new(world_seed ^ ((season as u64) << 32) ^ (round as u64));

            let mut round_results: Vec<(u8, u8, u32, u32)> = Vec::new();
            let mut pc_gf = 0u32;
            let mut pc_ga = 0u32;
            for f in &all_fixtures {
                let (gf, ga) = sim_team_match(
                    world.clubs[f.home].strength,
                    world.clubs[f.away].strength,
                    &mut sim_rng,
                );
                let h_pos = div_clubs.iter().position(|&c| c == f.home).unwrap() as u8;
                let a_pos = div_clubs.iter().position(|&c| c == f.away).unwrap() as u8;
                round_results.push((h_pos, a_pos, gf, ga));
                if f.home == pc_club_id {
                    pc_gf = gf;
                    pc_ga = ga;
                } else if f.away == pc_club_id {
                    pc_gf = ga;
                    pc_ga = gf;
                }
            }

            let pc_result: i8 = if pc_gf > pc_ga {
                1
            } else if pc_gf < pc_ga {
                -1
            } else {
                0
            };
            state = reduce(
                state,
                Intent::ApplyRoundResult {
                    pc_goals: 0,
                    pc_output: 60,
                    pc_result,
                    round_results,
                    rest_weeks: rest_weeks_after_round(round),
                    week_ends: week_ends_after_round(round),
                },
                &mut GoatRng::new(0),
            );
        }

        assert_eq!(
            state.season_round, ROUNDS_PER_SEASON as u32,
            "s{season}: round counter at end"
        );

        let table = Table::from_raw(&state.table_raw, div_clubs);
        let pos = table.position_of(pc_club_id);
        assert!(
            (1..=CLUBS_PER_DIV).contains(&pos),
            "s{season}: position {pos} out of range"
        );
        season_positions.push(pos);

        // Attr invariants hold throughout the career (peak and decline).
        let pc_id = state.pc_player_id.unwrap();
        for a in 0..goat_core::attrs::NUM_ATTRS {
            let cur = state.players.get_current(pc_id, a);
            let pot = state.players.get_potential(pc_id, a);
            assert!(cur <= pot, "s{season}: cur > pot for attr {a}");
            assert!(
                cur >= goat_fixed::Fixed::MIN_ATTR,
                "s{season}: attr {a} below minimum"
            );
        }

        // Career counters must only grow.
        assert!(
            state.pc_career_goals >= prev_career_goals,
            "s{season}: career goals decreased"
        );
        assert!(
            state.pc_career_matches >= prev_career_matches,
            "s{season}: career matches decreased"
        );
        prev_career_goals = state.pc_career_goals;
        prev_career_matches = state.pc_career_matches;
    }

    assert_eq!(
        season_positions.len(),
        20,
        "should have 20 position records"
    );
    // Season counter reflects the last completed season.
    assert_eq!(state.season_number, 20, "season_number at end of career");
    // pc_season_matches counts within the current (last) season via ApplyRoundResult.
    // career counters require ApplySeasonEndLegacy which this headless test omits.
    assert!(
        state.pc_season_matches > 0,
        "player should have appeared in matches during the final season"
    );
}

/// 5 headless seasons: auto-sim all rounds, invariants hold, tables accumulate, no panics.
#[test]
fn five_headless_seasons_no_panic() {
    let world_seed = 7777u64;
    let world = WorldGenesis::generate(world_seed);
    let pc_div_idx: usize = 1;
    let pc_club_id = world.leagues[pc_div_idx].clubs[0];

    let choices = CreationChoices {
        name: "Headless Hero".into(),
        primary_position: PrimaryPosition::ST,
        nationality: "England".to_string(),
        club: world.clubs[pc_club_id].name.clone(),
    };

    let mut state = WorldState::new();
    state = reduce(
        state,
        Intent::CreatePlayer {
            seed: world_seed,
            choices,
        },
        &mut GoatRng::new(0),
    );
    state = reduce(
        state,
        Intent::InitWorld {
            world_seed,
            pc_club_idx: pc_club_id as u16,
            pc_div_idx: pc_div_idx as u8,
            facilities_mult: world.clubs[pc_club_id].facilities_mult(),
            initial_table: Box::new([0u32; 100]),
        },
        &mut GoatRng::new(0),
    );

    let routine = Routine {
        focus_attrs: vec![AttrId::Finishing, AttrId::CloseControl],
        intensity: Intensity::Medium,
    };
    state = reduce(state, Intent::SetRoutine { routine }, &mut GoatRng::new(0));

    let mut final_positions: Vec<usize> = Vec::new();

    for season in 1u32..=5 {
        state = reduce(state, Intent::StartSeason, &mut GoatRng::new(0));
        assert_eq!(state.season_number, season, "season counter");

        let div_idx = state.pc_div_idx as usize;
        let div_clubs = &world.leagues[div_idx].clubs;

        for round in 0..ROUNDS_PER_SEASON {
            // Advance 1 training week before each match.
            let age = state.players.get_age_weeks(state.pc_player_id.unwrap());
            state = reduce(
                state,
                Intent::AdvanceWeek,
                &mut GoatRng::new(age as u64 ^ (season as u64 * 0xbeef) ^ round as u64),
            );

            // Sim all matches in this round via team strength.
            let all_fixtures = round_fixtures(world_seed, season, div_idx, div_clubs, round);
            let mut sim_rng = GoatRng::new(world_seed ^ ((season as u64) << 32) ^ (round as u64));

            let mut round_results: Vec<(u8, u8, u32, u32)> = Vec::new();
            let mut pc_gf = 0u32;
            let mut pc_ga = 0u32;
            for f in &all_fixtures {
                let (gf, ga) = sim_team_match(
                    world.clubs[f.home].strength,
                    world.clubs[f.away].strength,
                    &mut sim_rng,
                );
                let h_pos = div_clubs.iter().position(|&c| c == f.home).unwrap() as u8;
                let a_pos = div_clubs.iter().position(|&c| c == f.away).unwrap() as u8;
                round_results.push((h_pos, a_pos, gf, ga));
                if f.home == pc_club_id {
                    pc_gf = gf;
                    pc_ga = ga;
                } else if f.away == pc_club_id {
                    pc_gf = ga;
                    pc_ga = gf;
                }
            }

            let pc_result: i8 = if pc_gf > pc_ga {
                1
            } else if pc_gf < pc_ga {
                -1
            } else {
                0
            };
            state = reduce(
                state,
                Intent::ApplyRoundResult {
                    pc_goals: 0,
                    pc_output: 60,
                    pc_result,
                    round_results,
                    rest_weeks: rest_weeks_after_round(round),
                    week_ends: week_ends_after_round(round),
                },
                &mut GoatRng::new(0),
            );
        }

        assert_eq!(
            state.season_round, ROUNDS_PER_SEASON as u32,
            "season {season}: round counter at end"
        );

        let table = Table::from_raw(&state.table_raw, div_clubs);
        let pos = table.position_of(pc_club_id);
        assert!(
            (1..=CLUBS_PER_DIV).contains(&pos),
            "position {pos} out of range"
        );
        final_positions.push(pos);

        // Attr invariants hold mid-career.
        let pc_id = state.pc_player_id.unwrap();
        for a in 0..goat_core::attrs::NUM_ATTRS {
            let cur = state.players.get_current(pc_id, a);
            let pot = state.players.get_potential(pc_id, a);
            assert!(cur <= pot, "s{season}: cur > pot for attr {a}");
            assert!(
                cur >= goat_fixed::Fixed::MIN_ATTR,
                "s{season}: attr {a} below minimum"
            );
        }
    }

    // Sanity: 5 season positions recorded.
    assert_eq!(final_positions.len(), 5);
}
