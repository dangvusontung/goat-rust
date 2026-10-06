//! Release diagnostic: actual NPC journal over three seasons, codec and cold replay.
use goat_core::{chronology::Chronology, player::PlayerView, state::WorldState};
use goat_save::save::{from_bytes, from_world_state, to_bytes, to_world_state};
use goat_world::{calendar::dated_fixture_day, session::SimulationSession, world::WorldGenesis};
use std::time::Instant;

fn residency(label: &str) {
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status
            .lines()
            .filter(|l| l.starts_with("VmRSS:") || l.starts_with("VmHWM:"))
        {
            println!("{label} {line}");
        }
    }
}

fn main() {
    let simulation_begin = Instant::now();
    let args: Vec<_> = std::env::args().collect();
    let years = args
        .windows(2)
        .find(|w| w[0] == "--years")
        .map_or(3, |w| w[1].parse::<u32>().expect("years"));
    let final_codec = args.iter().any(|a| a == "--final-codec");
    let no_codec = args.iter().any(|a| a == "--no-codec");
    let single = args.iter().any(|a| a == "--single");
    let world = WorldGenesis::generate(42);
    let calendar = Chronology::new(2023);
    let mut state = WorldState::new();
    state.pc_player_id = Some(state.players.push(PlayerView::default()));
    state.dated_calendar = true;
    state.realistic_npc = args.iter().any(|a| a == "--realistic");
    state.world_seed = 42;
    state.career_base_year = 2023;
    state.pc_div_idx = 59;
    state.pc_club_idx = 1180;
    let mut session = SimulationSession::new();
    let mut last_bytes = Vec::new();
    for season in 1..=years {
        state.season_number = season;
        state.pc_epoch_day = calendar.frame(season).preparation_start;
        state = session.advance_deep(state, &world);
        for round in 0..goat_world::ROUNDS_PER_SEASON {
            state.pc_epoch_day = dated_fixture_day(calendar, season, round);
            state = session.advance_deep_without_pc(state, &world, round);
        }
        residency(&format!("season_{season}_before_codec"));
        if no_codec || final_codec && season < years {
            continue;
        }
        let data = from_world_state(&state, &state.players.snapshot(state.pc_player_id.unwrap()));
        let mut base = data.clone();
        base.npc_match_loads.clear();
        base.orbit_records.clear();
        let base_bytes = to_bytes(&base).len();
        let old_loads = 8 + data.npc_match_loads.len() * 24;
        let old_credits: usize = data
            .orbit_records
            .iter()
            .map(|r| 13 + r.credits.len() * 7)
            .sum();
        let begin = Instant::now();
        last_bytes = to_bytes(&data);
        let encode_us = begin.elapsed().as_micros();
        let begin = Instant::now();
        let decoded = from_bytes(&last_bytes).unwrap();
        let decode_us = begin.elapsed().as_micros();
        assert_eq!(decoded.npc_match_loads, data.npc_match_loads);
        assert_eq!(decoded.orbit_records, data.orbit_records);
        assert_eq!(decoded.npc_cards, data.npc_cards);
        assert_eq!(decoded.realistic_npc, data.realistic_npc);
        println!("season={season} loads={} credits={} old_journal_bytes={} compact_journal_bytes={} whole_save_bytes={} encode_us={encode_us} decode_us={decode_us}",
            data.npc_match_loads.len(), data.orbit_records.iter().map(|r| r.credits.len()).sum::<usize>(),
            old_loads + old_credits, last_bytes.len() - base_bytes, last_bytes.len());
    }
    println!(
        "simulation_and_codec_us={}",
        simulation_begin.elapsed().as_micros()
    );
    residency("single_session_after_codec");
    println!(
        "single_session_rows={} cards={} scope_count={}",
        state.npc_match_loads.len(),
        state.npc_cards.len(),
        state.deep_scopes.len()
    );
    if single || no_codec {
        return;
    }
    let mut resumed = to_world_state(&from_bytes(&last_bytes).unwrap(), &world);
    // The diagnostic has no PC beat engine; retain its explicit adapter club/league.
    resumed.pc_div_idx = state.pc_div_idx;
    resumed.pc_club_idx = state.pc_club_idx;
    let begin = Instant::now();
    let mut cold = SimulationSession::new();
    assert_eq!(
        cold.population_deep(&resumed).career_fingerprint(),
        session.population_deep(&state).career_fingerprint()
    );
    println!(
        "cold_resume_us={} loads_in_memory={} replay_equal=true",
        begin.elapsed().as_micros(),
        resumed.npc_match_loads.len()
    );
    residency("two_sessions_cold_replay");
    for idx in [0, 100, 1000] {
        let week = state.pc_epoch_day / 7;
        assert_eq!(
            cold.population_deep(&resumed).medical_status(idx, week),
            session.population_deep(&state).medical_status(idx, week)
        );
        assert_eq!(
            cold.population_deep(&resumed)
                .training_history(idx, 0, week),
            session
                .population_deep(&state)
                .training_history(idx, 0, week)
        );
    }
}
