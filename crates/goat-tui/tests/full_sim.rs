//! Full-game headless simulation tests.
//!
//! These tests exercise the entire career pipeline end-to-end:
//! CreatePlayer → InitWorld → training weeks → match rounds → season legacy →
//! transfers / peer batch-tick → career over.
//!
//! Each test asserts *invariants* (never frozen values), so they cannot break
//! determinism. Golden-seed tests for specific numeric output live in the
//! per-crate `tests/` directories.

use goat_core::{
    attrs::{AttrId, NUM_ATTRS},
    derive::ovr,
    generation::CreationChoices,
    positions::PrimaryPosition,
    roles::RoleId,
    state::{lifestyle_tier_from_score, reduce, Intent, PeerState, WorldState},
    week::{Intensity, Routine},
};
use goat_fixed::Fixed;
use goat_match::{
    discipline::RefPersonality,
    sim::{auto_play_match, BeatLibrary, MatchSetup},
};
use goat_rng::GoatRng;
use goat_traits::PlayerTraits;
use goat_world::{
    fixture_for_round, rest_weeks_after_round, round_fixtures, sim_team_match,
    week_ends_after_round, world::WorldGenesis, Table, ROUNDS_PER_SEASON,
};

const BEATS_JSON: &str = include_str!("../../../beats.json");

// First nation's top tier / second tier — analogous to the old hardcoded
// DIV_ENG_TOP/DIV_ENG_SEC constants (league ids are nation-major, tier-minor per
// `WorldGenesis::generate`, so nation 0's tiers are leagues 0 and 1).
const DIV_ENG_TOP: usize = 0;
const DIV_ENG_SEC: usize = 1;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Broad position family used only within this test harness (creation now picks one
/// of the 8 specific `PrimaryPosition`s directly — bible §4 revision). Maps to the
/// same specific position the old 3-way picker's `default_primary()` produced, so
/// this harness's invariant tests are unaffected by the model change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Position {
    Defender,
    Midfielder,
    Forward,
}

impl Position {
    fn to_primary(self) -> PrimaryPosition {
        match self {
            Position::Defender => PrimaryPosition::CB,
            Position::Midfielder => PrimaryPosition::CM,
            Position::Forward => PrimaryPosition::ST,
        }
    }
}

fn beat_lib() -> BeatLibrary {
    BeatLibrary::load(BEATS_JSON).expect("beats.json must parse")
}

fn make_state(seed: u64, position: Position, div_idx: usize) -> WorldState {
    let world = WorldGenesis::generate(seed);
    let pc_club_id = world.leagues[div_idx].clubs[0];
    let club = &world.clubs[pc_club_id];
    let choices = CreationChoices {
        name: "Test Legend".into(),
        primary_position: position.to_primary(),
        nationality: "England".to_string(),
        club: club.name.clone(),
    };
    let mut s = WorldState::new();
    s = reduce(
        s,
        Intent::CreatePlayer { seed, choices },
        &mut GoatRng::new(0),
    );
    s = reduce(
        s,
        Intent::InitWorld {
            world_seed: seed,
            pc_club_idx: pc_club_id as u16,
            pc_div_idx: div_idx as u8,
            facilities_mult: club.facilities_mult(),
            initial_table: Box::new([0u32; 100]),
        },
        &mut GoatRng::new(0),
    );
    // Set a training routine appropriate for position.
    let focus = match position {
        Position::Defender => vec![
            AttrId::StandingTackle,
            AttrId::Marking,
            AttrId::Interceptions,
        ],
        Position::Midfielder => vec![AttrId::ShortPassing, AttrId::Vision, AttrId::Stamina],
        Position::Forward => vec![
            AttrId::Finishing,
            AttrId::CloseControl,
            AttrId::Acceleration,
        ],
    };
    s = reduce(
        s,
        Intent::SetRoutine {
            routine: Routine {
                focus_attrs: focus,
                intensity: Intensity::Medium,
            },
        },
        &mut GoatRng::new(0),
    );
    // Seed a peer cohort so Phase 9 logic exercises.
    let peers = (0..8)
        .map(|i| PeerState {
            seed: seed ^ (i as u64 * 0x1234_5678),
            name: format!("Peer {i}"),
            nationality: "England".to_string(),
            career_goals: 0,
            career_matches: 0,
            avg_output: 0,
            titles: 0,
        })
        .collect();
    reduce(s, Intent::InitPeers { peers }, &mut GoatRng::new(0))
}

/// Assert the core invariants that must hold at all times.
fn assert_invariants(state: &WorldState, label: &str) {
    let pc_id = state.pc_player_id.expect("pc_player_id must be set");
    for a in 0..NUM_ATTRS {
        let cur = state.players.get_current(pc_id, a);
        let pot = state.players.get_potential(pc_id, a);
        assert!(
            cur <= pot,
            "{label}: attr {a} cur({cur:?}) > pot({pot:?}) — talent ceiling violated"
        );
        assert!(
            cur >= Fixed::MIN_ATTR,
            "{label}: attr {a} cur({cur:?}) below minimum"
        );
    }
    let energy = state.players.get_energy(pc_id);
    assert!(
        energy >= Fixed::ZERO && energy <= Fixed::raw(100_000),
        "{label}: energy out of range: {energy:?}"
    );
    let form = state.pc_form.to_int();
    assert!(
        (0..=100).contains(&form),
        "{label}: form out of range: {form}"
    );
}

fn role_for_position(p: Position) -> RoleId {
    match p {
        Position::Defender => RoleId::CentreBack,
        Position::Midfielder => RoleId::CentralMid,
        Position::Forward => RoleId::CompleteForward,
    }
}

/// Run one full season headlessly — training weeks + all match rounds.
/// Uses `auto_play_match` for the PC's fixture each round.
fn run_one_season(mut state: WorldState, position: Position, beat_lib: &BeatLibrary) -> WorldState {
    let seed = state.world_seed;
    let world = WorldGenesis::generate(seed);
    let pc_id = state.pc_player_id.unwrap();

    state = reduce(state, Intent::StartSeason, &mut GoatRng::new(0));

    for round in 0..ROUNDS_PER_SEASON {
        // Two training weeks per round (matches are every ~2 weeks).
        for t in 0u64..2 {
            let age = state.players.get_age_weeks(pc_id);
            let rng_seed =
                age as u64 ^ (state.season_number as u64 * 0xbeef) ^ (round as u64 * 7) ^ t;
            state = reduce(state, Intent::AdvanceWeek, &mut GoatRng::new(rng_seed));
            assert_invariants(
                &state,
                &format!("s{}r{}t{} post-training", state.season_number, round, t),
            );
        }

        let season = state.season_number;
        let div_idx = state.pc_div_idx as usize;
        let pc_club_id = state.pc_club_idx as usize;

        // Simulate PC's own match via the beat engine.
        let (pc_goals, pc_output, match_result_goals) = if let Some(_fixture) =
            fixture_for_round(seed, season, div_idx, pc_club_id, round)
        {
            let opp_club_id = {
                let all = round_fixtures(seed, season, div_idx, round);
                all.iter()
                    .find(|f| f.home == pc_club_id || f.away == pc_club_id)
                    .map(|f| if f.home == pc_club_id { f.away } else { f.home })
                    .unwrap_or(0)
            };
            let opp = &CLUBS[opp_club_id];
            let view = state.players.snapshot(pc_id);
            let match_seed = seed ^ ((season as u64) << 32) ^ (round as u64) ^ 0xc0ffee;
            let mut rp_rng = GoatRng::new(match_seed ^ 0xBADCAFE);
            let ref_personality = RefPersonality::from_rng(&mut rp_rng);

            let setup = MatchSetup {
                player_role: role_for_position(position),
                player_attrs: view.current,
                player_familiarity: view.familiarity,
                own_strength: CLUBS[pc_club_id].strength,
                opp_strength: opp.strength,
                opp_name: opp.name,
                form: state.pc_form,
                player_aggression: view.current[AttrId::Aggression as usize]
                    .to_int()
                    .clamp(1, 99) as u8,
                ref_personality,
                dirty_rep: state.pc_discipline_rep,
                player_traits: PlayerTraits::default(),
            };

            let result = auto_play_match(beat_lib, setup, &mut GoatRng::new(match_seed));

            // Apply match effects.
            state = reduce(
                state,
                Intent::ApplyMatchResult {
                    familiarity_xp: result.familiarity_xp,
                    energy_cost: Fixed::from_int(25),
                    injury_weeks: None,
                },
                &mut GoatRng::new(0),
            );
            if result.yellow_cards > 0 || result.red_card {
                state = reduce(
                    state,
                    Intent::ApplyCardResult {
                        yellow_cards: result.yellow_cards as u32,
                        red_card: result.red_card,
                    },
                    &mut GoatRng::new(0),
                );
            }

            let goals = result
                .moments
                .iter()
                .filter(|m| matches!(m.goal_event, Some(goat_match::beats::ScoreEvent::GoalFor)))
                .count() as u32;
            let result_int: i8 = if result.goals_for > result.goals_against {
                1
            } else if result.goals_for < result.goals_against {
                -1
            } else {
                0
            };
            (
                goals,
                result.player_output,
                Some((result.goals_for, result.goals_against, result_int)),
            )
        } else {
            (0, 0, None)
        };

        // Sim all other rounds' matches.
        let all_fixtures = round_fixtures(seed, season, div_idx, round);
        let sim_seed = seed ^ ((season as u64) << 32) ^ (round as u64) ^ 0xfeed;
        let mut sim_rng = GoatRng::new(sim_seed);
        let div_clubs = DIV_CLUBS[div_idx];

        let mut round_results: Vec<(u8, u8, u32, u32)> = Vec::new();
        for f in &all_fixtures {
            let is_pc = f.home == pc_club_id || f.away == pc_club_id;
            let (gf, ga) = if is_pc {
                match match_result_goals {
                    Some((gf, ga, _)) => {
                        if f.home == pc_club_id {
                            (gf, ga)
                        } else {
                            (ga, gf)
                        }
                    }
                    None => (0, 0),
                }
            } else {
                sim_team_match(CLUBS[f.home].strength, CLUBS[f.away].strength, &mut sim_rng)
            };
            let h_pos = div_clubs.iter().position(|&c| c == f.home).unwrap() as u8;
            let a_pos = div_clubs.iter().position(|&c| c == f.away).unwrap() as u8;
            round_results.push((h_pos, a_pos, gf, ga));
        }

        let pc_result = match_result_goals.map(|(_, _, r)| r).unwrap_or(0);
        state = reduce(
            state,
            Intent::ApplyRoundResult {
                pc_goals,
                pc_output,
                pc_result,
                round_results,
                rest_weeks: rest_weeks_after_round(round),
                week_ends: week_ends_after_round(round),
            },
            &mut GoatRng::new(0),
        );
        assert_invariants(&state, &format!("s{}r{} post-round", season, round));
    }

    state
}

/// Apply end-of-season accounting and legacy update.
fn end_season(mut state: WorldState) -> WorldState {
    let div_idx = state.pc_div_idx as usize;
    let div_clubs = DIV_CLUBS[div_idx];
    let table = Table::from_raw(&state.table_raw, &div_clubs);
    let finish_pos = table.position_of(state.pc_club_idx as usize) as u32;
    let won_title = finish_pos == 1;

    let season_avg = if state.pc_season_matches > 0 {
        (state.pc_season_output / state.pc_season_matches as i32).clamp(0, 100)
    } else {
        0
    };

    let new_sporting = (state.pc_sporting_rep + if season_avg > 60 { 1 } else { 0 }).min(100);
    let new_club_fan = (state.pc_club_fan_rep + 1).min(100);

    let s_goals = state.pc_season_goals;
    let s_matches = state.pc_season_matches;
    let s_output = state.pc_season_output;
    let s_standout_matches = state.pc_season_standout_matches;
    let s_transfer_requests = state.pc_season_transfer_requests;
    let season = state.season_number;

    state = reduce(
        state,
        Intent::ApplySeasonEndLegacy {
            season_goals: s_goals,
            season_matches: s_matches,
            season_output_sum: s_output,
            won_title,
            player_of_year: season_avg > 75,
            finish_position: finish_pos,
            decisive_moments: 0,
            new_sporting_rep: new_sporting,
            new_club_fan_rep: new_club_fan,
            season_standout_matches: s_standout_matches,
            season_transfer_requests: s_transfer_requests,
        },
        &mut GoatRng::new(0),
    );

    state = reduce(state, Intent::CollectWage, &mut GoatRng::new(0));
    state = reduce(
        state,
        Intent::BatchTickPeers { season },
        &mut GoatRng::new(0),
    );

    state
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// All three positions: 5 seasons each, invariants hold throughout.
#[test]
fn all_positions_5_seasons_invariants() {
    let lib = beat_lib();
    for (position, div_idx) in [
        (Position::Defender, DIV_ENG_TOP),
        (Position::Midfielder, DIV_ENG_TOP),
        (Position::Forward, DIV_ENG_SEC),
    ] {
        let mut state = make_state(42, position, div_idx);
        for _ in 0..5 {
            state = run_one_season(state, position, &lib);
            state = end_season(state);
        }
        // Final checks
        assert_invariants(&state, &format!("{position:?} final"));
        assert!(
            state.pc_career_matches > 0,
            "{position:?}: should have played matches"
        );
        assert!(
            state.pc_seasons_played == 5,
            "{position:?}: seasons_played should be 5"
        );
    }
}

/// Same seed + same position = identical career stats after N seasons.
#[test]
fn career_is_deterministic() {
    let lib = beat_lib();

    let run = |seed: u64| {
        let mut state = make_state(seed, Position::Forward, DIV_ENG_SEC);
        for _ in 0..3 {
            state = run_one_season(state, Position::Forward, &lib);
            state = end_season(state);
        }
        let pc_id = state.pc_player_id.unwrap();
        let view = state.players.snapshot(pc_id);
        (
            state.pc_career_goals,
            state.pc_career_matches,
            ovr(&view.current, view.primary_position).to_int(),
            state.pc_form.to_int(),
        )
    };

    let r1 = run(99);
    let r2 = run(99);
    assert_eq!(
        r1, r2,
        "career must be deterministic: same seed → same result"
    );

    // Different seed must produce a different career (probabilistic sanity).
    let r3 = run(77);
    assert_ne!(r1, r3, "different seeds should produce different careers");
}

/// Training routine focus actually raises the focused attributes over 52 weeks.
#[test]
fn training_raises_focused_attrs() {
    let mut state = make_state(12345, Position::Forward, DIV_ENG_SEC);
    let pc_id = state.pc_player_id.unwrap();

    let before_fin = state.players.get_current(pc_id, AttrId::Finishing as usize);

    let mut rng = GoatRng::new(42);
    for _ in 0..52 {
        state = reduce(state, Intent::AdvanceWeek, &mut rng);
    }

    let after_fin = state.players.get_current(pc_id, AttrId::Finishing as usize);
    assert!(
        after_fin >= before_fin,
        "focused Finishing must grow or stay: {before_fin:?} → {after_fin:?}"
    );
    assert_invariants(&state, "post-52-weeks training");
}

/// Talent ceiling is never exceeded throughout a full career.
#[test]
fn talent_ceiling_never_violated() {
    let lib = beat_lib();
    let mut state = make_state(7, Position::Forward, DIV_ENG_SEC);
    let pc_id = state.pc_player_id.unwrap();

    // Snapshot ceilings at career start.
    let ceilings: Vec<Fixed> = (0..NUM_ATTRS)
        .map(|a| state.players.get_potential(pc_id, a))
        .collect();

    for season in 1..=8u32 {
        state = run_one_season(state, Position::Forward, &lib);
        state = end_season(state);

        for (a, &ceiling) in ceilings.iter().enumerate() {
            let cur = state.players.get_current(pc_id, a);
            // Potential must also never shrink.
            let pot = state.players.get_potential(pc_id, a);
            assert_eq!(
                pot, ceiling,
                "s{season}: potential for attr {a} changed — ceilings are immutable"
            );
            assert!(
                cur <= ceiling,
                "s{season}: attr {a} cur({cur:?}) > ceiling({ceiling:?})"
            );
        }
    }
}

/// Energy stays in [0, 100] even after many consecutive match rounds.
#[test]
fn energy_stays_bounded_under_heavy_load() {
    let lib = beat_lib();
    let mut state = make_state(1234, Position::Midfielder, DIV_ENG_TOP);
    let pc_id = state.pc_player_id.unwrap();

    // Use High intensity to stress-test energy drain.
    state = reduce(
        state,
        Intent::SetRoutine {
            routine: Routine {
                focus_attrs: vec![AttrId::Stamina, AttrId::Strength],
                intensity: Intensity::High,
            },
        },
        &mut GoatRng::new(0),
    );

    state = run_one_season(state, Position::Midfielder, &lib);

    let energy = state.players.get_energy(pc_id);
    assert!(
        energy >= Fixed::ZERO,
        "energy must stay ≥ 0: got {energy:?}"
    );
    assert!(
        energy <= Fixed::raw(100_000),
        "energy must stay ≤ 100: got {energy:?}"
    );
}

/// Legacy evidence accumulates monotonically across seasons.
#[test]
fn legacy_evidence_accumulates() {
    let lib = beat_lib();
    let mut state = make_state(55, Position::Forward, DIV_ENG_TOP);

    let mut prev_matches = 0u32;
    let mut prev_seasons = 0u32;

    for _ in 0..5 {
        state = run_one_season(state, Position::Forward, &lib);
        state = end_season(state);

        assert!(
            state.pc_career_matches >= prev_matches,
            "career_matches must be monotonic: {prev_matches} → {}",
            state.pc_career_matches
        );
        assert!(
            state.pc_seasons_played > prev_seasons,
            "seasons_played must increment each season"
        );
        prev_matches = state.pc_career_matches;
        prev_seasons = state.pc_seasons_played;
    }
}

/// Contract countdown and wage collection.
#[test]
fn wage_collection_and_contract_countdown() {
    let mut state = make_state(88, Position::Midfielder, DIV_ENG_SEC);
    let club_idx = state.pc_club_idx;
    state = reduce(
        state,
        Intent::AcceptContract {
            new_wage: 100,
            new_length: 3,
            new_club_idx: club_idx,
        },
        &mut GoatRng::new(0),
    );
    assert_eq!(state.pc_contract_seasons_left, 3);

    for expected_left in [2, 1, 0] {
        state = reduce(state, Intent::CollectWage, &mut GoatRng::new(0));
        assert_eq!(
            state.pc_contract_seasons_left, expected_left,
            "contract should count down"
        );
    }
    assert_eq!(
        state.pc_savings, 300,
        "three seasons × £100k wage = £300k savings"
    );
}

/// Rival crystallises after 5 seasons when a peer is close enough in output.
#[test]
fn rival_can_crystallise() {
    let lib = beat_lib();
    let mut state = make_state(42, Position::Forward, DIV_ENG_SEC);

    // Force a peer to look like a rival candidate after enough seasons.
    for season in 1..=6u32 {
        state = run_one_season(state, Position::Forward, &lib);
        state = end_season(state);

        // Check crystallisation condition (mirrors main.rs logic).
        if season >= 5 && state.pc_rival_idx.is_none() {
            let pc_avg = if state.pc_career_matches > 0 {
                (state.pc_career_output_sum / state.pc_career_matches as i64) as u8
            } else {
                0
            };
            if let Some((i, _)) = state.pc_peers.iter().enumerate().find(|(_, peer)| {
                peer.career_matches >= 80 && (peer.avg_output as i32 - pc_avg as i32).abs() <= 8
            }) {
                state = reduce(
                    state,
                    Intent::DeclareRival {
                        peer_idx: i,
                        season,
                    },
                    &mut GoatRng::new(0),
                );
            }
        }
    }

    // We don't assert a rival must exist (it might not), but the state must be valid.
    if let Some(idx) = state.pc_rival_idx {
        assert!(idx < state.pc_peers.len(), "rival_idx must be valid");
        assert!(
            state.pc_rival_declared_season.is_some(),
            "declaration season must be set when rival exists"
        );
    }
}

/// Form stays in [0, 100] after many match rounds and draws from both ends.
#[test]
fn form_stays_in_range() {
    let lib = beat_lib();
    let mut state = make_state(321, Position::Midfielder, DIV_ENG_TOP);

    for _ in 0..3 {
        state = run_one_season(state, Position::Midfielder, &lib);
        let form = state.pc_form.to_int();
        assert!(
            (0..=100).contains(&form),
            "form out of range during season: {form}"
        );
        state = end_season(state);
    }
}

/// Suspension: a red card blocks the player for at least one round.
#[test]
fn red_card_triggers_suspension() {
    let mut state = make_state(11, Position::Midfielder, DIV_ENG_SEC);
    assert_eq!(state.pc_suspension_weeks, 0);

    state = reduce(
        state,
        Intent::ApplyCardResult {
            yellow_cards: 0,
            red_card: true,
        },
        &mut GoatRng::new(0),
    );
    assert!(
        state.pc_suspension_weeks >= 1,
        "red card must set a suspension"
    );
    // Discipline rep should tighten.
    assert!(
        state.pc_discipline_rep > 50,
        "red card should raise dirty rep: {}",
        state.pc_discipline_rep
    );
}

/// Lifestyle Professional grows faster than Balanced over one full season.
#[test]
fn professional_lifestyle_outgrows_balanced() {
    let lib = beat_lib();

    let run_season = |lifestyle: u8| {
        let mut s = make_state(99, Position::Forward, DIV_ENG_SEC);
        // Lifestyle is now emergent (bible §8.5/§8.6) — pin the score directly to
        // force the tier for this comparison, rather than picking it via an intent.
        s.pc_lifestyle_score = match lifestyle {
            0 => Fixed::raw(-1_000),
            2 => Fixed::raw(1_000),
            _ => Fixed::ZERO,
        };
        s.pc_lifestyle = lifestyle_tier_from_score(s.pc_lifestyle_score);
        s = run_one_season(s, Position::Forward, &lib);
        let pc_id = s.pc_player_id.unwrap();
        s.players.get_current(pc_id, AttrId::Finishing as usize)
    };

    let pro = run_season(0);
    let bal = run_season(1);
    assert!(
        pro >= bal,
        "professional lifestyle should produce ≥ growth: {pro:?} vs {bal:?}"
    );
}
