//! Golden-seed and determinism tests for `goat-training`.
//!
//! FROZEN values are marked with `// FROZEN:` comments and must NEVER be changed
//! to make a failing test pass. If a golden test fails, the production change is wrong.
//!
//! Values were determined by running the tests with `-- --nocapture` and copying
//! the printed actuals into the assert_eq! once the user approved them.

use goat_calendar::clock::Fixture;
use goat_calendar::subsystem::{DayContext, Subsystem};
use goat_core::attrs::AttrId;
use goat_core::player::PlayerView;
use goat_core::roles::{FamiliarityTier, NUM_ROLES};
use goat_fixed::Fixed;
use goat_rng::GoatRng;
use goat_training::{AttrTarget, Intensity, TrainingRoutine, TrainingSubsystem};

// ── Test fixtures ─────────────────────────────────────────────────────────────

/// Build a 17-year-old forward player with uniform attrs at `current`/`potential`.
fn make_player_17(current: i32, potential: i32, energy: i32) -> PlayerView {
    let mut view = PlayerView::default();
    view.name = "GoldenPlayer".into();
    for a in 0..goat_core::attrs::NUM_ATTRS {
        view.current[a] = Fixed::from_int(current);
        view.potential[a] = Fixed::from_int(potential);
    }
    view.familiarity = [FamiliarityTier::Awkward; NUM_ROLES];
    view.age_weeks = 17 * 52; // 884 weeks = 17 years (§5.1 young technical trainability)
    view.energy = Fixed::from_int(energy);
    view.injury_weeks = 0;
    view
}

fn make_training_ctx(epoch_day: u32, is_match: bool) -> DayContext {
    DayContext {
        epoch_day,
        season_id: 1,
        todays_fixtures: if is_match {
            vec![Fixture {
                id: 0,
                competition_id: 1,
                scheduled_day: epoch_day,
                original_day: epoch_day,
                is_orbit: true,
            }]
        } else {
            vec![]
        },
        active_windows: vec![],
        days_until_next_fixture: 7,
        congestion_score: 0,
    }
}

/// Build a scripted season snippet: 60 days with matches at days 7, 21, 40 and
/// rest days at 14–15, 28–29. All other days are training days.
/// Returns `(is_match_day, is_rest_day)` for each day in `0..60`.
fn scripted_day(day: u32) -> (bool, bool) {
    let match_days = [7u32, 21, 40];
    let rest_days = [14u32, 15, 28, 29];
    let is_match = match_days.contains(&day);
    let is_rest = rest_days.contains(&day);
    (is_match, is_rest)
}

// ── Golden-seed test (FROZEN after user approval) ─────────────────────────────

/// Run 60 scripted days and print actuals so they can be frozen.
///
/// Seed = 42. Routine: Single(Finishing) at Moderate intensity.
/// Player: 17-year-old, Finishing current=60, potential=85, energy=90.
/// All other attrs at 60/85 (only Finishing is targeted).
///
/// After first run, copy printed values into `golden_seed_frozen` below.
#[test]
fn golden_seed_print_actuals() {
    let player = make_player_17(60, 85, 90);
    let routine = TrainingRoutine {
        target: Some(AttrTarget::Single(AttrId::Finishing)),
        intensity: Intensity::Moderate,
    };
    let mut sub = TrainingSubsystem::new(player, routine.clone(), Fixed::ONE);

    // Seed 42: calendar's rng advances 1 roll per day; training derives its
    // per-day rng from that roll XOR TRAINING_DOMAIN_HASH.
    let mut cal_rng = GoatRng::new(42);
    let mut flashpoint_days: Vec<u32> = Vec::new();

    for day in 0u32..60 {
        let (is_match, is_rest) = scripted_day(day);
        if is_rest {
            sub.set_routine(TrainingRoutine::rest());
        } else {
            sub.set_routine(routine.clone());
        }
        let ctx = make_training_ctx(day, is_match);
        let report = sub.on_day(&ctx, &mut cal_rng);
        if report.stop_class == goat_calendar::subsystem::StopClass::SoftFlashpoint {
            flashpoint_days.push(day);
        }
    }

    let final_finishing = sub.get_current(AttrId::Finishing);
    let final_energy = sub.energy();

    println!("=== GOLDEN ACTUALS (seed=42, 60 days) ===");
    println!("final_finishing raw = {}", final_finishing.to_raw());
    println!("final_finishing int = {}", final_finishing.to_int());
    println!("final_energy raw    = {}", final_energy.to_raw());
    println!("flashpoint_days     = {:?}", flashpoint_days);
    println!("==========================================");
}

/// FROZEN golden-seed test.
///
/// Copy the values from `golden_seed_print_actuals` output here.
/// These values MUST NOT be changed to fix a failing test — if they fail,
/// a production change broke determinism (§9) or a previously correct
/// golden value was altered (CLAUDE.md: "Frozen golden values").
#[test]
fn golden_seed_frozen() {
    let player = make_player_17(60, 85, 90);
    let routine = TrainingRoutine {
        target: Some(AttrTarget::Single(AttrId::Finishing)),
        intensity: Intensity::Moderate,
    };
    let mut sub = TrainingSubsystem::new(player, routine.clone(), Fixed::ONE);
    let mut cal_rng = GoatRng::new(42);
    let mut flashpoint_days: Vec<u32> = Vec::new();

    for day in 0u32..60 {
        let (is_match, is_rest) = scripted_day(day);
        if is_rest {
            sub.set_routine(TrainingRoutine::rest());
        } else {
            sub.set_routine(routine.clone());
        }
        let ctx = make_training_ctx(day, is_match);
        let report = sub.on_day(&ctx, &mut cal_rng);
        if report.stop_class == goat_calendar::subsystem::StopClass::SoftFlashpoint {
            flashpoint_days.push(day);
        }
    }

    let final_finishing = sub.get_current(AttrId::Finishing);
    let final_energy = sub.energy();

    // FROZEN: exact raw Fixed values from first approved run (seed=42, 60-day sequence).
    // DO NOT UPDATE these to fix a failing test — a failing golden test means the
    // change broke determinism or altered a previously correct value (CLAUDE.md).
    //
    // Context: only 4 rest days in 60 days at Moderate intensity; energy drains to 0
    // by ~day 12 and mostly stays there. Growth is small (~0.799 total) because the
    // headroom-scaled formula reduces gains when within 30 pts of ceiling, and low-energy
    // days apply the 0.600 floor factor. This demonstrates the energy-management trade-off.
    assert_eq!(
        final_finishing.to_raw(),
        60_799,
        "FROZEN: final Finishing raw value"
    );
    assert_eq!(
        final_energy.to_raw(),
        0,
        "FROZEN: final energy raw value (energy drained)"
    );
    assert_eq!(
        flashpoint_days,
        Vec::<u32>::new(),
        "FROZEN: no flashpoints in this sequence"
    );
}

// ── Determinism test ──────────────────────────────────────────────────────────

/// Same seed + same scripted sequence must produce byte-identical final state (§2.3).
///
/// Two independent runs of the identical 60-day sequence. Both must yield the
/// same Finishing value and energy — no wall-clock, no global state, no HashMap
/// iteration can have affected the result.
#[test]
fn determinism_same_seed_same_state() {
    fn run_sequence(seed: u64) -> (i32, i32, Vec<u32>) {
        let player = make_player_17(60, 85, 90);
        let routine = TrainingRoutine {
            target: Some(AttrTarget::Single(AttrId::Finishing)),
            intensity: Intensity::Moderate,
        };
        let mut sub = TrainingSubsystem::new(player, routine.clone(), Fixed::ONE);
        let mut cal_rng = GoatRng::new(seed);
        let mut flashpoint_days: Vec<u32> = Vec::new();

        for day in 0u32..60 {
            let (is_match, is_rest) = scripted_day(day);
            if is_rest {
                sub.set_routine(TrainingRoutine::rest());
            } else {
                sub.set_routine(routine.clone());
            }
            let ctx = make_training_ctx(day, is_match);
            let report = sub.on_day(&ctx, &mut cal_rng);
            if report.stop_class == goat_calendar::subsystem::StopClass::SoftFlashpoint {
                flashpoint_days.push(day);
            }
        }

        (
            sub.get_current(AttrId::Finishing).to_raw(),
            sub.energy().to_raw(),
            flashpoint_days,
        )
    }

    let result_a = run_sequence(42);
    let result_b = run_sequence(42);

    assert_eq!(
        result_a, result_b,
        "same seed must produce byte-identical state"
    );
}

/// Different seeds must produce different final attribute values.
/// (Sanity check that seeding actually propagates into training RNG.)
#[test]
fn different_seeds_produce_different_results() {
    fn run_sequence(seed: u64) -> i32 {
        let player = make_player_17(60, 85, 90);
        let routine = TrainingRoutine {
            target: Some(AttrTarget::Single(AttrId::Finishing)),
            intensity: Intensity::Moderate,
        };
        let mut sub = TrainingSubsystem::new(player, routine.clone(), Fixed::ONE);
        let mut cal_rng = GoatRng::new(seed);

        for day in 0u32..60 {
            let (is_match, is_rest) = scripted_day(day);
            if is_rest {
                sub.set_routine(TrainingRoutine::rest());
            } else {
                sub.set_routine(routine.clone());
            }
            let ctx = make_training_ctx(day, is_match);
            sub.on_day(&ctx, &mut cal_rng);
        }

        sub.get_current(AttrId::Finishing).to_raw()
    }

    let a = run_sequence(1);
    let b = run_sequence(2);
    // With 60 days of ±0.015 jitter, the final values should differ across seeds.
    assert_ne!(
        a, b,
        "different seeds should produce different final values"
    );
}

// ── Season-length sanity test ─────────────────────────────────────────────────

/// Full 365-day season with matches every 12 days must not panic and must
/// preserve all invariants (current ≤ potential, energy in [0,100]).
#[test]
fn full_season_invariants_hold() {
    let player = make_player_17(30, 99, 90);
    let routine = TrainingRoutine {
        target: Some(AttrTarget::Single(AttrId::Finishing)),
        intensity: Intensity::Moderate,
    };
    let mut sub = TrainingSubsystem::new(player, routine.clone(), Fixed::ONE);
    let mut cal_rng = GoatRng::new(7);

    // Matches every 12 days; rest days at day 6, 13, 20, ...
    for day in 0u32..365 {
        let is_match = day % 12 == 0 && day > 0;
        let is_rest = day % 7 == 6;
        if is_rest {
            sub.set_routine(TrainingRoutine::rest());
        } else {
            sub.set_routine(routine.clone());
        }
        let ctx = make_training_ctx(day, is_match);
        sub.on_day(&ctx, &mut cal_rng);

        // Invariant: energy in [0.0, 100.0]
        let e = sub.energy();
        assert!(e >= Fixed::ZERO, "energy below 0 on day {day}: {e:?}");
        assert!(
            e <= Fixed::from_int(100),
            "energy above 100 on day {day}: {e:?}"
        );
    }

    // Invariant: Finishing grew (net positive over a full season of training).
    let final_finishing = sub.get_current(AttrId::Finishing);
    assert!(
        final_finishing > Fixed::from_int(30),
        "Finishing must grow over a full season: {final_finishing:?}"
    );

    // Invariant: current ≤ potential for all attrs.
    for a in 0..goat_core::attrs::NUM_ATTRS {
        let cur = sub.players.get_current(sub.orbit_id, a);
        let pot = sub.players.get_potential(sub.orbit_id, a);
        assert!(
            cur <= pot,
            "current ({cur:?}) > potential ({pot:?}) for attr {a}"
        );
    }
}
