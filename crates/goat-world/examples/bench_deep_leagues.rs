//! Native diagnostic: NPC deep fixtures only, excludes the PC beat engine and I/O.
use goat_core::{chronology::Chronology, state::WorldState};
use goat_world::{calendar::dated_fixture_day, session::SimulationSession, world::WorldGenesis};
use std::time::Instant;
fn main() {
    let world = WorldGenesis::generate(42);
    let calendar = Chronology::new(2023);
    let mut state = WorldState::new();
    state.dated_calendar = true;
    state.world_seed = 42;
    state.career_base_year = 2023;
    state.season_number = 1;
    state.pc_div_idx = 59;
    state.pc_club_idx = 1180;
    let mut session = SimulationSession::new();
    let start = Instant::now();
    state = session.advance_deep(state, &world);
    println!("cold_us={}", start.elapsed().as_micros());
    let mut rounds = Vec::new();
    let start = Instant::now();
    for round in 0..goat_world::ROUNDS_PER_SEASON {
        state.pc_epoch_day = dated_fixture_day(calendar, 1, round);
        let begin = Instant::now();
        state = session.advance_deep_without_pc(state, &world, round);
        rounds.push(begin.elapsed().as_micros());
    }
    let elapsed = start.elapsed().as_micros();
    rounds.sort_unstable();
    println!(
        "leagues={} fixtures={} dose_rows={} round_median_us={} round_max_us={} season_us={}",
        state.deep_scopes[0].leagues.len(),
        state.deep_results.len(),
        state.npc_match_loads.len(),
        rounds[rounds.len() / 2],
        rounds.last().unwrap(),
        elapsed
    );
    state.pc_epoch_day = calendar.frame(2).preparation_start;
    state.season_number = 2;
    let begin = Instant::now();
    let players = session.population_deep(&state).len();
    println!(
        "boundary_us={} players={} rebuilds={}",
        begin.elapsed().as_micros(),
        players,
        session.rebuild_count()
    );
}
