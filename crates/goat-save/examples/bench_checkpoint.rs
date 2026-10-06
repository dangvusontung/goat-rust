//! Actual dated NPC seasons; file I/O, checkpoint resume and seed replay measured separately.
use goat_core::{chronology::Chronology, player::PlayerView, state::WorldState};
use goat_save::save::{
    from_world_state, from_world_state_with_session, load_from_file, save_to_file,
    session_from_save, to_bytes, to_world_state,
};
use goat_world::{calendar::dated_fixture_day, session::SimulationSession, world::WorldGenesis};
use std::{path::Path, time::Instant};
fn memory(label: &str) {
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status
            .lines()
            .filter(|l| l.starts_with("VmRSS:") || l.starts_with("VmHWM:"))
        {
            println!("{label} {line}");
        }
    }
}
fn load(path: &Path) -> (WorldState, SimulationSession, WorldGenesis, u128) {
    let begin = Instant::now();
    let data = load_from_file(path).unwrap();
    let world = WorldGenesis::generate(data.world_seed);
    let state = to_world_state(&data, &world);
    let mut session = session_from_save(&data, &state);
    session.population_deep(&state);
    assert_eq!(session.rebuild_count(), 0, "checkpoint was not used");
    let elapsed = begin.elapsed().as_micros();
    drop(data);
    (state, session, world, elapsed)
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if let Some(w) = args.windows(2).find(|w| w[0] == "--load") {
        let (state, session, _, us) = load(Path::new(&w[1]));
        println!(
            "season={} disk_decode_restore_first_query_us={us} rebuilds={} rows={}",
            state.season_number,
            session.rebuild_count(),
            state.npc_match_loads.len()
        );
        memory("loaded_game_only");
        return;
    }
    let years = args
        .windows(2)
        .find(|w| w[0] == "--years")
        .map_or(3, |w| w[1].parse::<u32>().unwrap());
    let oracle = !args.iter().any(|a| a == "--no-oracle");
    let directory = args
        .windows(2)
        .find(|w| w[0] == "--dir")
        .map_or("/tmp/goat-checkpoints", |w| w[1].as_str());
    std::fs::create_dir_all(directory).unwrap();
    let world = WorldGenesis::generate(42);
    let calendar = Chronology::new(2023);
    let mut state = WorldState::new();
    state.pc_player_id = Some(state.players.push(PlayerView::default()));
    state.world_seed = 42;
    state.career_base_year = 2023;
    state.dated_calendar = true;
    state.realistic_npc = true;
    state.pc_div_idx = 59;
    state.pc_club_idx = 1180;
    let mut warm = SimulationSession::new();
    let simulation = Instant::now();
    for season in 1..=years {
        state.season_number = season;
        state.pc_epoch_day = calendar.frame(season).preparation_start;
        state = warm.advance_deep(state, &world);
        for round in 0..goat_world::ROUNDS_PER_SEASON {
            state.pc_epoch_day = dated_fixture_day(calendar, season, round);
            state = warm.advance_deep_without_pc(state, &world, round);
        }
        println!(
            "completed_season={season} cumulative_wall_us={} rows={}",
            simulation.elapsed().as_micros(),
            state.npc_match_loads.len()
        );
        if ![3, 10, 20].contains(&season) && season != years {
            continue;
        }
        memory("warm_game");
        let view = state.players.snapshot(state.pc_player_id.unwrap());
        let plain_bytes = to_bytes(&from_world_state(&state, &view)).len();
        let begin = Instant::now();
        let data = from_world_state_with_session(&state, &view, &mut warm);
        let export_us = begin.elapsed().as_micros();
        assert!(!data.resume_checkpoint.is_empty());
        let path = Path::new(directory).join(format!("seed-42-season-{season}.gsav"));
        let begin = Instant::now();
        save_to_file(&data, &path).unwrap();
        let write_us = begin.elapsed().as_micros();
        let save_bytes = std::fs::metadata(&path).unwrap().len();
        let (mut loaded, mut resumed, _, load_us) = load(&path);
        assert_eq!(loaded.deep_results, state.deep_results);
        assert_eq!(loaded.npc_match_loads, state.npc_match_loads);
        assert_eq!(loaded.npc_cards, state.npc_cards);
        assert_eq!(loaded.orbit_records, state.orbit_records);
        let exact = resumed.resume_checkpoint(&loaded).unwrap();
        assert!(
            exact == data.resume_checkpoint,
            "whole cached state differed after round trip"
        );
        drop(exact);
        println!("season={season} plain_save_bytes={plain_bytes} checkpoint_bytes={} full_save_bytes={save_bytes} export_us={export_us} atomic_encode_write_us={write_us} disk_decode_restore_first_query_us={load_us} snapshot_exact=true rebuilds={}",data.resume_checkpoint.len(),resumed.rebuild_count());
        memory("diagnostic_with_saved_data_and_two_sessions");
        drop(data);
        if oracle {
            let begin = Instant::now();
            let mut cold = SimulationSession::new();
            let a = warm.population_deep(&state);
            let b = cold.population_deep(&state);
            let c = resumed.population_deep(&loaded);
            assert_eq!(a.fingerprint(), b.fingerprint());
            assert_eq!(a.career_fingerprint(), b.career_fingerprint());
            assert_eq!(b.fingerprint(), c.fingerprint());
            assert_eq!(b.form, c.form);
            assert_eq!(b.goalkeeper, c.goalkeeper);
            println!(
                "season={season} seed_replay_us={}",
                begin.elapsed().as_micros()
            );
            let week = state.pc_epoch_day / 7;
            let young = a.len() - 1;
            for idx in [0, 100, 1000, young] {
                assert_eq!(a.current_ovr(idx, week), b.current_ovr(idx, week));
                assert_eq!(b.current_ovr(idx, week), c.current_ovr(idx, week));
                assert_eq!(a.medical_status(idx, week), b.medical_status(idx, week));
                assert_eq!(b.medical_status(idx, week), c.medical_status(idx, week));
                assert_eq!(
                    a.training_history(idx, 0, week),
                    c.training_history(idx, 0, week)
                );
                assert_eq!(b.injury_history(idx, week), c.injury_history(idx, week));
                let av = a.promote(idx, week, "audit", &world);
                let bv = b.promote(idx, week, "audit", &world);
                let cv = c.promote(idx, week, "audit", &world);
                assert_eq!(
                    av.as_ref().map(|v| v.current),
                    bv.as_ref().map(|v| v.current)
                );
                assert_eq!(
                    bv.as_ref().map(|v| v.current),
                    cv.as_ref().map(|v| v.current)
                );
            }
            assert_eq!(warm.league_scores(&state), cold.league_scores(&state));
            assert_eq!(cold.league_scores(&state), resumed.league_scores(&loaded));
            let begin = Instant::now();
            let mut expected = state.clone();
            expected.season_number += 1;
            loaded.season_number += 1;
            expected.pc_epoch_day = dated_fixture_day(calendar, expected.season_number, 0);
            loaded.pc_epoch_day = expected.pc_epoch_day;
            expected = cold.advance_deep_without_pc(expected, &world, 0);
            loaded = resumed.advance_deep_without_pc(loaded, &world, 0);
            assert_eq!(expected.deep_results, loaded.deep_results);
            assert_eq!(expected.npc_cards, loaded.npc_cards);
            assert_eq!(expected.npc_match_loads, loaded.npc_match_loads);
            assert_eq!(expected.orbit_records, loaded.orbit_records);
            println!("season={season} next_season_fixture_us={} next_equal=true active_youth_medical_equal=true",begin.elapsed().as_micros());
        }
    }
}
