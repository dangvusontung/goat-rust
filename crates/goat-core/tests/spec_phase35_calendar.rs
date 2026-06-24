//! Phase 3.5 SPEC — the CalendarEngine is live in the week loop (Option A, golden-safe).
//!
//! Proves two things through the real `reduce(AdvanceWeek)` path:
//!   1. Calendar window flashpoints surface in `WorldState` as weeks advance.
//!   2. The calendar tick does NOT perturb attribute growth — `golden_week`'s frozen
//!      values still hold (asserted there); here we add a direct determinism check.
//!
//! The engine runs on its own RNG stream (seeded from `world_seed`), independent of the
//! growth RNG — that independence is what makes wiring it in golden-safe.

use goat_calendar::WindowKind;
use goat_core::calendar_loop::SEASON_DAYS;
use goat_core::generation::{CreationChoices, Position};
use goat_core::state::{reduce, Intent, WorldState};
use goat_core::week::{Intensity, Routine};
use goat_rng::GoatRng;

fn forward(seed: u64) -> WorldState {
    let choices = CreationChoices {
        name: "Cal".into(),
        position: Position::Forward,
        nationality: "Brazilian",
        club: "Riverside Town",
    };
    let mut s = WorldState::new();
    s = reduce(
        s,
        Intent::CreatePlayer { seed, choices },
        &mut GoatRng::new(0),
    );
    let routine = Routine {
        focus_attrs: vec![goat_core::attrs::AttrId::Finishing],
        intensity: Intensity::High,
    };
    reduce(s, Intent::SetRoutine { routine }, &mut GoatRng::new(0))
}

#[test]
fn calendar_flashpoints_surface_over_a_season() {
    let mut s = forward(7);
    let mut seen: Vec<WindowKind> = Vec::new();
    // 53 weeks ≈ one 365-day season — each window should open once.
    for _ in 0..53 {
        s = reduce(s, Intent::AdvanceWeek, &mut GoatRng::new(1));
        for f in &s.last_week_flashpoints {
            seen.push(f.window);
        }
    }
    assert!(
        seen.contains(&WindowKind::InternationalBreak)
            && seen.contains(&WindowKind::TransferWinter)
            && seen.contains(&WindowKind::TransferSummer),
        "all three calendar windows should fire across a season; saw {seen:?}"
    );
}

#[test]
fn epoch_day_advances_seven_per_week() {
    let mut s = forward(3);
    for wk in 1..=10u32 {
        s = reduce(s, Intent::AdvanceWeek, &mut GoatRng::new(wk as u64));
        assert_eq!(s.pc_epoch_day, wk * 7, "epoch day should be 7×week");
    }
    assert!(s.pc_epoch_day < SEASON_DAYS); // 70 < 365
}

/// Golden-safety: the calendar tick must not change attribute growth. Two runs with the
/// SAME growth RNG but DIFFERENT world_seed (hence different calendar streams) must yield
/// byte-identical attributes — proving the calendar stream can't leak into growth.
#[test]
fn calendar_seed_does_not_affect_growth() {
    let attrs_after = |world_seed: u64| {
        let mut s = forward(9);
        s.world_seed = world_seed; // change ONLY the calendar's stream source
        for _ in 0..40 {
            s = reduce(s, Intent::AdvanceWeek, &mut GoatRng::new(123));
        }
        let pc = s.pc_player_id.unwrap();
        (0..goat_core::attrs::NUM_ATTRS)
            .map(|a| s.players.get_current(pc, a))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        attrs_after(1),
        attrs_after(999_999),
        "world_seed (calendar stream) must not influence attribute growth"
    );
}
