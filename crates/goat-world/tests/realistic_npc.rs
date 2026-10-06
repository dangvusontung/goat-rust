use goat_core::{
    discipline::NpcCardEvent,
    state::{reduce, Intent, WorldState},
};
use goat_rng::GoatRng;
use goat_world::{calendar::dated_fixture_day, session::SimulationSession, world::WorldGenesis};

fn state() -> WorldState {
    let mut state = WorldState::new();
    state.dated_calendar = true;
    state.realistic_npc = true;
    state.world_seed = 42;
    state.career_base_year = 2023;
    state.season_number = 1;
    state.pc_div_idx = 59;
    state.pc_club_idx = 1180;
    state
}
#[test]
fn red_card_bans_three_fixtures_and_cold_replay_preserves_next_results() {
    let world = WorldGenesis::generate(42);
    let mut state = state();
    let mut warm = SimulationSession::new();
    let calendar = state.chronology().unwrap();
    state.pc_epoch_day = dated_fixture_day(calendar, 1, 0);
    state = warm.advance_deep(state, &world);
    assert!(!state.npc_cards.is_empty());
    let player = state
        .orbit_records
        .iter()
        .flat_map(|r| r.credits.iter())
        .find(|c| {
            state
                .npc_match_loads
                .iter()
                .any(|l| l.pop_idx == c.pop_idx && l.minutes == 90)
                && !state
                    .npc_cards
                    .iter()
                    .any(|e| e.pop_idx == c.pop_idx && e.kind > 0)
        })
        .unwrap()
        .pop_idx;
    let load = *state
        .npc_match_loads
        .iter()
        .find(|l| l.pop_idx == player && l.minutes == 90)
        .unwrap();
    state = reduce(
        state,
        Intent::RecordNpcCards {
            cards: vec![NpcCardEvent {
                season: 1,
                competition_id: 1,
                pop_idx: player,
                fixture_id: load.fixture_id,
                epoch_day: load.epoch_day,
                minute: 90,
                kind: 2,
            }],
        },
        &mut GoatRng::new(0),
    );
    let mut cold = SimulationSession::new();
    let mut fresh = state.clone();
    for round in 1..=3 {
        state.pc_epoch_day = dated_fixture_day(calendar, 1, round);
        fresh.pc_epoch_day = state.pc_epoch_day;
        state = warm.advance_deep(state, &world);
        fresh = cold.advance_deep(fresh, &world);
        assert_eq!(state.deep_results, fresh.deep_results);
        assert_eq!(state.npc_cards, fresh.npc_cards);
        assert_eq!(state.npc_match_loads, fresh.npc_match_loads);
        assert_eq!(state.orbit_records, fresh.orbit_records);
        assert_eq!(
            state
                .npc_match_loads
                .iter()
                .find(|l| l.pop_idx == player && l.epoch_day == state.pc_epoch_day)
                .unwrap()
                .minutes,
            0
        );
    }
}
#[test]
fn completed_input_edits_invalidate_compressed_snapshot_and_match_full_rebuild() {
    let world = WorldGenesis::generate(42);
    let mut state = state();
    let mut warm = SimulationSession::new();
    state.pc_epoch_day = dated_fixture_day(state.chronology().unwrap(), 1, 0);
    state = warm.advance_deep(state, &world);
    state.season_number = 2;
    state.pc_epoch_day = state.chronology().unwrap().frame(2).preparation_start;
    let before = warm.population_deep(&state).career_fingerprint();
    let builds = warm.rebuild_count();
    assert_eq!(
        before,
        SimulationSession::new()
            .population_deep(&state)
            .career_fingerprint()
    );
    state.orbit_records[0].credits[0].goals += 1;
    state.npc_match_loads[0].minutes = 1;
    let changed = warm.population_deep(&state).career_fingerprint();
    assert_eq!(warm.rebuild_count(), builds + 1);
    let mut fresh = SimulationSession::new();
    assert_eq!(changed, fresh.population_deep(&state).career_fingerprint());
    assert_eq!(warm.league_scores(&state), fresh.league_scores(&state));
    let young = warm.population_deep(&state).len() - 1;
    for idx in [0, 100, 1000, young] {
        assert_eq!(
            warm.population_deep(&state).training_history(idx, 0, 60),
            fresh.population_deep(&state).training_history(idx, 0, 60)
        );
        assert_eq!(
            warm.population_deep(&state).medical_status(idx, 60),
            fresh.population_deep(&state).medical_status(idx, 60)
        );
    }
    assert_eq!(
        warm.population_deep(&state).goalkeeper,
        fresh.population_deep(&state).goalkeeper
    );
    state.pc_epoch_day = dated_fixture_day(state.chronology().unwrap(), 2, 0);
    let expected = fresh.advance_deep(state.clone(), &world);
    let actual = warm.advance_deep(state, &world);
    assert_eq!(actual.deep_results, expected.deep_results);
    assert_eq!(actual.npc_cards, expected.npc_cards);
    assert_eq!(actual.npc_match_loads, expected.npc_match_loads);
    assert_eq!(actual.orbit_records, expected.orbit_records);
}
