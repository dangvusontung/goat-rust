//! Golden-seed week-loop tests.
//!
//! Fixed seed + fixed routine + N weeks → exact attribute/energy state.
//! Values frozen from first green run. NEVER update them to fix a failing test.

use goat_core::attrs::{AttrId, NUM_ATTRS};
use goat_core::generation::CreationChoices;
use goat_core::positions::PrimaryPosition;
use goat_core::state::{reduce, Intent, WorldState};
use goat_core::week::{Intensity, Routine};
use goat_fixed::Fixed;
use goat_rng::GoatRng;

fn forward_state() -> WorldState {
    let choices = CreationChoices {
        name: "Golden Fwd".into(),
        primary_position: PrimaryPosition::ST,
        primary_role: None,
        nationality: "Brazilian".to_string(),
        club: "Riverside Town".to_string(),
    };
    let s = WorldState::new();
    reduce(
        s,
        Intent::CreatePlayer {
            seed: 12345,
            choices,
        },
        &mut GoatRng::new(0),
    )
}

/// Advance exactly 52 weeks (1 season) with a fixed routine and seed,
/// then assert exact attribute values.
///
/// These values are frozen from the first green run.
#[test]
fn golden_52_weeks_forward() {
    let s = forward_state();

    let routine = Routine {
        focus_attrs: vec![
            AttrId::CloseControl,
            AttrId::BallControl,
            AttrId::Vision,
            AttrId::Finishing,
        ],
        intensity: Intensity::Medium,
    };
    let s = reduce(s, Intent::SetRoutine { routine }, &mut GoatRng::new(0));

    let s = reduce(s, Intent::AdvanceWeeks { n: 52 }, &mut GoatRng::new(999));

    // ── Frozen values (re-frozen after BL7 durability/injury-proneness trait, Design
    // round 9): seed 12345's rolled `durability_x10` isn't the neutral midpoint, so
    // this seed's injury-week timing over the 52-week horizon shifted, changing how
    // many weeks actually trained. Expected — this coefficient is designed to add
    // per-seed injury variance (see `week.rs::durability_neutral_value_reproduces_
    // pre_existing_injury_numbers` for proof the *formula* itself is unchanged at
    // neutral durability).
    let dri = s.players.get_current(0, AttrId::CloseControl as usize);
    assert_eq!(
        dri,
        Fixed::raw(63_173),
        "CloseControl frozen at 63.173 after 52 wks"
    );

    let vis = s.players.get_current(0, AttrId::Vision as usize);
    assert_eq!(
        vis,
        Fixed::raw(24_292),
        "Vision frozen at 24.292 after 52 wks"
    );

    // Energy must still be in valid range.
    let energy = s.players.get_energy(0);
    assert!(energy >= Fixed::ZERO);
    assert!(energy <= Fixed::from_int(100));

    // No attr may exceed its potential.
    for a in 0..NUM_ATTRS {
        assert!(
            s.players.get_current(0, a) <= s.players.get_potential(0, a),
            "current > potential after 52 wks for attr {a}"
        );
    }
}

/// Temp dump: capture 52-week values after ceiling change.
#[test]
fn dump_52_weeks_new_values() {
    let s = forward_state();
    let routine = Routine {
        focus_attrs: vec![
            AttrId::CloseControl,
            AttrId::BallControl,
            AttrId::Vision,
            AttrId::Finishing,
        ],
        intensity: Intensity::Medium,
    };
    let s = reduce(s, Intent::SetRoutine { routine }, &mut GoatRng::new(0));
    let s = reduce(s, Intent::AdvanceWeeks { n: 52 }, &mut GoatRng::new(999));
    let dri = s.players.get_current(0, AttrId::CloseControl as usize);
    let vis = s.players.get_current(0, AttrId::Vision as usize);
    eprintln!("Dribbling raw={} val={:?}", dri.to_raw(), dri);
    eprintln!("Vision    raw={} val={:?}", vis.to_raw(), vis);
}

/// Long-horizon sanity: simulate a full 16→38 year career headless.
/// No panics, invariants hold, career arc is visible (physical declines, mental rises).
#[test]
fn long_horizon_full_career() {
    let s = forward_state();

    // Forward routine focusing on technical and mental attrs.
    let routine = Routine {
        focus_attrs: vec![
            AttrId::Finishing,
            AttrId::Composure,
            AttrId::Vision,
            AttrId::BallControl,
        ],
        intensity: Intensity::Medium,
    };
    let mut s = reduce(s, Intent::SetRoutine { routine }, &mut GoatRng::new(0));
    let mut rng = GoatRng::new(77777);

    // Run 22 seasons (16 → 38 years) = 1 144 weeks.
    // AdvanceWeek is gated to one session per calendar week
    // (TASK-CORE-double-week-tick); this harness has no round loop, so it
    // simulates each week boundary itself by clearing the flag. The per-tick
    // math is untouched — every tick below runs exactly as before the gate.
    for _ in 0..1_144 {
        s = reduce(s, Intent::AdvanceWeek, &mut rng);
        s.pc_week_training_done = false;
    }

    // ── Invariants (must hold at all ages) ───────────────────────────────────
    for a in 0..NUM_ATTRS {
        let cur = s.players.get_current(0, a);
        let pot = s.players.get_potential(0, a);
        assert!(cur >= Fixed::MIN_ATTR, "attr {a} below 1 at age 38");
        assert!(cur <= pot, "current > potential for attr {a} at age 38");
    }

    let energy = s.players.get_energy(0);
    assert!(energy >= Fixed::ZERO, "energy below 0 at age 38");
    assert!(energy <= Fixed::from_int(100), "energy above 100 at age 38");

    // ── Career arc: physical declined, mental still high ─────────────────────
    // Acceleration (physical): should have declined significantly by 38.
    // C.9b tuning: SEC_BASE_PCT=91 raises Acc ceiling to 90 (was 87), start=76.5,
    // total decay ≈ 22.88 → floor ≈ 53.6. Threshold updated from 55 → 58.
    let acc = s.players.get_current(0, AttrId::Acceleration as usize);
    assert!(
        acc <= Fixed::from_int(58),
        "Acceleration should have declined by 38 (got {acc:?})"
    );

    // Composure (mental, focused): should be close to potential by 38.
    let composure = s.players.get_current(0, AttrId::Composure as usize);
    let composure_pot = s.players.get_potential(0, AttrId::Composure as usize);
    let gap = composure_pot - composure;
    assert!(
        gap <= Fixed::from_int(10),
        "Composure should be near potential at 38 (gap={gap:?})"
    );

    // Age must be 38 years.
    let age_weeks = s.players.get_age_weeks(0);
    assert_eq!(age_weeks, 16 * 52 + 1_144, "age must be exactly 38y at end");
}
