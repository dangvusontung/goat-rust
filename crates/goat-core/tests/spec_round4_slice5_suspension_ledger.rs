//! Design round 4, Slice 5 §5.1 — `SuspensionLedger` per-competition scoping.
//!
//! The regression this slice exists to prevent: before this slice, `pc_suspension_weeks`
//! was one single global scalar, so a ban earned in any competition blocked selection in
//! every competition. These tests prove a ban is scoped to the exact competition it was
//! earned in.

use goat_core::{
    calendar_loop::{DOMESTIC_CUP_COMPETITION_ID, LEAGUE_COMPETITION_ID},
    generation::CreationChoices,
    positions::PrimaryPosition,
    state::{reduce, Intent, WorldState},
};
use goat_rng::GoatRng;

fn base_state() -> WorldState {
    let choices = CreationChoices {
        name: "Test Legend".into(),
        primary_position: PrimaryPosition::ST,
        nationality: "England".to_string(),
        club: "Burnley".to_string(),
    };
    let mut s = WorldState::new();
    s = reduce(
        s,
        Intent::CreatePlayer { seed: 42, choices },
        &mut GoatRng::new(0),
    );
    s
}

/// The concrete regression this slice exists to prevent: suspend the PC in the
/// domestic cup, confirm their next league fixture still selects them normally.
#[test]
fn suspension_in_one_competition_does_not_block_availability_in_another() {
    let mut s = base_state();
    assert_eq!(s.pc_suspension_matches_remaining(LEAGUE_COMPETITION_ID), 0);
    assert_eq!(
        s.pc_suspension_matches_remaining(DOMESTIC_CUP_COMPETITION_ID),
        0
    );

    // Red card in a domestic-cup tie.
    s = reduce(
        s,
        Intent::ApplyCardResult {
            competition_id: DOMESTIC_CUP_COMPETITION_ID,
            yellow_cards: 0,
            red_card: true,
        },
        &mut GoatRng::new(0),
    );
    assert!(
        s.pc_suspension_matches_remaining(DOMESTIC_CUP_COMPETITION_ID) >= 1,
        "red card in the cup must suspend the PC from the cup"
    );
    assert_eq!(
        s.pc_suspension_matches_remaining(LEAGUE_COMPETITION_ID),
        0,
        "a cup suspension must never bleed into the league"
    );

    // The PC's next league fixture is resolved — must select them normally (no league
    // ban exists), and must NOT touch the cup suspension at all (matches actually
    // played in the league don't serve a cup ban).
    s = reduce(
        s,
        Intent::ApplyRoundResult {
            competition_id: LEAGUE_COMPETITION_ID,
            pc_goals: 1,
            pc_assists: 0,
            pc_decisive_count: 0,
            pc_clutch_count: 0,
            fixture_importance: goat_calendar::FixtureImportance::League,
            pc_output: 70,
            pc_result: 1,
            round_results: Vec::new(),
            rest_weeks: 0,
            week_ends: true,
        },
        &mut GoatRng::new(0),
    );
    assert_eq!(
        s.pc_suspension_matches_remaining(LEAGUE_COMPETITION_ID),
        0,
        "PC was never banned from the league — still available"
    );
    let cup_remaining_before = s.pc_suspension_matches_remaining(DOMESTIC_CUP_COMPETITION_ID);
    assert!(
        cup_remaining_before >= 1,
        "the league round resolving must not serve the cup's suspension"
    );

    // Now resolve a domestic-cup fixture: THIS must serve the cup ban.
    s = reduce(
        s,
        Intent::ApplyOrbitMatchResult {
            competition_id: DOMESTIC_CUP_COMPETITION_ID,
        },
        &mut GoatRng::new(0),
    );
    assert_eq!(
        s.pc_suspension_matches_remaining(DOMESTIC_CUP_COMPETITION_ID),
        cup_remaining_before - 1,
        "a domestic-cup fixture being played must serve exactly one match of the cup ban"
    );
}

/// A PC suspended in the league only must still be selectable in the cup.
#[test]
fn league_suspension_does_not_block_cup_availability() {
    let mut s = base_state();
    s = reduce(
        s,
        Intent::ApplyCardResult {
            competition_id: LEAGUE_COMPETITION_ID,
            yellow_cards: 0,
            red_card: true,
        },
        &mut GoatRng::new(0),
    );
    assert!(s.pc_suspension_matches_remaining(LEAGUE_COMPETITION_ID) >= 1);
    assert_eq!(
        s.pc_suspension_matches_remaining(DOMESTIC_CUP_COMPETITION_ID),
        0,
        "a league suspension must never bleed into the cup"
    );
}

/// Suspensions in two different competitions are tracked independently and don't
/// interfere with each other's countdown.
#[test]
fn simultaneous_suspensions_in_two_competitions_decrement_independently() {
    let mut s = base_state();
    s = reduce(
        s,
        Intent::ApplyCardResult {
            competition_id: LEAGUE_COMPETITION_ID,
            yellow_cards: 0,
            red_card: true,
        },
        &mut GoatRng::new(0),
    );
    s = reduce(
        s,
        Intent::ApplyCardResult {
            competition_id: DOMESTIC_CUP_COMPETITION_ID,
            yellow_cards: 0,
            red_card: true,
        },
        &mut GoatRng::new(0),
    );
    let league_before = s.pc_suspension_matches_remaining(LEAGUE_COMPETITION_ID);
    let cup_before = s.pc_suspension_matches_remaining(DOMESTIC_CUP_COMPETITION_ID);
    assert!(league_before >= 1 && cup_before >= 1);

    // Serve a league match only.
    s = reduce(
        s,
        Intent::ApplyRoundResult {
            competition_id: LEAGUE_COMPETITION_ID,
            pc_goals: 0,
            pc_assists: 0,
            pc_decisive_count: 0,
            pc_clutch_count: 0,
            fixture_importance: goat_calendar::FixtureImportance::League,
            pc_output: 0,
            pc_result: 0,
            round_results: Vec::new(),
            rest_weeks: 0,
            week_ends: true,
        },
        &mut GoatRng::new(0),
    );
    assert_eq!(
        s.pc_suspension_matches_remaining(LEAGUE_COMPETITION_ID),
        league_before - 1
    );
    assert_eq!(
        s.pc_suspension_matches_remaining(DOMESTIC_CUP_COMPETITION_ID),
        cup_before,
        "the cup ban must be untouched by a league round resolving"
    );
}

/// A served-out suspension (0 matches remaining) is removed from the ledger, not left
/// as a dangling zero entry — keeps the ledger genuinely tiny in the common case.
#[test]
fn served_out_suspension_is_removed_from_the_ledger() {
    let mut s = base_state();
    s = reduce(
        s,
        Intent::ApplyCardResult {
            competition_id: LEAGUE_COMPETITION_ID,
            yellow_cards: 0,
            red_card: false,
        },
        &mut GoatRng::new(0),
    );
    // No card at all this time -- yellow accumulation path: force a 5th yellow.
    s.pc_yellow_cards_season = 4;
    s = reduce(
        s,
        Intent::ApplyCardResult {
            competition_id: LEAGUE_COMPETITION_ID,
            yellow_cards: 1,
            red_card: false,
        },
        &mut GoatRng::new(0),
    );
    assert_eq!(s.pc_suspension_matches_remaining(LEAGUE_COMPETITION_ID), 1);
    s = reduce(
        s,
        Intent::ApplyRoundResult {
            competition_id: LEAGUE_COMPETITION_ID,
            pc_goals: 0,
            pc_assists: 0,
            pc_decisive_count: 0,
            pc_clutch_count: 0,
            fixture_importance: goat_calendar::FixtureImportance::League,
            pc_output: 0,
            pc_result: 0,
            round_results: Vec::new(),
            rest_weeks: 0,
            week_ends: true,
        },
        &mut GoatRng::new(0),
    );
    assert_eq!(s.pc_suspension_matches_remaining(LEAGUE_COMPETITION_ID), 0);
    assert!(
        s.pc_suspensions.is_empty(),
        "a fully-served ban must be dropped from the ledger, not left at 0 remaining"
    );
}
