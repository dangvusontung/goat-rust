//! Golden-seed generation test (appendix C.5 rewrite).
//!
//! Fixed seed + fixed choices → exact player snapshot.
//! Values were re-frozen after the C.5 generation rewrite (position-shaped potentials +
//! bounded noise). New behaviour = new goldens. Never update to fix a failing test.

use goat_core::attrs::{AttrId, NUM_ATTRS};
use goat_core::generation::{generate_player, CreationChoices, Position};
use goat_core::roles::{FamiliarityTier, RoleId};
use goat_fixed::Fixed;

fn forward_choices() -> CreationChoices {
    CreationChoices {
        name: "Golden Forward".into(),
        position: Position::Forward,
        nationality: "Brazilian",
        club: "Local FC",
    }
}

/// Seed 12345, Forward — golden values re-frozen after C.9b tuning (KEY=95, IMP=92, SEC=91).
/// FROZEN after first green run post-tuning.
#[test]
fn golden_seed_12345_forward() {
    let p = generate_player(12345, &forward_choices());

    // ── Frozen potential values (C.5 position-shaped + C.9b tuning) ──────────
    // ST Key=Fin[2]/AttPos[22]: 95% × 99 = ~94 ± noise.
    // Imp=BallControl[13]/ShotPower[4]/Composure[27]: 92% × 99 = ~91 ± noise.
    // Sec attrs (Acceleration etc.): 91% × 99 = ~90 ± noise.
    // Zero-weight attrs unchanged (~40–49).
    assert_eq!(p.potential[AttrId::Finishing as usize], Fixed::from_int(92));
    assert_eq!(
        p.potential[AttrId::AttPositioning as usize],
        Fixed::from_int(94)
    );
    assert_eq!(
        p.potential[AttrId::BallControl as usize],
        Fixed::from_int(93)
    );
    assert_eq!(p.potential[AttrId::Composure as usize], Fixed::from_int(89));
    assert_eq!(p.potential[AttrId::ShotPower as usize], Fixed::from_int(92));
    assert_eq!(
        p.potential[AttrId::Acceleration as usize],
        Fixed::from_int(90)
    );

    // ── Frozen current values ─────────────────────────────────────────────────
    // Physical (Acc=90): 90_000 × 850/1000 = 76_500
    // Technical (BCo=93): 93_000 × 700/1000 = 65_100 (unchanged)
    // Mental (Vision=44 zero-weight): 44_000 × 550/1000 = 24_200 (unchanged)
    assert_eq!(p.current[AttrId::Acceleration as usize], Fixed::raw(76_500));
    assert_eq!(p.current[AttrId::BallControl as usize], Fixed::raw(65_100));
    assert_eq!(p.current[AttrId::Vision as usize], Fixed::raw(24_200));

    // ── Frozen familiarity ────────────────────────────────────────────────────
    // primary_position = ST (Forward default)
    assert_eq!(
        p.primary_position,
        goat_core::positions::PrimaryPosition::ST
    );
    // Role familiarity seeding still works via roll_primary_role
    assert!(
        [FamiliarityTier::Natural, FamiliarityTier::Competent,]
            .contains(&p.familiarity[RoleId::CompleteForward as usize]),
        "CompleteForward should be at least Competent for a Forward"
    );
    assert_eq!(
        p.familiarity[RoleId::CentreBack as usize],
        FamiliarityTier::Awkward
    );

    // ── Structural invariants (always true — not frozen) ─────────────────────
    for i in 0..NUM_ATTRS {
        assert!(
            p.current[i] <= p.potential[i],
            "current > potential at attr {i}"
        );
        assert!(
            p.current[i] >= Fixed::MIN_ATTR,
            "current below 1 at attr {i}"
        );
        assert!(
            p.potential[i] <= Fixed::MAX_ATTR,
            "potential above 99 at attr {i}"
        );
    }
}

/// Seed 777, Defender — structural invariants + familiarity shape.
#[test]
fn golden_seed_777_defender() {
    let choices = CreationChoices {
        name: "Golden Defender".into(),
        position: Position::Defender,
        nationality: "English",
        club: "Academy United",
    };
    let p = generate_player(777, &choices);

    // primary_position must be CB for a Defender
    assert_eq!(
        p.primary_position,
        goat_core::positions::PrimaryPosition::CB
    );

    // Structural invariants
    for i in 0..NUM_ATTRS {
        assert!(
            p.current[i] <= p.potential[i],
            "current > potential at attr {i}"
        );
        assert!(p.current[i] >= Fixed::MIN_ATTR);
        assert!(p.potential[i] <= Fixed::MAX_ATTR);
    }

    // Defender roles must have exactly one Natural role
    let def_roles = [
        RoleId::CentreBack,
        RoleId::Sweeper,
        RoleId::FullBack,
        RoleId::WingBack,
    ];
    let natural_count = def_roles
        .iter()
        .filter(|&&r| p.familiarity[r as usize] == FamiliarityTier::Natural)
        .count();
    assert_eq!(
        natural_count, 1,
        "exactly one natural defender role for seed 777"
    );

    // All other defender roles must be Competent
    let competent_def = def_roles
        .iter()
        .filter(|&&r| p.familiarity[r as usize] != FamiliarityTier::Awkward)
        .count();
    assert_eq!(
        competent_def, 4,
        "all 4 defender roles should be Natural or Competent"
    );

    // Forward roles must be Awkward
    assert_eq!(
        p.familiarity[RoleId::CompleteForward as usize],
        FamiliarityTier::Awkward
    );
    assert_eq!(
        p.familiarity[RoleId::Trequartista as usize],
        FamiliarityTier::Awkward
    );

    // CB Key attrs (Marking[18], StandingTackle[17], Heading[20]) must outrank
    // zero-weight attrs (Finishing[2]) in potential — position-shape invariant.
    let avg_key = (p.potential[AttrId::Marking as usize]
        + p.potential[AttrId::StandingTackle as usize]
        + p.potential[AttrId::Heading as usize])
        .to_raw()
        / 3;
    let fin = p.potential[AttrId::Finishing as usize].to_raw();
    assert!(
        avg_key > fin,
        "CB Key attr avg ({avg_key}) must exceed Finishing ({fin}) for seed 777"
    );
}
