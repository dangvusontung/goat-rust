use goat_core::state::WorldState;
use goat_world::{calendar::dated_fixture_day, session::SimulationSession, world::WorldGenesis};
fn state() -> WorldState {
    let mut s = WorldState::new();
    s.season_number = 1;
    s.world_seed = 42;
    s.career_base_year = 2023;
    s.dated_calendar = true;
    s.realistic_npc = true;
    s.pc_div_idx = 59;
    s.pc_club_idx = 1180;
    s
}
#[test]
fn checkpoint_preserves_live_youth_health_history_next_match_and_year_boundary() {
    let world = WorldGenesis::generate(42);
    let mut state = state();
    let mut warm = SimulationSession::new();
    state.pc_epoch_day = dated_fixture_day(state.chronology().unwrap(), 1, 0);
    state = warm.advance_deep_without_pc(state, &world, 0);
    state.season_number = 2;
    state.pc_epoch_day = dated_fixture_day(state.chronology().unwrap(), 2, 0);
    state = warm.advance_deep_without_pc(state, &world, 0);
    let bytes = warm.resume_checkpoint(&state).unwrap();
    let mut resumed = SimulationSession::new();
    assert!(resumed.restore_checkpoint(&state, &bytes));
    assert_eq!(resumed.rebuild_count(), 0);
    assert_eq!(bytes, resumed.resume_checkpoint(&state).unwrap());
    let mut fresh = SimulationSession::new();
    let indices = [0, 100, 1000, warm.population_deep(&state).len() - 1];
    let week = state.pc_epoch_day / 7;
    for idx in indices {
        let a = warm.population_deep(&state);
        let b = resumed.population_deep(&state);
        let c = fresh.population_deep(&state);
        assert_eq!(a.current_ovr(idx, week), b.current_ovr(idx, week));
        assert_eq!(b.current_ovr(idx, week), c.current_ovr(idx, week));
        assert_eq!(a.medical_status(idx, week), b.medical_status(idx, week));
        assert_eq!(b.medical_status(idx, week), c.medical_status(idx, week));
        assert_eq!(
            a.training_history(idx, 0, week),
            b.training_history(idx, 0, week)
        );
        assert_eq!(a.injury_history(idx, week), b.injury_history(idx, week));
    }
    assert_eq!(
        warm.population_deep(&state).form,
        resumed.population_deep(&state).form
    );
    assert_eq!(warm.league_scores(&state), resumed.league_scores(&state));
    let mut restored_state = state.clone();
    state.pc_epoch_day = dated_fixture_day(state.chronology().unwrap(), 2, 1);
    restored_state.pc_epoch_day = state.pc_epoch_day;
    state = warm.advance_deep_without_pc(state, &world, 1);
    restored_state = resumed.advance_deep_without_pc(restored_state, &world, 1);
    assert_eq!(state.deep_results, restored_state.deep_results);
    assert_eq!(state.npc_cards, restored_state.npc_cards);
    assert_eq!(state.npc_match_loads, restored_state.npc_match_loads);
    assert_eq!(state.orbit_records, restored_state.orbit_records);
    assert_eq!(resumed.rebuild_count(), 0);
    state.season_number = 3;
    restored_state.season_number = 3;
    state.pc_epoch_day = dated_fixture_day(state.chronology().unwrap(), 3, 0);
    restored_state.pc_epoch_day = state.pc_epoch_day;
    state = warm.advance_deep_without_pc(state, &world, 0);
    restored_state = resumed.advance_deep_without_pc(restored_state, &world, 0);
    assert_eq!(state.deep_results, restored_state.deep_results);
    assert_eq!(state.npc_cards, restored_state.npc_cards);
    assert_eq!(state.npc_match_loads, restored_state.npc_match_loads);
    assert_eq!(state.orbit_records, restored_state.orbit_records);
    assert_eq!(
        warm.population_deep(&state).fingerprint(),
        resumed.population_deep(&state).fingerprint()
    );
}
#[test]
fn edited_inputs_and_corrupt_cache_do_not_replace_an_existing_session() {
    let world = WorldGenesis::generate(42);
    let mut s = state();
    let mut warm = SimulationSession::new();
    s.pc_epoch_day = dated_fixture_day(s.chronology().unwrap(), 1, 0);
    s = warm.advance_deep_without_pc(s, &world, 0);
    let bytes = warm.resume_checkpoint(&s).unwrap();
    let fingerprint = warm.population_deep(&s).career_fingerprint();
    let mut edited = s.clone();
    edited.npc_match_loads[0].minutes = 1;
    assert!(!warm.restore_checkpoint(&edited, &bytes));
    edited = s.clone();
    edited.orbit_records[0].credits[0].goals += 1;
    assert!(!warm.restore_checkpoint(&edited, &bytes));
    edited = s.clone();
    edited.npc_cards[0].kind = (edited.npc_cards[0].kind + 1) % 3;
    assert!(!warm.restore_checkpoint(&edited, &bytes));
    edited = s.clone();
    edited.npc_match_loads.swap(0, 1);
    assert!(!warm.restore_checkpoint(&edited, &bytes));
    edited = s.clone();
    edited.npc_match_loads.remove(0);
    assert!(!warm.restore_checkpoint(&edited, &bytes));
    edited = s.clone();
    edited.deep_results[0].home_goals += 1;
    assert!(!warm.restore_checkpoint(&edited, &bytes));
    edited = s.clone();
    edited.world_seed += 1;
    assert!(!warm.restore_checkpoint(&edited, &bytes));
    edited = s.clone();
    edited.pc_epoch_day += 1;
    assert!(!warm.restore_checkpoint(&edited, &bytes));
    edited = s.clone();
    edited.career_base_year += 1;
    assert!(!warm.restore_checkpoint(&edited, &bytes));
    let mut bad = bytes.clone();
    bad[0] ^= 1;
    assert!(!warm.restore_checkpoint(&s, &bad));
    let mut bad = bytes.clone();
    let last = bad.len() - 1;
    bad[last] ^= 1;
    assert!(!warm.restore_checkpoint(&s, &bad));
    for end in [0, 4, 16, bytes.len() / 2, bytes.len() - 1] {
        assert!(!warm.restore_checkpoint(&s, &bytes[..end]));
    }
    assert_eq!(warm.population_deep(&s).career_fingerprint(), fingerprint);
}
