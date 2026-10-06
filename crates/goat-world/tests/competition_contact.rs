use goat_core::{
    competitions::{CompetitionCalendar, DatedFixture, FixtureResult},
    deep::DeepScope,
    state::WorldState,
};
use goat_world::{competitions::match_roster, session::SimulationSession};
#[test]
fn contact_reads_do_not_expand_scope_or_reroll_light_results() {
    let mut state = WorldState::new();
    state.world_seed = 42;
    state.dated_calendar = true;
    state.realistic_npc = true;
    state.career_base_year = 2023;
    state.season_number = 1;
    state.pc_club_idx = 1180;
    state.pc_div_idx = 59;
    state.deep_scopes = vec![DeepScope {
        season: 1,
        epoch_day: 0,
        pc_league: 59,
        pc_club: 1180,
        leagues: vec![59],
    }];
    let f = DatedFixture {
        id: 99,
        workload_id: 99,
        season: 1,
        competition: 2,
        region: 19,
        round: 0,
        slot: 0,
        stage: 1,
        leg: 1,
        original_day: 48,
        day: 48,
        home: 1180,
        away: 1140,
        priority: 3,
    };
    let old = FixtureResult {
        fixture: DatedFixture {
            competition: 1,
            home: 1140,
            away: 1141,
            id: 10,
            workload_id: 10,
            stage: 0,
            leg: 0,
            ..f.clone()
        },
        goals: [2, 1],
        winner: None,
        detailed: false,
    };
    state.competition_calendar = Some(CompetitionCalendar {
        prepared_through: 1,
        fixtures: vec![f.clone()],
        results: vec![old.clone()],
        ..Default::default()
    });
    let scope = state.deep_scopes.clone();
    let mut session = SimulationSession::new();
    let a = match_roster(&mut session, &state, &f);
    let b = match_roster(&mut session, &state, &f);
    assert!(!a.teams[1].is_empty());
    assert_eq!(
        a.teams[1].iter().map(|p| p.id).collect::<Vec<_>>(),
        b.teams[1].iter().map(|p| p.id).collect::<Vec<_>>()
    );
    assert_eq!(state.deep_scopes, scope);
    assert_eq!(state.competition_calendar.unwrap().results, vec![old]);
}
