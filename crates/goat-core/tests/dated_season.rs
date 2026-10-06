use goat_core::{
    chronology::{Chronology, CivilDate},
    generation::CreationChoices,
    positions::PrimaryPosition,
    state::{reduce, Intent, WorldState},
};
use goat_fixed::Fixed;
use goat_rng::GoatRng;
fn state() -> WorldState {
    let s = reduce(
        WorldState::new(),
        Intent::CreatePlayer {
            seed: 42,
            choices: CreationChoices {
                name: "Calendar".into(),
                primary_position: PrimaryPosition::ST,
                nationality: "England".into(),
                club: "Test".into(),
            },
        },
        &mut GoatRng::new(0),
    );
    let s = reduce(
        s,
        Intent::EnableDatedCalendar { base_year: 2023 },
        &mut GoatRng::new(0),
    );
    reduce(
        s,
        Intent::StartSeason { fixtures: vec![] },
        &mut GoatRng::new(0),
    )
}
#[test]
fn boundary_keeps_injury_energy_and_partial_week_remainder() {
    let c = Chronology::new(2023);
    let mut s = state();
    let id = s.pc_player_id.unwrap();
    let before = c
        .epoch_day(CivilDate {
            year: 2024,
            month: 6,
            day: 25,
        })
        .unwrap();
    s = reduce(
        s,
        Intent::AdvanceToDate {
            epoch_day: before,
            train: false,
        },
        &mut GoatRng::new(1),
    );
    s.players.set_injury_weeks(id, 3);
    s.players.set_energy(id, Fixed::from_int(40));
    let close = c.frame(1).next_preparation_start;
    s = reduce(
        s,
        Intent::AdvanceToDate {
            epoch_day: close,
            train: false,
        },
        &mut GoatRng::new(1),
    );
    let injury = s.players.get_injury_weeks(id);
    let energy = s.players.get_energy(id);
    assert!(injury > 0 && injury < 3);
    s = reduce(
        s,
        Intent::StartSeason { fixtures: vec![] },
        &mut GoatRng::new(0),
    );
    assert_eq!(s.pc_epoch_day, close);
    assert_eq!(s.players.get_injury_weeks(id), injury);
    assert_eq!(s.players.get_energy(id), energy);
    assert_eq!(s.pc_age_years(), 17);
    let target = close + 7;
    s = reduce(
        s,
        Intent::AdvanceToDate {
            epoch_day: target,
            train: false,
        },
        &mut GoatRng::new(0),
    );
    assert_eq!(s.players.get_injury_weeks(id), injury - 1);
    assert_eq!(s.players.get_age_weeks(id), 16 * 52 + target / 7);
    assert_eq!(s.pc_development_history.weeks.len(), (target / 7) as usize);
    for (week, row) in s.pc_development_history.weeks.iter().enumerate() {
        assert_eq!(row.epoch_day, week as u32 * 7);
    }
    let weeks = s.pc_development_history.weeks.len();
    s = reduce(
        s,
        Intent::AdvanceToDate {
            epoch_day: target,
            train: true,
        },
        &mut GoatRng::new(3),
    );
    assert_eq!(s.pc_development_history.weeks.len(), weeks);
}
#[test]
fn calendar_window_and_fixture_reschedule_use_absolute_days() {
    use goat_calendar::{Fixture, FixtureImportance, WindowKind};
    let c = Chronology::new(2023);
    let date = c.frame(2).start_day;
    let f = |id| Fixture {
        id,
        competition_id: 1,
        scheduled_day: date,
        original_day: date,
        is_orbit: true,
        importance: FixtureImportance::League,
        leg_for_id: None,
    };
    let (day, flashes, fixtures) = goat_core::calendar_loop::advance_dated_calendar(
        c.frame(2).preparation_start,
        date + 1,
        2023,
        42,
        &[f(1), f(2)],
    );
    assert_eq!(day, date + 1);
    assert!(flashes
        .iter()
        .any(|w| w.window == WindowKind::OffSeason && w.day == c.frame(2).preparation_start));
    assert_eq!(
        fixtures.iter().filter(|f| f.scheduled_day == date).count(),
        1
    );
    assert!(fixtures.iter().any(|f| f.scheduled_day > date));
}

#[test]
fn moved_fixture_order_and_completed_ids_prevent_duplicate_credit() {
    use goat_calendar::{Fixture, FixtureImportance};
    let mut s = state();
    let f = |id, day| Fixture {
        id,
        competition_id: 1,
        scheduled_day: day,
        original_day: day,
        is_orbit: true,
        importance: FixtureImportance::League,
        leg_for_id: None,
    };
    s.pc_season_fixtures = vec![f(10, 60), f(20, 50)];
    assert_eq!(s.next_dated_fixture().unwrap().0, 1);
    s = reduce(
        s,
        Intent::AdvanceToDate {
            epoch_day: 50,
            train: false,
        },
        &mut GoatRng::new(0),
    );
    s = reduce(
        s,
        Intent::BeginDatedFixture { fixture_id: 20 },
        &mut GoatRng::new(0),
    );
    let intent = || Intent::ApplyRoundResult {
        competition_id: 1,
        pc_goals: 1,
        pc_assists: 0,
        pc_decisive_count: 0,
        pc_clutch_count: 0,
        fixture_importance: FixtureImportance::League,
        pc_output: 50,
        pc_result: 1,
        round_results: vec![],
        rest_weeks: 0,
        week_ends: true,
    };
    s = reduce(s, intent(), &mut GoatRng::new(0));
    assert_eq!(s.pc_played_fixture_ids, vec![20]);
    assert_eq!(s.season_round, 1);
    let goals = s.pc_season_goals;
    s = reduce(
        s,
        Intent::BeginDatedFixture { fixture_id: 20 },
        &mut GoatRng::new(0),
    );
    s = reduce(s, intent(), &mut GoatRng::new(0));
    assert_eq!(s.season_round, 1);
    assert_eq!(s.pc_season_goals, goals);
    assert_eq!(s.next_dated_fixture().unwrap().0, 0);
}

#[test]
fn weekly_injury_onset_is_dated_when_the_period_completes() {
    struct InjuryRoll;
    impl goat_rng::RngSource for InjuryRoll {
        fn next_u64(&mut self) -> u64 {
            0
        }
    }
    let mut s = state();
    let id = s.pc_player_id.unwrap();
    s.players.set_energy(id, Fixed::ZERO);
    s = reduce(
        s,
        Intent::AdvanceToDate {
            epoch_day: 3,
            train: true,
        },
        &mut InjuryRoll,
    );
    assert!(s.pc_development_history.health.is_empty());
    s = reduce(
        s,
        Intent::AdvanceToDate {
            epoch_day: 7,
            train: true,
        },
        &mut InjuryRoll,
    );
    let onset = s
        .pc_development_history
        .health
        .iter()
        .find(|e| e.kind == 0)
        .expect("forced injury roll must produce an onset");
    assert_eq!(onset.epoch_day, 7);
}
