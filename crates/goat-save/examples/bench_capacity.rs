//! Diagnostics only; a capacity profile is not a supported gameplay/save model.
use goat_core::{
    player::PlayerView,
    state::{reduce, Intent, WorldState},
};
use goat_rng::GoatRng;
use goat_save::save::{self, SaveData};
use goat_world::session::SimulationSession;
use std::{
    path::{Path, PathBuf},
    time::Instant,
};
const MAGIC: &[u8; 8] = b"GOATCAP1";
fn value(args: &[String], key: &str, default: u32) -> u32 {
    args.windows(2)
        .find(|w| w[0] == key)
        .map_or(default, |w| w[1].parse().expect("invalid numeric option"))
}
fn memory(label: &str) {
    if let Ok(s) = std::fs::read_to_string("/proc/self/status") {
        for line in s
            .lines()
            .filter(|l| l.starts_with("VmRSS:") || l.starts_with("VmHWM:"))
        {
            println!("memory={label} {line}");
        }
    }
}
fn session(players: u32, deep: u32) -> SimulationSession {
    SimulationSession::capacity_benchmark(players, deep).unwrap()
}
fn envelope(data: &SaveData, players: u32, deep: u32) -> Vec<u8> {
    let body = save::to_bytes(data);
    let mut bytes = Vec::with_capacity(body.len() + 16);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&players.to_le_bytes());
    bytes.extend_from_slice(&deep.to_le_bytes());
    bytes.extend_from_slice(&body);
    bytes
}
fn read(path: &Path) -> (SaveData, u32, u32) {
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(bytes.get(..8).unwrap(), MAGIC);
    let players = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let deep = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
    (save::from_bytes(&bytes[16..]).unwrap(), players, deep)
}
fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, &byte| {
        (hash ^ byte as u64).wrapping_mul(0x100000001b3)
    })
}
fn fingerprint(state: &WorldState, pop: &goat_world::population::Population) -> u64 {
    let mut hash = checksum(&goat_world::competitions::encode_calendar(
        state.competition_calendar.as_ref().unwrap(),
    ));
    hash ^= pop.fingerprint().rotate_left(7) ^ pop.career_fingerprint().rotate_left(19);
    for idx in 0..pop.len() {
        hash = hash.rotate_left(5)
            ^ pop.form[idx] as u64
            ^ pop.contract_end_day(idx).unwrap_or(0) as u64;
        for &(day, club) in pop.club_history(idx) {
            hash = hash.rotate_left(11) ^ day as u64 ^ ((club as u64) << 32);
        }
    }
    hash
}
fn load_only(args: &[String], path: &Path) {
    let mode = args
        .windows(2)
        .find(|w| w[0] == "--mode")
        .map_or("checkpoint", |w| w[1].as_str());
    assert!(matches!(mode, "checkpoint" | "cold"));
    let start = Instant::now();
    let (data, players, deep) = read(path);
    let world = goat_world::capacity::world(players, deep, data.world_seed).unwrap();
    let state = save::to_world_state(&data, &world);
    let mut sess = session(players, deep);
    let restored = mode == "checkpoint" && sess.restore_checkpoint(&state, &data.resume_checkpoint);
    if mode == "checkpoint" {
        assert!(
            restored,
            "checkpoint absent or refused; use cold mode to measure fallback"
        );
    }
    drop(data);
    let pop = sess.population_deep(&state);
    let count = pop.len();
    let hash = fingerprint(&state, pop);
    let elapsed = start.elapsed().as_micros();
    println!("mode={mode} players_target={players} deep_budget={deep} season={} population={count} load_first_query_us={elapsed} restored={restored} fingerprint={hash}",state.season_number);
    memory("loaded_single_session");
    println!("rebuilds={}", sess.rebuild_count());
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if let Some(w) = args.windows(2).find(|w| w[0] == "--load") {
        load_only(&args, Path::new(&w[1]));
        return;
    }
    let players = value(&args, "--players", 0);
    let deep = value(&args, "--deep", 6);
    let years = value(&args, "--years", 1);
    let weeks = args
        .windows(2)
        .find(|w| w[0] == "--weeks")
        .map(|w| w[1].parse::<u32>().unwrap());
    assert!(years > 0 && years <= 30);
    assert!(weeks.is_none_or(|n| n > 0 && n <= 52 && years == 1));
    let dir = args
        .windows(2)
        .find(|w| w[0] == "--dir")
        .map_or(PathBuf::from("/tmp/goat-capacity"), |w| {
            PathBuf::from(&w[1])
        });
    std::fs::create_dir_all(&dir).unwrap();
    let world = goat_world::capacity::world(players, deep, 42).unwrap();
    let mut state = WorldState::new();
    state.world_seed = 42;
    state.dated_calendar = true;
    state.realistic_npc = true;
    state.career_base_year = 2023;
    state.season_number = 1;
    state.pc_club_idx = 1180;
    state.pc_div_idx = 59;
    state.pc_player_id = Some(state.players.push(PlayerView::default()));
    state = reduce(state, Intent::EnableDatedCompetitions, &mut GoatRng::new(0));
    let mut warm = session(players, deep);
    let total = Instant::now();
    let start = Instant::now();
    state = warm.prepare_competitions(state, &world).unwrap();
    let population = warm.population_deep(&state).len();
    if players != 0 {
        assert_eq!(population, players as usize);
    }
    println!("profile=dense_rosters arch={} sim={} players_target={players} genesis_population={population} clubs={} leagues={} nations={} deep_budget={deep} selected_deep={} startup_us={}",std::env::consts::ARCH,save::SIM_VERSION,world.clubs.len(),world.leagues.len(),world.nations.len(),state.deep_scopes.last().unwrap().leagues.len(),start.elapsed().as_micros());
    memory("startup_game_only");
    for year in 1..=years {
        let frame = state.chronology().unwrap().frame(year);
        let end = weeks.map_or(frame.end_day, |w| frame.preparation_start + w * 7);
        let season_start = Instant::now();
        let mut number = 0;
        while state.pc_epoch_day < end {
            number += 1;
            let before = state.pc_epoch_day;
            let through = (before + 7).min(end);
            let count = state.competition_calendar.as_ref().unwrap().results.len();
            let tick = Instant::now();
            state = warm.advance_competitions(state, &world, through).unwrap();
            let us = tick.elapsed().as_micros();
            let matches = state.competition_calendar.as_ref().unwrap().results.len() - count;
            let market = goat_world::market::events(state.chronology().unwrap(), year)
                .iter()
                .any(|e| e.day > before && e.day <= through);
            println!("season={year} week={number} from={before} through={through} weekly_us={us} fixtures={matches} market={market}");
        }
        let cal = state.competition_calendar.as_ref().unwrap();
        let population = warm.population_deep(&state);
        let allocated = population.len();
        let active = (0..allocated)
            .filter(|&idx| {
                population.club_at(idx, state.pc_epoch_day).is_some()
                    && !population.is_retired(idx, state.pc_epoch_day / 7)
            })
            .count();
        let hash = fingerprint(&state, population);
        println!("season={year} progression_us={} allocated={allocated} active={active} fixtures={} detailed={} loads={} cards={} fingerprint={hash}",season_start.elapsed().as_micros(),cal.results.iter().filter(|r|r.fixture.season==year).count(),cal.results.iter().filter(|r|r.fixture.season==year&&r.detailed).count(),state.npc_match_loads.len(),state.npc_cards.len());
        memory("warm_game_only");
        if year == 1 || [3, 6, 10, 20].contains(&year) || year == years {
            let begin = Instant::now();
            let view = state.pc_display_view();
            let data = save::from_world_state_with_session(&state, &view, &mut warm);
            let export = begin.elapsed().as_micros();
            let cache = data.resume_checkpoint.len();
            let canonical = save::to_bytes(&save::from_world_state(&state, &view)).len();
            let file = dir.join(format!(
                "players-{players}-deep-{deep}-year-{year}.capbench"
            ));
            let begin = Instant::now();
            let bytes = envelope(&data, players, deep);
            let size = bytes.len();
            std::fs::write(&file, &bytes).unwrap();
            println!("season={year} canonical_save_bytes={canonical} checkpoint_bytes={cache} full_save_bytes={size} export_us={export} encode_write_us={} file={}",begin.elapsed().as_micros(),file.display());
            assert!(
                save::from_bytes(&bytes).is_err(),
                "diagnostic envelope must not load as gameplay"
            );
            drop(bytes);
            if cache != 0 {
                let mut resumed = session(players, deep);
                assert!(resumed.restore_checkpoint(&state, &data.resume_checkpoint));
                assert_eq!(
                    resumed.resume_checkpoint(&state).unwrap(),
                    data.resume_checkpoint
                );
                assert_eq!(fingerprint(&state, resumed.population_deep(&state)), hash);
                assert_eq!(resumed.rebuild_count(), 0);
                println!("season={year} checkpoint_exact=true");
            } else {
                println!("season={year} checkpoint_exact=unavailable");
            }
            drop(data);
            memory("after_save_validation");
        }
        if year < years {
            let start = Instant::now();
            state = warm.start_next_competition_season(state, &world).unwrap();
            println!(
                "closed_season={year} annual_boundary_us={} next_season={}",
                start.elapsed().as_micros(),
                state.season_number
            );
            memory("post_boundary_game_only");
        }
    }
    println!(
        "whole_run_us={} rebuilds={}",
        total.elapsed().as_micros(),
        warm.rebuild_count()
    );
}
