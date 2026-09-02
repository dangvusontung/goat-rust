//! Golden-seed generation test (appendix C.5 rewrite).
//!
//! Fixed seed + fixed choices → exact player snapshot.
//! Values were re-frozen after the C.5 generation rewrite (position-shaped potentials +
//! bounded noise). New behaviour = new goldens. Never update to fix a failing test.

use goat_core::attrs::{AttrId, NUM_ATTRS};
use goat_core::generation::{generate_player, CreationChoices};
use goat_core::positions::PrimaryPosition;
use goat_core::roles::{FamiliarityTier, RoleId};
use goat_fixed::Fixed;

fn forward_choices() -> CreationChoices {
    CreationChoices {
        name: "Golden Forward".into(),
        primary_position: PrimaryPosition::ST,
        nationality: "Brazilian".to_string(),
        club: "Local FC".to_string(),
    }
}

/// Seed 12345, Forward.
///
/// RE-FROZEN (intentional behaviour change): the talent lottery was restored —
/// `CEILING_MIN`/`CEILING_MAX` went 99/99 → 70/99, so this seed now rolls ceiling **79**
/// instead of 99. Same stream order, same formulas; only the ceiling input changed.
/// This is exactly the class of change goldens exist to catch — updated deliberately,
/// per the header rule ("new behaviour = new goldens"), NOT to silence a failure.
#[test]
fn golden_seed_12345_forward() {
    let p = generate_player(12345, &forward_choices());

    // ── Frozen potential values (ceiling 79, C.5 position-shaped + noise) ─────
    // ST Key=Fin[2]/AttPos[22]: 95% × 79 = ~75 ± noise.
    // Imp=BallControl[13]/ShotPower[4]/Composure[27]: 92% × 79 = ~73 ± noise.
    // Sec attrs (Acceleration etc.): 91% × 79 = ~72 ± noise.
    // Zero-weight attrs unchanged (~40–49).
    assert_eq!(p.potential[AttrId::Finishing as usize], Fixed::from_int(73));
    assert_eq!(
        p.potential[AttrId::AttPositioning as usize],
        Fixed::from_int(75)
    );
    assert_eq!(
        p.potential[AttrId::BallControl as usize],
        Fixed::from_int(74)
    );
    assert_eq!(p.potential[AttrId::Composure as usize], Fixed::from_int(70));
    assert_eq!(p.potential[AttrId::ShotPower as usize], Fixed::from_int(73));
    assert_eq!(
        p.potential[AttrId::Acceleration as usize],
        Fixed::from_int(71)
    );

    // ── Frozen current values ─────────────────────────────────────────────────
    // Physical (Acc=71): 71_000 × 850/1000 = 60_350
    // Technical (BCo=74): 74_000 × 700/1000 = 51_800
    // Mental (Vision=40 zero-weight): 40_000 × 550/1000 = 22_000
    assert_eq!(p.current[AttrId::Acceleration as usize], Fixed::raw(60_350));
    assert_eq!(p.current[AttrId::BallControl as usize], Fixed::raw(51_800));
    assert_eq!(p.current[AttrId::Vision as usize], Fixed::raw(22_000));

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
        primary_position: PrimaryPosition::CB,
        nationality: "English".to_string(),
        club: "Academy United".to_string(),
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
