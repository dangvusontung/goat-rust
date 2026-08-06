//! BL5.2 decisive-moment detection — pure-predicate golden tests over hand-built
//! `MomentSummary` fixtures (no live match, world, or save needed), mirroring how
//! `resolve_contest`/`injury_prob` are tested in isolation.
//!
//! Covers both scenarios Tùng verified in the task doc, plus negative cases
//! (blowout scoreline, too-early minute, non-stakes beat, actual concession).

use goat_match::beats::ScoreEvent;
use goat_match::sim::{is_decisive, MomentSummary, DECISIVE_MINUTE_CUTOFF};

/// A stakes-bearing attacking moment: success branch carries `score_success`,
/// failure branch none; resolved outcome/success/score/minute as given.
fn attacking_moment(
    minute: u32,
    before: (u32, u32),
    success: bool,
    score_success: Option<ScoreEvent>,
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
        success_event: score_success,
        failure_event: None,
    }
}

/// A defensive moment: failure branch carries `score_failure`; resolved as given.
fn defensive_moment(
    minute: u32,
    before: (u32, u32),
    success: bool,
    score_failure: Option<ScoreEvent>,
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
        success_event: None,
        failure_event: score_failure,
    }
}

/// Tùng's scenario 1: trailing 0-1 at 82', score twice to win 2-1 — BOTH goals
/// count, each judged against the score before it (0-1, then 1-1).
#[test]
fn comeback_goals_late_in_close_game_both_count() {
    let first = attacking_moment(
        82,
        (0, 1),
        true,
        Some(ScoreEvent::GoalFor),
        Some(ScoreEvent::GoalFor),
    );
    let second = attacking_moment(
        88,
        (1, 1),
        true,
        Some(ScoreEvent::GoalFor),
        Some(ScoreEvent::GoalFor),
    );
    assert!(is_decisive(&first), "equalizer at 82' while trailing 0-1");
    assert!(is_decisive(&second), "winner at 88' from 1-1");
}

/// Tùng's scenario 2: leading 1-0, last-ditch tackle at ~85' — the defense holds
/// (no GoalAgainst happens). Counts regardless of any card shown (cards are an
/// independent roll after contest resolution, so they never enter this check).
#[test]
fn late_defensive_stand_in_close_game_counts_card_or_not() {
    let stand = defensive_moment(85, (1, 0), true, Some(ScoreEvent::GoalAgainst), None);
    assert!(is_decisive(&stand), "defensive stand at 85' protecting 1-0");
}

/// An assist (teammate finishes) counts as "scored" the same as a goal (BL5.1 split).
#[test]
fn late_assist_in_close_game_counts() {
    let assist = attacking_moment(
        83,
        (1, 1),
        true,
        Some(ScoreEvent::AssistFor),
        Some(ScoreEvent::AssistFor),
    );
    assert!(is_decisive(&assist));
}

/// Boundary: the cutoff minute itself counts, one minute earlier does not.
#[test]
fn minute_cutoff_is_inclusive() {
    let at = attacking_moment(
        DECISIVE_MINUTE_CUTOFF,
        (1, 1),
        true,
        Some(ScoreEvent::GoalFor),
        Some(ScoreEvent::GoalFor),
    );
    let before = attacking_moment(
        DECISIVE_MINUTE_CUTOFF - 1,
        (1, 1),
        true,
        Some(ScoreEvent::GoalFor),
        Some(ScoreEvent::GoalFor),
    );
    assert!(is_decisive(&at));
    assert!(!is_decisive(&before), "minute 79 is too early");
}

/// Negative: blowout — a goal at 85' with the score already 3-0 means nothing.
#[test]
fn blowout_goal_does_not_count() {
    let m = attacking_moment(
        85,
        (3, 0),
        true,
        Some(ScoreEvent::GoalFor),
        Some(ScoreEvent::GoalFor),
    );
    assert!(!is_decisive(&m), "gap of 3 exceeds the closeness threshold");
    let trailing = attacking_moment(
        85,
        (0, 2),
        true,
        Some(ScoreEvent::GoalFor),
        Some(ScoreEvent::GoalFor),
    );
    assert!(
        !is_decisive(&trailing),
        "consolation at 0-2 is not close enough either"
    );
}

/// Negative: a non-stakes beat (neither branch could produce a goal) never counts,
/// even late and close.
#[test]
fn non_stakes_beat_does_not_count() {
    let m = attacking_moment(85, (1, 1), true, None, None);
    assert!(!is_decisive(&m));
}

/// Negative: the defensive stand that FAILED — GoalAgainst actually happened —
/// is a concession, not a decisive moment for the PC's side.
#[test]
fn actual_concession_does_not_count() {
    let m = defensive_moment(
        85,
        (1, 0),
        false,
        Some(ScoreEvent::GoalAgainst),
        Some(ScoreEvent::GoalAgainst),
    );
    assert!(!is_decisive(&m));
}
