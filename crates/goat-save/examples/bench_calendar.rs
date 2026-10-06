//! Actual dated competitions with save/checkpoint continuation and seasonal comparison.
use goat_core::{
    player::PlayerView,
    state::{reduce, Intent, WorldState},
};
use goat_rng::GoatRng;
use goat_world::{session::SimulationSession, world::WorldGenesis};
use std::time::Instant;
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let years = args
        .windows(2)
        .find(|w| w[0] == "--years")
        .map_or(1, |w| w[1].parse::<u32>().unwrap());
    let weeks = args
        .windows(2)
        .find(|w| w[0] == "--weeks")
        .map(|w| w[1].parse::<u32>().unwrap());
    let world = WorldGenesis::generate(42);
    let mut state = WorldState::new();
    state.world_seed = 42;
    state.dated_calendar = true;
    state.realistic_npc = true;
    state.career_base_year = 2023;
    state.season_number = 1;
    state.pc_div_idx = 59;
    state.pc_club_idx = 1180;
    state.pc_player_id = Some(state.players.push(PlayerView::default()));
    state = reduce(state, Intent::EnableDatedCompetitions, &mut GoatRng::new(0));
    let mut session = SimulationSession::new();
    let run = Instant::now();
    for year in 1..=years {
        let start = Instant::now();
        let frame = state.chronology().unwrap().frame(year);
        let end = weeks.map_or(frame.end_day, |n| frame.preparation_start + n * 7);
        if let Some(n) = weeks {
            for week in 1..=n {
                let tick = Instant::now();
                state = session
                    .advance_competitions(state, &world, frame.preparation_start + week * 7)
                    .unwrap();
                println!("week={week} weekly_us={}", tick.elapsed().as_micros());
            }
        } else {
            state = session.advance_competitions(state, &world, end).unwrap();
        }
        let cal = state.competition_calendar.as_ref().unwrap();
        let mut dates = std::collections::BTreeMap::<(bool, u32), Vec<u32>>::new();
        let mut ids = std::collections::BTreeSet::new();
        for f in cal
            .fixtures
            .iter()
            .chain(cal.results.iter().map(|r| &r.fixture))
        {
            assert!(ids.insert(f.id));
            for team in [f.home, f.away] {
                dates.entry((f.national(), team)).or_default().push(f.day);
            }
        }
        for days in dates.values_mut() {
            days.sort_unstable();
            assert!(days.windows(2).all(|pair| pair[1] - pair[0] >= 3));
        }
        println!("season={year} season_us={} fixtures={} detailed={} postponed={} cards={} loads={} coefficients={} pending_national={}",start.elapsed().as_micros(),cal.results.iter().filter(|r|r.fixture.season==year).count(),cal.results.iter().filter(|r|r.fixture.season==year && r.detailed).count(),cal.results.iter().filter(|r|r.fixture.season==year && r.fixture.original_day!=r.fixture.day).count(),state.npc_cards.len(),state.npc_match_loads.len(),cal.coefficients.len(),cal.fixtures.iter().filter(|f|f.national() && f.season==year).count());
        let start = Instant::now();
        let view = state.pc_display_view();
        let save = goat_save::save::from_world_state_with_session(&state, &view, &mut session);
        let bytes = goat_save::save::to_bytes(&save);
        println!(
            "save_bytes={} checkpoint_bytes={} encode_us={}",
            bytes.len(),
            save.resume_checkpoint.len(),
            start.elapsed().as_micros()
        );
        let start = Instant::now();
        let data = goat_save::save::from_bytes(&bytes).unwrap();
        let loaded = goat_save::save::to_world_state(&data, &world);
        let mut resumed = goat_save::save::session_from_save(&data, &loaded);
        assert_eq!(loaded.competition_calendar, state.competition_calendar);
        resumed.population_deep(&loaded);
        assert_eq!(resumed.rebuild_count(), 0);
        assert_eq!(
            session.resume_checkpoint(&state),
            resumed.resume_checkpoint(&loaded)
        );
        println!(
            "resume_us={} exact=true rebuilds=0",
            start.elapsed().as_micros()
        );
        let start = Instant::now();
        let mut cold = SimulationSession::new();
        let a = session.population_deep(&state);
        let b = cold.population_deep(&state);
        assert_eq!(a.fingerprint(), b.fingerprint());
        if a.career_fingerprint() != b.career_fingerprint() {
            for i in (0..a.len())
                .filter(|&i| {
                    a.career_apps[i] != b.career_apps[i]
                        || a.career_goals[i] != b.career_goals[i]
                        || a.career_titles[i] != b.career_titles[i]
                })
                .take(10)
            {
                eprintln!(
                    "DIFF idx={i} apps={}/{} goals={}/{} titles={}/{} form={}/{}",
                    a.career_apps[i],
                    b.career_apps[i],
                    a.career_goals[i],
                    b.career_goals[i],
                    a.career_titles[i],
                    b.career_titles[i],
                    a.form[i],
                    b.form[i]
                );
            }
        }
        assert_eq!(a.career_fingerprint(), b.career_fingerprint());
        assert_eq!(a.form, b.form);
        for i in [0, 100, 1000, a.len() - 1] {
            let week = state.pc_epoch_day / 7;
            assert_eq!(a.current_ovr(i, week), b.current_ovr(i, week));
            assert_eq!(a.medical_status(i, week), b.medical_status(i, week));
        }
        assert_eq!(session.league_scores(&state), cold.league_scores(&state));
        println!(
            "cold_us={} retained_cold_equal=true",
            start.elapsed().as_micros()
        );
        if year < years {
            state = session
                .start_next_competition_season(state, &world)
                .unwrap();
            let loaded = resumed
                .start_next_competition_season(loaded, &world)
                .unwrap();
            let through = state.pc_epoch_day + 25;
            state = session
                .advance_competitions(state, &world, through)
                .unwrap();
            let loaded = resumed
                .advance_competitions(loaded, &world, through)
                .unwrap();
            assert_eq!(state.competition_calendar, loaded.competition_calendar);
            assert_eq!(state.npc_match_loads, loaded.npc_match_loads);
            assert_eq!(state.orbit_records, loaded.orbit_records);
            println!(
                "summer_continuation_equal=true season={}",
                state.season_number
            );
        }
    }
    println!("whole_run_us={}", run.elapsed().as_micros());
    if let Ok(s) = std::fs::read_to_string("/proc/self/status") {
        for l in s
            .lines()
            .filter(|l| l.starts_with("VmRSS:") || l.starts_with("VmHWM:"))
        {
            println!("{l}");
        }
    }
}
