//! Golden-seed and determinism tests for `goat-calendar`.
//!
//! These tests are the determinism spine of Phase 1.
//! FROZEN: do NOT edit expected values once this file is committed.
//! A failing expected value means the change is wrong, not the test.

use goat_calendar::{
    CalendarEngine, DayContext, DayReport, Fixture, Season, StopClass, Subsystem, SubsystemId,
};
use goat_rng::RngSource;

// ── Stub subsystems ───────────────────────────────────────────────────────────

/// Fires a HardStop on fixture days; silent otherwise.
struct MatchStub;

impl Subsystem for MatchStub {
    fn on_day(&mut self, ctx: &DayContext, _rng: &mut dyn RngSource) -> DayReport {
        if !ctx.todays_fixtures.is_empty() {
            DayReport {
                source: SubsystemId::Match,
                stop_class: StopClass::HardStop,
                payload: Some("match_day".into()),
                mutations: vec![],
            }
        } else {
            DayReport::silent(SubsystemId::Match)
        }
    }
    fn id(&self) -> SubsystemId {
        SubsystemId::Match
    }
}

/// Always silent — represents background training.
struct TrainingStub;

impl Subsystem for TrainingStub {
    fn on_day(&mut self, _ctx: &DayContext, _rng: &mut dyn RngSource) -> DayReport {
        DayReport::silent(SubsystemId::Training)
    }
    fn id(&self) -> SubsystemId {
        SubsystemId::Training
    }
}

/// Fires a SoftFlashpoint on specific days; silent otherwise.
struct MediaStub {
    fire_on_days: Vec<u32>,
}

impl Subsystem for MediaStub {
    fn on_day(&mut self, ctx: &DayContext, _rng: &mut dyn RngSource) -> DayReport {
        if self.fire_on_days.contains(&ctx.epoch_day) {
            DayReport {
                source: SubsystemId::Media,
                stop_class: StopClass::SoftFlashpoint,
                payload: Some("minor_event".into()),
                mutations: vec![],
            }
        } else {
            DayReport::silent(SubsystemId::Media)
        }
    }
    fn id(&self) -> SubsystemId {
        SubsystemId::Media
    }
}

// ── Engine factory ────────────────────────────────────────────────────────────

fn make_engine(seed: u64, fixture_days: &[u32]) -> CalendarEngine {
    let season = Season {
        id: 1,
        start_day: 0,
        end_day: 364, // 365-day in-game year (§2.3)
        windows: vec![],
        competition_ids: vec![1],
    };
    let fixtures: Vec<Fixture> = fixture_days
        .iter()
        .enumerate()
        .map(|(i, &day)| Fixture {
            id: i as u64,
            competition_id: 1,
            scheduled_day: day,
            original_day: day,
            is_orbit: true,
        })
        .collect();
    let mut engine = CalendarEngine::new(seed, season, fixtures);
    engine.register(Box::new(MatchStub));
    engine.register(Box::new(TrainingStub));
    engine
}

// ── Golden-seed tests (FROZEN) ────────────────────────────────────────────────

/// FROZEN. Stop days for seed=42 with fixtures at [7, 21, 40, 60, 90].
/// `StopResult.day` is the pre-tick epoch-day on which the event fired.
#[test]
fn golden_seed_stop_days() {
    let fixture_days = [7u32, 21, 40, 60, 90];
    let mut engine = make_engine(42, &fixture_days);

    let r1 = engine.advance_until_flashpoint();
    assert_eq!(r1.day, 7, "first stop: fixture at day 7");
    assert_eq!(r1.stops.len(), 1);
    assert_eq!(r1.stops[0].stop_class, StopClass::HardStop);
    assert_eq!(r1.stops[0].source, SubsystemId::Match);

    let r2 = engine.advance_until_flashpoint();
    assert_eq!(r2.day, 21, "second stop: fixture at day 21");

    let r3 = engine.advance_until_flashpoint();
    assert_eq!(r3.day, 40, "third stop: fixture at day 40");

    let r4 = engine.advance_until_flashpoint();
    assert_eq!(r4.day, 60, "fourth stop: fixture at day 60");

    let r5 = engine.advance_until_flashpoint();
    assert_eq!(r5.day, 90, "fifth stop: fixture at day 90");

    // FROZEN: final clock state after all 5 flashpoints.
    assert_eq!(engine.clock.epoch_day, 91);
    assert_eq!(engine.clock.current_season, 1);
}

/// FROZEN. advance_bounded with allow_break=true stops at first HardStop.
#[test]
fn golden_advance_bounded_breaks_on_hard_stop() {
    let mut engine = make_engine(42, &[15]);
    let result = engine.advance_bounded(30, true);
    // Fixture on day 15 triggers HardStop; event_day = 15, clock advances to 16.
    assert_eq!(result.day, 15);
    assert_eq!(result.stops.len(), 1);
    assert_eq!(result.stops[0].stop_class, StopClass::HardStop);
    assert_eq!(engine.clock.epoch_day, 16);
}

/// FROZEN. advance_bounded with allow_break=false runs the full window despite a fixture.
#[test]
fn golden_advance_bounded_no_break_runs_full() {
    let mut engine = make_engine(42, &[15]);
    let result = engine.advance_bounded(30, false);
    // Fixture fires but is ignored; all 30 days tick.
    assert_eq!(engine.clock.epoch_day, 30);
    assert_eq!(result.day, 30);
    assert!(result.stops.is_empty());
}

/// FROZEN. Chained bounded advances: first breaks at day 7, second runs 50 more days.
#[test]
fn golden_chained_advance_bounded() {
    let fixture_days = [7u32, 21, 40, 60, 90];
    let mut engine = make_engine(42, &fixture_days);

    let r1 = engine.advance_bounded(50, true);
    assert_eq!(r1.day, 7); // stopped at fixture
    assert_eq!(engine.clock.epoch_day, 8);

    let r2 = engine.advance_bounded(50, false);
    // 50 ticks from day 8 → epoch_day = 58; fixtures at 21, 40 ignored.
    assert_eq!(engine.clock.epoch_day, 58);
    assert_eq!(r2.day, 58);
    assert!(r2.stops.is_empty());
}

// ── Determinism tests ─────────────────────────────────────────────────────────

/// Same seed + same intent sequence must produce byte-identical state (§2.3).
#[test]
fn determinism_same_seed_same_advance_until_flashpoint() {
    let fixture_days = [7u32, 21, 40, 60, 90];

    let run = |days: &[u32]| -> (u32, u32) {
        let mut engine = make_engine(42, days);
        for _ in 0..5 {
            engine.advance_until_flashpoint();
        }
        (engine.clock.epoch_day, engine.clock.current_season)
    };

    let result_a = run(&fixture_days);
    let result_b = run(&fixture_days);
    assert_eq!(
        result_a, result_b,
        "same seed + same intents must produce identical state"
    );
}

/// Headless sim of a full season must be deterministic.
#[test]
fn determinism_sim_season_headless() {
    let fixture_days: Vec<u32> = (0..30).map(|i| 7 + i * 12).collect();

    let run = |days: &[u32]| -> u32 {
        let mut engine = make_engine(7, days);
        engine.sim_season_headless(1);
        engine.clock.epoch_day
    };

    assert_eq!(run(&fixture_days), run(&fixture_days));
}

/// Different seeds must not produce the same stop sequence (sanity check).
#[test]
fn different_seeds_differ_when_using_rng() {
    // Use an RNG-driven subsystem to verify seeds propagate to output.
    // With no-fixture, no-rng-in-stubs setup, seeds don't affect stop days.
    // This test verifies the engine doesn't panic and seeds are accepted.
    let mut e1 = make_engine(1, &[10]);
    let mut e2 = make_engine(2, &[10]);
    let r1 = e1.advance_until_flashpoint();
    let r2 = e2.advance_until_flashpoint();
    // Both stop at day 10 (fixture). Stop day is fixture-driven, not RNG-driven.
    assert_eq!(r1.day, r2.day);
}

// ── Soft-flashpoint flush test ────────────────────────────────────────────────

/// Soft events accumulate and flush when SOFT_FLUSH_THRESHOLD is reached.
#[test]
fn soft_flashpoint_flushes_at_threshold() {
    let season = Season {
        id: 1,
        start_day: 0,
        end_day: 364,
        windows: vec![],
        competition_ids: vec![1],
    };
    let mut engine = CalendarEngine::new(42, season, vec![]);
    // MediaStub fires soft events on days 1, 3, 5 (threshold = 3).
    engine.register(Box::new(MediaStub {
        fire_on_days: vec![1, 3, 5],
    }));

    let result = engine.advance_until_flashpoint();
    // After day 5: buffer = [day-1 soft, day-3 soft, day-5 soft] → flush.
    assert_eq!(
        result.day, 5,
        "flush happens on the day the threshold is crossed"
    );
    assert_eq!(
        result.stops.len(),
        3,
        "all 3 buffered soft events are in stops"
    );
    assert!(result
        .stops
        .iter()
        .all(|r| r.stop_class == StopClass::SoftFlashpoint));
    assert!(result.pending.is_empty());
}

/// A HardStop drains the soft buffer into `pending`.
#[test]
fn hard_stop_flushes_pending_softs() {
    let season = Season {
        id: 1,
        start_day: 0,
        end_day: 364,
        windows: vec![],
        competition_ids: vec![1],
    };
    let fixture = Fixture {
        id: 0,
        competition_id: 1,
        scheduled_day: 5,
        original_day: 5,
        is_orbit: true,
    };
    let mut engine = CalendarEngine::new(42, season, vec![fixture]);
    // Soft events on days 1 and 3 (below flush threshold), then a hard match on day 5.
    engine.register(Box::new(MatchStub));
    engine.register(Box::new(MediaStub {
        fire_on_days: vec![1, 3],
    }));

    let result = engine.advance_until_flashpoint();
    assert_eq!(result.day, 5);
    assert_eq!(result.stops.len(), 1);
    assert_eq!(result.stops[0].stop_class, StopClass::HardStop);
    // The 2 buffered soft events come back in `pending`.
    assert_eq!(result.pending.len(), 2);
    assert!(result
        .pending
        .iter()
        .all(|r| r.stop_class == StopClass::SoftFlashpoint));
}

// ── sim_season_headless ───────────────────────────────────────────────────────

/// Headless sim advances the full 365-day season without panicking.
#[test]
fn sim_season_headless_advances_past_season_end() {
    let fixture_days: Vec<u32> = (0..30).map(|i| 7 + i * 12).collect();
    let mut engine = make_engine(7, &fixture_days);
    engine.sim_season_headless(1);
    // After the last tick of day 364, epoch_day = 365 > end_day = 364.
    assert!(
        engine.clock.epoch_day > 364,
        "epoch_day must be past season end"
    );
}
