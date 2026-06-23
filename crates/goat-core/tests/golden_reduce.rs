//! Golden-seed test #1: fixed intent sequence → exact state values.
//!
//! Expected values must NEVER be updated to make a failing test pass.
//! If a change breaks this test, the change is wrong.

use goat_core::attrs::AttrId;
use goat_core::attrs::NUM_ATTRS;
use goat_core::player::PlayerView;
use goat_core::roles::{FamiliarityTier, NUM_ROLES};
use goat_core::state::{reduce, Intent, WorldState};
use goat_fixed::Fixed;
use goat_rng::GoatRng;

fn make_state() -> WorldState {
    let mut s = WorldState::new();
    let view = PlayerView {
        name: "Golden".into(),
        current: [Fixed::from_int(50); NUM_ATTRS],
        potential: [Fixed::from_int(75); NUM_ATTRS],
        familiarity: [FamiliarityTier::Awkward; NUM_ROLES],
        ..PlayerView::default()
    };
    let id = s.players.push(view);
    s.pc_player_id = Some(id);
    s.pc_club = "Riverside Town";
    s
}

#[test]
fn golden_noop_preserves_all_attrs() {
    let state = make_state();
    let mut rng = GoatRng::new(1);
    let out = reduce(state, Intent::NoOp, &mut rng);

    // Every attribute must remain at 50 after a no-op.
    for a in 0..NUM_ATTRS {
        assert_eq!(
            out.players.get_current(0, a),
            Fixed::from_int(50),
            "attr {a} changed after NoOp"
        );
    }
    for r in 0..NUM_ROLES {
        assert_eq!(
            out.players.get_familiarity(0, r),
            FamiliarityTier::Awkward,
            "familiarity {r} changed after NoOp"
        );
    }
}

#[test]
fn golden_delta_sequence() {
    let state = make_state();
    let mut rng = GoatRng::new(42);

    // Apply +5 to Finishing (attr index 2)
    let s1 = reduce(
        state,
        Intent::ApplyAttrDelta {
            player_id: 0,
            attr: AttrId::Finishing,
            delta: Fixed::from_int(5),
        },
        &mut rng,
    );
    assert_eq!(
        s1.players.get_current(0, AttrId::Finishing as usize),
        Fixed::from_int(55),
        "Finishing should be 55 after +5"
    );

    // Apply +20 — must clamp at potential (75)
    let s2 = reduce(
        s1,
        Intent::ApplyAttrDelta {
            player_id: 0,
            attr: AttrId::Finishing,
            delta: Fixed::from_int(20),
        },
        &mut rng,
    );
    assert_eq!(
        s2.players.get_current(0, AttrId::Finishing as usize),
        Fixed::from_int(75),
        "Finishing should be clamped at 75 (potential)"
    );

    // Other attrs must remain at 50
    assert_eq!(
        s2.players.get_current(0, AttrId::SprintSpeed as usize),
        Fixed::from_int(50),
        "unrelated attr must be unchanged"
    );
}
