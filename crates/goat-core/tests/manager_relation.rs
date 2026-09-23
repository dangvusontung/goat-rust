//! PA2 M1.5 — manager trust/favor state intents.
//!
//! The values live in `WorldState`; the *derivation* (manager profile, base
//! formulas) lives in goat-world — these tests cover only storage, clamping,
//! and the agitation shock.

use goat_core::state::{reduce, Intent, WorldState};
use goat_rng::GoatRng;

#[test]
fn set_manager_relation_clamps() {
    let mut s = WorldState::new();
    assert_eq!((s.pc_manager_trust, s.pc_manager_favor), (50, 50));
    s = reduce(
        s,
        Intent::SetManagerRelation {
            trust: 140,
            favor: -20,
        },
        &mut GoatRng::new(0),
    );
    assert_eq!(s.pc_manager_trust, 100);
    assert_eq!(s.pc_manager_favor, 0);
}

#[test]
fn apply_manager_relation_accumulates_and_clamps() {
    let mut s = WorldState::new();
    s = reduce(
        s,
        Intent::ApplyManagerRelation {
            trust_delta: 7,
            favor_delta: -4,
        },
        &mut GoatRng::new(0),
    );
    assert_eq!((s.pc_manager_trust, s.pc_manager_favor), (57, 46));
    // Clamp at both ends.
    s = reduce(
        s,
        Intent::ApplyManagerRelation {
            trust_delta: 1000,
            favor_delta: -1000,
        },
        &mut GoatRng::new(0),
    );
    assert_eq!((s.pc_manager_trust, s.pc_manager_favor), (100, 0));
}

#[test]
fn agitate_for_transfer_hits_trust_and_favor() {
    let mut s = WorldState::new();
    s = reduce(s, Intent::AgitateForTransfer, &mut GoatRng::new(0));
    assert_eq!(s.pc_power_ladder, 1);
    assert_eq!(s.pc_manager_trust, 45, "agitation burns professional trust");
    assert_eq!(s.pc_manager_favor, 47, "agitation burns personal favor");
}
