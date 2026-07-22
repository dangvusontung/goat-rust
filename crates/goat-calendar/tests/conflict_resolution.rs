//! `resolveFixturesForDay`-equivalent conflict resolution (`docs/MAIN.md:1090-1117`).
//!
//! TDD module for TASK-DESIGN-round4-competitions-slice1-foundation.md §1.4.
//! Mirrors the bible's own AC set and `promotion.rs`'s existing idempotency-test style.

use goat_calendar::{
    CalendarEngine, CalendarWindow, Competition, CompetitionKind, Fixture, FixtureImportance,
    Season, WindowKind,
};

fn season(windows: Vec<CalendarWindow>) -> Season {
    Season {
        id: 1,
        start_day: 0,
        end_day: 364,
        windows,
        competition_ids: vec![1, 2],
    }
}

fn competitions() -> Vec<Competition> {
    vec![
        Competition {
            id: 1, // league
            kind: CompetitionKind::League,
            priority: 100,
            is_orbit: true,
        },
        Competition {
            id: 2, // synthetic domestic cup — a real one doesn't exist until a sibling slice
            kind: CompetitionKind::DomesticCup,
            priority: 50,
            is_orbit: true,
        },
    ]
}

fn league_fixture(id: u64, day: u32) -> Fixture {
    Fixture {
        id,
        competition_id: 1,
        scheduled_day: day,
        original_day: day,
        is_orbit: true,
        importance: FixtureImportance::League,
        leg_for_id: None,
    }
}

fn cup_fixture(id: u64, day: u32, importance: FixtureImportance) -> Fixture {
    Fixture {
        id,
        competition_id: 2,
        scheduled_day: day,
        original_day: day,
        is_orbit: true,
        importance,
        leg_for_id: None,
    }
}

/// A same-day clash between a higher-priority (league) and a lower-priority (cup)
/// competition resolves in favour of the higher-priority competition, regardless of
/// which fixture has the higher `FixtureImportance`.
#[test]
fn higher_priority_competition_wins_same_day_clash() {
    let fixtures = vec![
        league_fixture(1, 10),
        // Higher FixtureImportance than the league match, but a lower-priority
        // competition — priority must win the tie-break, not importance.
        cup_fixture(2, 10, FixtureImportance::DomesticCupFinal),
    ];
    let mut engine = CalendarEngine::new(42, season(vec![]), fixtures, competitions());
    engine.advance_bounded(11, false);

    let league = engine.fixtures.iter().find(|f| f.id == 1).unwrap();
    let cup = engine.fixtures.iter().find(|f| f.id == 2).unwrap();
    assert_eq!(
        league.scheduled_day, 10,
        "higher-priority league match holds the day"
    );
    assert_ne!(
        cup.scheduled_day, 10,
        "lower-priority cup match is bumped off day 10"
    );
    assert_eq!(
        cup.original_day, 10,
        "original_day is preserved as an audit trail"
    );
}

/// The bumped fixture always moves forward (to a later day), never backward.
#[test]
fn bumped_fixture_reschedules_forward_not_backward() {
    let fixtures = vec![
        league_fixture(1, 20),
        cup_fixture(2, 20, FixtureImportance::DomesticCupEarly),
    ];
    let mut engine = CalendarEngine::new(7, season(vec![]), fixtures, competitions());
    engine.advance_bounded(21, false);

    let cup = engine.fixtures.iter().find(|f| f.id == 2).unwrap();
    assert!(
        cup.scheduled_day > 20,
        "bumped fixture must move to a later day, got {}",
        cup.scheduled_day
    );
}

/// Two-legged ties reschedule together: if one leg is bumped, its partner shifts by
/// the same number of days rather than being left behind or moved independently.
#[test]
fn two_legged_tie_reschedules_together() {
    let leg1 = Fixture {
        id: 1,
        competition_id: 2,
        scheduled_day: 30,
        original_day: 30,
        is_orbit: true,
        importance: FixtureImportance::ContinentalTier1,
        leg_for_id: None,
    };
    let leg2 = Fixture {
        id: 2,
        competition_id: 2,
        scheduled_day: 44, // 2 weeks later, as a real two-legged tie would be
        original_day: 44,
        is_orbit: true,
        importance: FixtureImportance::ContinentalTier1,
        leg_for_id: Some(1), // leg 2 points back at leg 1
    };
    // A higher-priority league fixture clashes with leg 1 only.
    let clash = Fixture {
        id: 3,
        competition_id: 1,
        scheduled_day: 30,
        original_day: 30,
        is_orbit: true,
        importance: FixtureImportance::League,
        leg_for_id: None,
    };
    let mut competitions = competitions();
    // Continental tier 1 outranks league in the real ladder, but here we want the
    // league fixture to win so leg 1 (and therefore leg 2) gets bumped — flip the
    // competition priorities for this synthetic scenario.
    competitions[0].priority = 200; // league
    competitions[1].priority = 50; // continental (reused competition_id 2 slot)

    let fixtures = vec![leg1, leg2, clash];
    let mut engine = CalendarEngine::new(99, season(vec![]), fixtures, competitions);
    engine.advance_bounded(31, false);

    let new_leg1 = engine
        .fixtures
        .iter()
        .find(|f| f.id == 1)
        .unwrap()
        .scheduled_day;
    let new_leg2 = engine
        .fixtures
        .iter()
        .find(|f| f.id == 2)
        .unwrap()
        .scheduled_day;
    assert!(new_leg1 > 30, "leg 1 must be bumped off the clash day");
    let delta = new_leg1 - 30;
    assert_eq!(
        new_leg2,
        44 + delta,
        "leg 2 must shift by the same delta as leg 1, preserving the 2-leg spacing"
    );
}

/// A bumped fixture never lands inside an active calendar window.
#[test]
fn bumped_fixture_avoids_active_windows() {
    let fixtures = vec![
        league_fixture(1, 50),
        cup_fixture(2, 50, FixtureImportance::DomesticCupEarly),
    ];
    let windows = vec![CalendarWindow {
        kind: WindowKind::InternationalBreak,
        start_day: 51,
        end_day: 60,
    }];
    let mut engine = CalendarEngine::new(3, season(windows), fixtures, competitions());
    engine.advance_bounded(51, false);

    let cup = engine.fixtures.iter().find(|f| f.id == 2).unwrap();
    assert!(
        cup.scheduled_day > 60,
        "bumped fixture must clear the active window, got {}",
        cup.scheduled_day
    );
}

/// A bumped fixture keeps at least the minimum rest gap from every other orbit fixture
/// already on the calendar — it can't be shoved into the very next day if another
/// fixture is already close by.
#[test]
fn bumped_fixture_respects_min_rest_gap() {
    let fixtures = vec![
        league_fixture(1, 70),
        cup_fixture(2, 70, FixtureImportance::DomesticCupEarly),
        // Already-scheduled fixture just after the clash day — the bumped cup match
        // must not land within MIN_REST_GAP_DAYS of it.
        league_fixture(3, 72),
    ];
    let mut engine = CalendarEngine::new(11, season(vec![]), fixtures, competitions());
    engine.advance_bounded(71, false);

    let cup = engine.fixtures.iter().find(|f| f.id == 2).unwrap();
    assert!(
        cup.scheduled_day.abs_diff(72) >= 3,
        "bumped fixture must keep a minimum rest gap from day 72, got {}",
        cup.scheduled_day
    );
}

/// Conflict resolution is idempotent: re-resolving a day that's already down to one
/// orbit fixture is a no-op.
#[test]
fn no_clash_is_a_no_op() {
    let fixtures = vec![league_fixture(1, 5)];
    let mut engine = CalendarEngine::new(1, season(vec![]), fixtures, competitions());
    engine.advance_bounded(6, false);
    let f = engine.fixtures.iter().find(|f| f.id == 1).unwrap();
    assert_eq!(f.scheduled_day, 5, "a lone fixture never gets rescheduled");
}
