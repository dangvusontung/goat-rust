//! SPEC — the player clock stays synced to the season calendar
//! (tasks/TASK-CORE-time-model-age-sync.md).
//!
//! Before this fix, `age_weeks` only ticked inside `Intent::AdvanceWeek` (training),
//! so playing matches without training every single week left the player's age
//! drifting further and further behind the season calendar. The fix makes
//! `Intent::ApplyRoundResult` (untrained match weeks + skipped break weeks) and
//! `Intent::StartSeason` (the off-season back-fill) co-own the clock, converging
//! on one invariant every season boundary:
//!
//!   age_weeks == START_AGE_WEEKS + (season_number - 1) * 52
//!
//! `golden_week`'s frozen per-tick growth/injury/decay math is untouched — these
//! tests only exercise HOW OFTEN a tick runs, never what a tick does.

use goat_core::generation::CreationChoices;
use goat_core::positions::PrimaryPosition;
use goat_core::state::{reduce, Intent, WorldState};
use goat_core::tuning::START_AGE_WEEKS;
use goat_core::week::{Intensity, Routine};
use goat_rng::GoatRng;

fn new_career(seed: u64) -> WorldState {
    let choices = CreationChoices {
        name: "Clockwork".into(),
        primary_position: PrimaryPosition::ST,
        nationality: "English".to_string(),
        club: "Local FC".to_string(),
    };
    let mut s = WorldState::new();
    s = reduce(
        s,
        Intent::CreatePlayer { seed, choices },
        &mut GoatRng::new(0),
    );
    reduce(
        s,
        Intent::InitWorld {
            world_seed: seed,
            pc_club_idx: 0,
            pc_div_idx: 0,
            facilities_mult: goat_fixed::Fixed::ONE,
            initial_table: Box::new([0u32; 100]),
        },
        &mut GoatRng::new(0),
    )
}

fn age_weeks(state: &WorldState) -> u32 {
    state.players.get_age_weeks(state.pc_player_id.unwrap())
}

/// Pure off-season back-fill mechanism: zero rounds played, zero training — every
/// week of every season must still elapse via `StartSeason`'s rest-week back-fill.
#[test]
fn start_season_alone_backfills_to_the_yearly_target() {
    let mut state = new_career(1);
    for season in 1u32..=10 {
        state = reduce(
            state,
            Intent::StartSeason { fixtures: vec![] },
            &mut GoatRng::new(0),
        );
        assert_eq!(state.season_number, season);
        let expected = START_AGE_WEEKS + (season - 1) * 52;
        assert_eq!(
            age_weeks(&state),
            expected,
            "season {season}: age should be exactly {expected} weeks"
        );
    }
}

/// Realistic season: the player never trains and never plays a round (worst case
/// from the bug report — "you can play all 30 rounds at 16y 0w"). Round transitions
/// alone (via `ApplyRoundResult`'s untrained-week rest tick) plus the off-season
/// back-fill must still hit the invariant at every season boundary.
#[test]
fn untrained_rounds_still_sync_to_the_calendar() {
    let mut state = new_career(2);
    const ROUNDS_PER_SEASON: u32 = 30;

    for season in 1u32..=5 {
        state = reduce(
            state,
            Intent::StartSeason { fixtures: vec![] },
            &mut GoatRng::new(0),
        );
        assert_eq!(state.season_number, season);
        assert_eq!(
            age_weeks(&state),
            START_AGE_WEEKS + (season - 1) * 52,
            "season {season} start"
        );

        for round in 0..ROUNDS_PER_SEASON {
            // No AdvanceWeek intent fired here — the player never trains.
            state = reduce(
                state,
                Intent::ApplyRoundResult {
                    competition_id: goat_core::calendar_loop::LEAGUE_COMPETITION_ID,
                    pc_goals: 0,
                    pc_assists: 0,
                    pc_output: 0,
                    pc_result: 0,
                    round_results: Vec::new(),
                    // Not testing the exact goat-world break-week schedule here — a
                    // fixed small gap is enough to prove ApplyRoundResult's own rest
                    // tick plus the off-season back-fill still converge.
                    rest_weeks: if round % 5 == 4 { 1 } else { 0 },
                    week_ends: true, // synthetic schedule: every round its own week
                },
                &mut GoatRng::new(0),
            );
        }
        assert!(
            age_weeks(&state) < START_AGE_WEEKS + season * 52,
            "in-season ticking alone must never reach next season's target \
             (age {} at season {season} end)",
            age_weeks(&state)
        );
    }
}

/// Realistic season with disciplined weekly training (the intended loop): training
/// every week plus round transitions must NOT double-tick and must still land on
/// the exact yearly target once the off-season back-fill runs.
#[test]
fn trained_every_round_still_hits_exact_target() {
    let mut state = new_career(3);
    let routine = Routine {
        focus_attrs: vec![goat_core::attrs::AttrId::Finishing],
        intensity: Intensity::Medium,
    };
    state = reduce(state, Intent::SetRoutine { routine }, &mut GoatRng::new(0));
    const ROUNDS_PER_SEASON: u32 = 30;

    for season in 1u32..=5 {
        state = reduce(
            state,
            Intent::StartSeason { fixtures: vec![] },
            &mut GoatRng::new(0),
        );
        assert_eq!(
            age_weeks(&state),
            START_AGE_WEEKS + (season - 1) * 52,
            "season {season} start"
        );

        for round in 0..ROUNDS_PER_SEASON {
            state = reduce(
                state,
                Intent::AdvanceWeek,
                &mut GoatRng::new(round as u64 ^ (season as u64 * 0xbeef)),
            );
            state = reduce(
                state,
                Intent::ApplyRoundResult {
                    competition_id: goat_core::calendar_loop::LEAGUE_COMPETITION_ID,
                    pc_goals: 0,
                    pc_assists: 0,
                    pc_output: 60,
                    pc_result: 0,
                    round_results: Vec::new(),
                    rest_weeks: if round % 5 == 4 { 1 } else { 0 },
                    week_ends: true, // synthetic schedule: every round its own week
                },
                &mut GoatRng::new(0),
            );
        }
        assert!(
            age_weeks(&state) <= START_AGE_WEEKS + season * 52,
            "in-season ticking (training + rounds) must never overshoot next \
             season's target (age {} at season {season} end)",
            age_weeks(&state)
        );
    }
}
