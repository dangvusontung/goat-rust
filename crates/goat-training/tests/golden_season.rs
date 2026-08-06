//! Golden-seed + determinism tests for the training subsystem (appendix Step 3).
//!
//! The scripted season: 266 days (38 weeks) — train Mon–Fri (days % 7 in 0..=4,
//! Moderate routine on ShortPassing), match on day % 7 == 5 (38 orbit fixtures),
//! rest on day % 7 == 6 (routine None). Deterministic in (seed, script).

use goat_calendar::subsystem::{DayContext, DayReport, StopClass, Subsystem};
use goat_core::attrs::{AttrId, NUM_ATTRS};
use goat_fixed::Fixed;
use goat_training::{Intensity, Training, TrainingRoutine};

const SEED: u64 = 42;
const DAYS: u32 = 266;

fn fresh_training() -> Training {
    let mut current = [Fixed::from_int(50); NUM_ATTRS];
    let mut potential = [Fixed::from_int(70); NUM_ATTRS];
    current[AttrId::ShortPassing as usize] = Fixed::from_int(60);
    potential[AttrId::ShortPassing as usize] = Fixed::from_int(85);
    let mut t = Training::new(SEED, current, potential, 17 * 365);
    t.set_routine(Some(TrainingRoutine {
        target: goat_training::AttrTarget::Single(AttrId::ShortPassing),
        intensity: Intensity::Moderate,
    }));
    t
}

fn fixture(day: u32) -> goat_calendar::Fixture {
    goat_calendar::Fixture {
        id: day as u64,
        competition_id: 1,
        scheduled_day: day,
        original_day: day,
        is_orbit: true,
        importance: goat_calendar::FixtureImportance::League,
        leg_for_id: None,
    }
}

fn ctx_for(day: u32) -> DayContext {
    DayContext {
        epoch_day: day,
        season_id: 1,
        todays_fixtures: if day % 7 == 5 {
            vec![fixture(day)]
        } else {
            vec![]
        },
        active_windows: vec![],
        days_until_next_fixture: u32::MAX,
        congestion_score: 0,
    }
}

/// Run the scripted season directly against `on_day`, returning per-day reports
/// and the final subsystem state.
fn drive_direct() -> (Vec<DayReport>, Training) {
    let mut t = fresh_training();
    let mut reports = Vec::new();
    let mut sink = goat_rng::GoatRng::new(0); // Training never draws from this.
    for day in 0..DAYS {
        // Scripted week: rest day is %7 == 6 — EXCEPT the two-week Hard
        // overtraining stretch (days 140..=153, no rest day), which exists to
        // drive energy below the Overtrained threshold deterministically.
        let hard_stretch = (140..=153).contains(&day);
        if day % 7 == 6 && !hard_stretch {
            t.set_routine(None);
        } else {
            t.set_routine(Some(TrainingRoutine {
                target: goat_training::AttrTarget::Single(AttrId::ShortPassing),
                intensity: if hard_stretch {
                    Intensity::Hard
                } else {
                    Intensity::Moderate
                },
            }));
        }
        reports.push(t.on_day(&ctx_for(day), &mut sink));
    }
    (reports, t)
}

// ── Frozen golden values (captured 2026-08-06; FROZEN once Tùng approves) ────
// Any change to these means the training math moved — never update to make a
// failing run pass.

const GOLDEN_FINAL_ATTR_RAW: i32 = 69_184; // ShortPassing: 60.000 -> 69.184
const GOLDEN_FINAL_ENERGY_RAW: i32 = 100_000; // recovered to full by season end
const GOLDEN_FINAL_AGE_DAYS: u32 = 6_471; // 17*365 + 266
const GOLDEN_SOFT_DAYS: [u32; 14] = [
    129, 148, 149, 150, 151, 153, 154, 155, 156, 157, 158, 164, 165, 172,
];
const GOLDEN_BREAKTHROUGH_DAYS: [u32; 1] = [129];

/// Golden-seed test (training test #1, appendix Step 3): exact final attribute
/// value, exact final energy, and the exact set of soft-flashpoint days for the
/// scripted one-season sequence.
#[test]
fn golden_training_season() {
    let (reports, t) = drive_direct();

    assert_eq!(
        t.current(AttrId::ShortPassing).to_raw(),
        GOLDEN_FINAL_ATTR_RAW
    );
    assert_eq!(t.energy().to_raw(), GOLDEN_FINAL_ENERGY_RAW);
    assert_eq!(t.age_days(), GOLDEN_FINAL_AGE_DAYS);
    // An untrained attribute never moved.
    assert_eq!(
        t.current(AttrId::Finishing).to_raw(),
        50_000,
        "no cross-attribute leakage"
    );

    let soft_days: Vec<u32> = reports
        .iter()
        .enumerate()
        .filter(|(_, r)| r.stop_class == StopClass::SoftFlashpoint)
        .map(|(d, _)| d as u32)
        .collect();
    assert_eq!(soft_days, GOLDEN_SOFT_DAYS);

    let breakthrough_days: Vec<u32> = reports
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            r.payload
                .as_deref()
                .is_some_and(|p| p.starts_with("Breakthrough"))
        })
        .map(|(d, _)| d as u32)
        .collect();
    assert_eq!(breakthrough_days, GOLDEN_BREAKTHROUGH_DAYS);
    assert_eq!(
        reports[129].payload.as_deref(),
        Some("Breakthrough! Short Pass reaches 65.")
    );
    assert_eq!(
        reports[148].payload.as_deref(),
        Some("Overtrained — gains suffer at 16% energy.")
    );

    // Every non-flashpoint day is Silent (manage-by-exception, §2.2).
    for (day, r) in reports.iter().enumerate() {
        if !GOLDEN_SOFT_DAYS.contains(&(day as u32)) {
            assert_eq!(r.stop_class, StopClass::Silent, "day {day} must be silent");
            assert!(r.payload.is_none());
        }
    }
}

/// Determinism test (appendix Step 3): the same scripted season run twice must
/// produce a byte-identical snapshot — attributes, energy, and every report.
#[test]
fn determinism_byte_identical() {
    let (reports_a, ta) = drive_direct();
    let (reports_b, tb) = drive_direct();

    for a in 0..NUM_ATTRS {
        let id = AttrId::ALL[a];
        assert_eq!(ta.current(id).to_raw(), tb.current(id).to_raw());
    }
    assert_eq!(ta.energy().to_raw(), tb.energy().to_raw());
    assert_eq!(ta.age_days(), tb.age_days());

    let sig = |reports: &[DayReport]| {
        reports
            .iter()
            .map(|r| (r.stop_class, r.payload.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(sig(&reports_a), sig(&reports_b));
}

/// Engine-drive parity: the same scripted season driven through the REAL
/// `CalendarEngine` (`tick_one_day`, fixtures sorted/conflict-resolved by the
/// engine) must produce the identical report stream as the direct `on_day`
/// drive — the subsystem behaves the same inside the calendar tick as outside.
/// Both drives use the default standing routine every day (the engine owns its
/// boxed subsystem, so per-day routine edits from the golden script can't be
/// reproduced there — this test's job is parity of the tick path, not the script).
#[test]
fn engine_tick_matches_direct_drive() {
    use goat_calendar::clock::{Competition, CompetitionKind, Season};
    use goat_calendar::engine::CalendarEngine;

    let make_engine = || {
        let season = Season {
            id: 1,
            start_day: 0,
            end_day: DAYS - 1,
            windows: vec![],
            competition_ids: vec![1],
        };
        let fixtures: Vec<goat_calendar::Fixture> =
            (0..38u32).map(|w| fixture(w * 7 + 5)).collect();
        let competitions = vec![Competition {
            id: 1,
            kind: CompetitionKind::League,
            priority: 0,
            is_orbit: true,
        }];
        let mut engine = CalendarEngine::new(SEED, season, fixtures, competitions);
        engine.register(Box::new(fresh_training()));
        engine
    };

    let mut engine = make_engine();
    let mut engine_reports = Vec::new();
    for _ in 0..DAYS {
        engine_reports.push(engine.tick_one_day());
    }

    // Direct drive, default routine standing for the whole season.
    let mut t = fresh_training();
    let mut sink = goat_rng::GoatRng::new(0);
    let mut direct_reports = Vec::new();
    for day in 0..DAYS {
        direct_reports.push(t.on_day(&ctx_for(day), &mut sink));
    }

    assert_eq!(engine_reports.len(), direct_reports.len());
    for (day, (eng, dir)) in engine_reports.iter().zip(direct_reports.iter()).enumerate() {
        assert_eq!(eng.len(), 1, "one subsystem registered, day {day}");
        assert_eq!(
            eng[0].stop_class, dir.stop_class,
            "stop class differs, day {day}"
        );
        assert_eq!(eng[0].payload, dir.payload, "payload differs, day {day}");
    }
    // And the engine drive surfaces training events through the calendar (with
    // no rest days in this standing-routine script, energy collapses early and
    // the season's flashpoints are Overtrained — parity above is the point).
    assert!(engine_reports.iter().flatten().any(|r| {
        r.stop_class == StopClass::SoftFlashpoint
            && r.payload
                .as_deref()
                .is_some_and(|p| p.starts_with("Overtrained"))
    }));
}
