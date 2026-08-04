//! BL5.3 clutch index — pure-predicate tests over hand-built `MomentSummary`
//! fixtures. Clutch is the high-leverage SUBSET of decisive moments: equalizers
//! and go-ahead goals count, insurance goals don't; late defensive stops in a
//! one-goal game always count.

use goat_match::beats::ScoreEvent;
use goat_match::sim::{is_clutch, is_decisive, MomentSummary};

fn moment(
    minute: u32,
    before: (u32, u32),
    success: bool,
    success_event: Option<ScoreEvent>,
    failure_event: Option<ScoreEvent>,
    resolved: Option<ScoreEvent>,
) -> MomentSummary {
    MomentSummary {
        beat_id: "test".to_string(),
        minute,
        choice_idx: 0,
        success,
        setup_text: String::new(),
        outcome_text: String::new(),
        goal_event: resolved,
        goals_for_before: before.0,
        goals_against_before: before.1,
        success_event,
        failure_event,
    }
}

fn goal(minute: u32, before: (u32, u32)) -> MomentSummary {
    moment(
        minute,
        before,
        true,
        Some(ScoreEvent::GoalFor),
        None,
        Some(ScoreEvent::GoalFor),
    )
}

fn stop(minute: u32, before: (u32, u32)) -> MomentSummary {
    moment(
        minute,
        before,
        true,
        None,
        Some(ScoreEvent::GoalAgainst),
        None,
    )
}

/// Equalizer at 82' while trailing: decisive AND clutch.
#[test]
fn equalizer_while_trailing_is_clutch() {
    let m = goal(82, (0, 1));
    assert!(is_decisive(&m));
    assert!(is_clutch(&m));
}

/// Go-ahead goal at 88' from 1-1: decisive AND clutch.
#[test]
fn go_ahead_goal_from_level_is_clutch() {
    let m = goal(88, (1, 1));
    assert!(is_decisive(&m));
    assert!(is_clutch(&m));
}

/// Insurance goal at 85' while already 2-1 up: still decisive (late, close,
/// stakes) but NOT clutch — the lead was already there.
#[test]
fn insurance_goal_while_ahead_is_decisive_not_clutch() {
    let m = goal(85, (2, 1));
    assert!(is_decisive(&m));
    assert!(!is_clutch(&m), "extending an existing lead is not clutch");
}

/// Late defensive stops protecting a one-goal lead or a level score: clutch.
#[test]
fn late_stops_in_one_goal_games_are_clutch() {
    assert!(is_clutch(&stop(85, (1, 0))), "protecting a 1-0 lead");
    assert!(is_clutch(&stop(86, (1, 1))), "preserving a 1-1 scoreline");
    assert!(is_clutch(&stop(84, (0, 1))), "keeping a 0-1 game alive");
}

/// Anything that isn't decisive can't be clutch either (early minute here).
#[test]
fn non_decisive_is_never_clutch() {
    let m = goal(60, (1, 1));
    assert!(!is_decisive(&m));
    assert!(!is_clutch(&m));
}
