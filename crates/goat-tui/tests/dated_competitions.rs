//! Headless verification of the test adapter's dated match/save transaction.
use goat_core::{
    player::PlayerView,
    state::{reduce, Intent, WorldState},
};
use goat_fixed::Fixed;
use goat_match::{
    contest::auto_pick_generated_choice,
    sim::{advance_beat, dated_match_receipt, dated_match_setup, start_match_dated, BeatLibrary},
};
use goat_rng::GoatRng;
use goat_traits::PlayerTraits;
use goat_world::{
    competitions::{match_roster, next_pc_fixture},
    session::SimulationSession,
    world::WorldGenesis,
};
fn play(session: &mut SimulationSession, state: WorldState, world: &WorldGenesis) -> WorldState {
    let state = session.advance_until_pc_fixture(state, world).unwrap();
    let f = next_pc_fixture(&state, world).unwrap().clone();
    let roster = match_roster(session, &state, &f);
    let lib = BeatLibrary::load(include_str!("../../../beats.json")).unwrap();
    let setup = dated_match_setup(&state, &roster, PlayerTraits::default());
    let seed = state.world_seed ^ f.id;
    let mut rng = GoatRng::new(seed);
    let mut active = start_match_dated(&lib, setup, roster.clone(), seed, &mut rng);
    for _ in 0..500 {
        if active.is_complete {
            break;
        }
        let choice = active.current_beat().map_or(0, |b| {
            auto_pick_generated_choice(&b.choices, &active.setup.player_attrs)
        });
        active = advance_beat(active, choice, &lib, &mut rng);
    }
    assert!(active.is_complete);
    assert_eq!(active.current_minute(), 90);
    let receipt = dated_match_receipt(active.final_result.as_ref().unwrap(), &roster);
    let result = session
        .apply_dated_pc_match(state, world, receipt.clone())
        .unwrap();
    let again = session
        .apply_dated_pc_match(result.clone(), world, receipt)
        .unwrap();
    assert_eq!(again.pc_season_matches, result.pc_season_matches);
    assert_eq!(again.npc_match_loads, result.npc_match_loads);
    result
}
#[test]
fn dated_pc_match_is_idempotent_and_next_match_survives_save() {
    let world = WorldGenesis::generate(42);
    let mut state = WorldState::new();
    state.world_seed = 42;
    state.dated_calendar = true;
    state.career_base_year = 2023;
    state.season_number = 1;
    state.pc_club_idx = 1180;
    state.pc_div_idx = 59;
    state.pc_player_id = Some(state.players.push(PlayerView {
        current: [Fixed::from_int(99); goat_core::attrs::NUM_ATTRS],
        ..Default::default()
    }));
    state = reduce(state, Intent::EnableDatedCompetitions, &mut GoatRng::new(0));
    let mut warm = SimulationSession::new();
    let state = play(&mut warm, state, &world);
    assert_eq!(state.pc_season_matches, 1);
    let save =
        goat_save::save::from_world_state_with_session(&state, &state.pc_display_view(), &mut warm);
    let loaded = goat_save::save::from_bytes(&goat_save::save::to_bytes(&save)).unwrap();
    let restored = goat_save::save::to_world_state(&loaded, &world);
    let mut resumed = goat_save::save::session_from_save(&loaded, &restored);
    assert_eq!(resumed.rebuild_count(), 0);
    let a = play(&mut warm, state, &world);
    let b = play(&mut resumed, restored, &world);
    assert_eq!(a.competition_calendar, b.competition_calendar);
    assert_eq!(a.orbit_records, b.orbit_records);
    assert_eq!(a.npc_match_loads, b.npc_match_loads);
    assert_eq!(a.pc_season_goals, b.pc_season_goals);
    assert_eq!(a.pc_form, b.pc_form);
    assert_eq!(
        warm.population_deep(&a).career_fingerprint(),
        resumed.population_deep(&b).career_fingerprint()
    );
}
