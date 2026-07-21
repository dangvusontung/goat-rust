//! Player generation pipeline (bible §5.3, appendix C.5).
//!
//! `generate_player` is a pure function of `(seed, choices)` — same inputs,
//! same player, bit-for-bit, on every platform.

use goat_fixed::Fixed;
use goat_rng::{GoatRng, RngSource};

use crate::attrs::{AgeCurveArchetype, ATTR_ARCHETYPES, NUM_ATTRS};
use crate::player::PlayerView;
use crate::positions::{PrimaryPosition, POSITION_WEIGHT_TABLE};
use crate::roles::{FamiliarityTier, PositionFamily, RoleId, NUM_ROLES, ROLE_POSITION_FAMILY};
use crate::tuning::{
    CEILING_MAX, CEILING_MIN, IMP_BASE_PCT, KEY_BASE_PCT, MENTAL_START_PCT, NOISE_SALT,
    NOISE_WIDTH_PER_SPIKE, NONE_POT_ABS_LOW, NONE_POT_HIGH_PCT, PHYSICAL_START_PCT, SEC_BASE_PCT,
    TECHNICAL_START_PCT,
};

/// Choices the player makes at career-creation time (bible §4).
///
/// Nationality and club are stub lists in this phase; real world arrives in Phase 5.
/// Their effects on play land in later phases; they are recorded in state now.
///
/// `primary_position` picks one of the 8 specific outfield positions directly
/// (bible §4 revision: the player selects a specific position, not a broad family).
/// Goalkeeper is parked (bible §11).
#[derive(Debug, Clone)]
pub struct CreationChoices {
    pub name: String,
    pub primary_position: PrimaryPosition,
    pub nationality: &'static str,
    pub club: &'static str,
}

/// Hardcoded stub nationality list for phases 1–4.
pub const STUB_NATIONALITIES: &[&str] = &[
    "English",
    "Brazilian",
    "Spanish",
    "French",
    "German",
    "Italian",
    "Portuguese",
    "Argentine",
    "Dutch",
    "Senegalese",
];

/// Hardcoded stub club list for phases 1–4.
pub const STUB_CLUBS: &[&str] = &[
    "Local FC",
    "Academy United",
    "Riverside Town",
    "Hillside Athletic",
    "Valley Rangers",
];

/// Generate a player deterministically from a seed and creation choices.
///
/// Steps follow bible §5.3 + appendix C.5:
/// 1. Roll the ceiling (talent band).
/// 2. Roll spikiness (1–3) controlling noise width.
/// 3. Roll role DNA (primary role, biased by position) — kept for familiarity seeding.
/// 4. Roll per-attribute potentials shaped by position tier + bounded noise.
/// 5. Set current values at age ~16 from age-curve archetype percentages.
/// 6. Seed familiarity: natural for primary role, competent for same-family, awkward otherwise.
pub fn generate_player(seed: u64, choices: &CreationChoices) -> PlayerView {
    let mut rng = GoatRng::new(seed);

    // Step 1 — ceiling (talent band)
    let ceiling = rng.next_range_u8(CEILING_MIN, CEILING_MAX);

    // Step 2 — spikiness: 1=even, 2=varied, 3=spiky specialist (C.5)
    let spikiness = rng.next_range_u8(1, 3) as i32;

    // Step 3 — role DNA: kept to preserve familiarity seeding for the match engine
    let family = choices.primary_position.family();
    let primary_role = roll_primary_role(&mut rng, family);

    // Step 4 — per-attribute potentials (C.5: position-shaped + bounded noise)
    let primary_pos = choices.primary_position;
    let potential = roll_potentials(seed, ceiling, spikiness, primary_pos);

    // Step 5 — starting current values
    let current = derive_starting_current(&potential);

    // Step 6 — familiarity seeding
    let familiarity = seed_familiarity(family, primary_role);

    PlayerView {
        name: choices.name.clone(),
        current,
        potential,
        familiarity,
        primary_position: primary_pos,
        ..PlayerView::default()
    }
}

/// Pick the primary role, weighted toward the chosen position family.
///
/// Roles in the player's position family have triple the probability of roles
/// outside it.
fn roll_primary_role(rng: &mut impl RngSource, family: PositionFamily) -> RoleId {
    const TICKETS_IN: u32 = 3;
    const TICKETS_OUT: u32 = 1;
    let total: u32 = RoleId::ALL
        .iter()
        .map(|&r| {
            if ROLE_POSITION_FAMILY[r as usize] == family {
                TICKETS_IN
            } else {
                TICKETS_OUT
            }
        })
        .sum();

    let pick = rng.next_range_u32(0, total - 1);
    let mut cumulative: u32 = 0;
    for &role in &RoleId::ALL {
        let tickets = if ROLE_POSITION_FAMILY[role as usize] == family {
            TICKETS_IN
        } else {
            TICKETS_OUT
        };
        cumulative += tickets;
        if pick < cumulative {
            return role;
        }
    }
    RoleId::ALL[0]
}

/// Roll attribute potentials per appendix C.5.
///
/// Each attribute's potential is derived from the position tier (Key/Imp/Sec/Zero)
/// plus per-attribute centre-weighted bounded noise seeded from `hash(seed, attr)`.
/// The main RNG (`&mut impl RngSource`) is NOT used here — the noise is deterministic
/// from the player seed and attribute index, keeping the main RNG stream clean.
fn roll_potentials(
    seed: u64,
    ceiling: u8,
    spikiness: i32,
    pos: PrimaryPosition,
) -> [Fixed; NUM_ATTRS] {
    let weights = &POSITION_WEIGHT_TABLE[pos as usize];
    let noise_half = spikiness * NOISE_WIDTH_PER_SPIKE;

    let mut potential = [Fixed::ZERO; NUM_ATTRS];
    for i in 0..NUM_ATTRS {
        let w = weights[i];

        // Base potential from position tier (C.5 role-shape).
        let base: i32 = if w >= 3 {
            pct(ceiling, KEY_BASE_PCT) as i32
        } else if w == 2 {
            pct(ceiling, IMP_BASE_PCT) as i32
        } else if w == 1 {
            pct(ceiling, SEC_BASE_PCT) as i32
        } else {
            // Zero-weight: floor at NONE_POT_ABS_LOW, cap at pct(ceiling, NONE_POT_HIGH_PCT).
            let hi = (pct(ceiling, NONE_POT_HIGH_PCT) as i32).max(NONE_POT_ABS_LOW as i32);
            // Centre between floor and cap — noise not applied to zero-weight attrs.
            (NONE_POT_ABS_LOW as i32 + hi) / 2
        };

        // Bounded noise from hash(seed, attr) — C.5 deterministic deviation.
        let noise = if w > 0 {
            per_attr_noise(seed, i, noise_half)
        } else {
            0
        };

        let val = (base + noise).clamp(1, 99);
        potential[i] = Fixed::from_int(val);
    }
    potential
}

/// Centre-weighted bounded noise for one attribute: average of 2 uniform draws in
/// `[-half, half]`. Seeded deterministically from `hash(player_seed, attr_idx)`.
///
/// Using the average of 2 draws creates a triangular distribution — extreme values
/// are rarer than the edges, giving the "tight cluster" property C.5 requires.
fn per_attr_noise(player_seed: u64, attr_idx: usize, half: i32) -> i32 {
    if half == 0 {
        return 0;
    }
    // Derive a per-attr seed from the player seed.
    let attr_seed = player_seed.wrapping_add((attr_idx as u64).wrapping_mul(NOISE_SALT));
    let mut rng = GoatRng::new(attr_seed);

    let width = (half * 2 + 1) as u32; // range [0, 2*half]
    let a = rng.next_range_u32(0, width - 1) as i32 - half;
    let b = rng.next_range_u32(0, width - 1) as i32 - half;
    // Average of two draws: centre-weighted, still bounded in [-half, half].
    (a + b) / 2
}

/// Apply age-curve start percentages to compute current values at age ~16.
fn derive_starting_current(potential: &[Fixed; NUM_ATTRS]) -> [Fixed; NUM_ATTRS] {
    let mut current = [Fixed::ZERO; NUM_ATTRS];
    for i in 0..NUM_ATTRS {
        let pct_mult = match ATTR_ARCHETYPES[i] {
            AgeCurveArchetype::Physical => PHYSICAL_START_PCT,
            AgeCurveArchetype::Technical => TECHNICAL_START_PCT,
            AgeCurveArchetype::Mental => MENTAL_START_PCT,
        };
        current[i] = (potential[i] * pct_mult).clamp_attr();
    }
    current
}

/// Seed familiarity (bible §5.3 step 6 / appendix C.4):
/// - Natural: one role in the chosen position family (primary_role if in-family; else first in-family).
/// - Competent: all remaining roles in the chosen position family.
/// - Awkward: everything else.
fn seed_familiarity(family: PositionFamily, primary_role: RoleId) -> [FamiliarityTier; NUM_ROLES] {
    let natural_role = if ROLE_POSITION_FAMILY[primary_role as usize] == family {
        primary_role
    } else {
        *RoleId::ALL
            .iter()
            .find(|&&r| ROLE_POSITION_FAMILY[r as usize] == family)
            .expect("every position family has at least one role")
    };

    let mut fam = [FamiliarityTier::Awkward; NUM_ROLES];
    for &role in &RoleId::ALL {
        fam[role as usize] = if role == natural_role {
            FamiliarityTier::Natural
        } else if ROLE_POSITION_FAMILY[role as usize] == family {
            FamiliarityTier::Competent
        } else {
            FamiliarityTier::Awkward
        };
    }
    fam
}

/// Integer percentage helper: `(value * pct) / 100`, minimum 1.
fn pct(value: u8, pct: u8) -> u8 {
    ((value as u16 * pct as u16) / 100).max(1) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attrs::NUM_ATTRS;

    fn choices(pos: PrimaryPosition) -> CreationChoices {
        CreationChoices {
            name: "Test Player".into(),
            primary_position: pos,
            nationality: "English",
            club: "Local FC",
        }
    }

    #[test]
    fn current_never_exceeds_potential() {
        for seed in [1u64, 42, 999, 12345, 99999] {
            let p = generate_player(seed, &choices(PrimaryPosition::ST));
            for i in 0..NUM_ATTRS {
                assert!(
                    p.current[i] <= p.potential[i],
                    "seed {seed}: current > potential for attr {i}"
                );
            }
        }
    }

    #[test]
    fn all_attrs_in_valid_range() {
        for seed in [7u64, 100, 55555] {
            let p = generate_player(seed, &choices(PrimaryPosition::CM));
            for i in 0..NUM_ATTRS {
                assert!(
                    p.current[i] >= Fixed::MIN_ATTR,
                    "current below 1 at attr {i}"
                );
                assert!(
                    p.current[i] <= Fixed::MAX_ATTR,
                    "current above 99 at attr {i}"
                );
                assert!(
                    p.potential[i] >= Fixed::MIN_ATTR,
                    "potential below 1 at attr {i}"
                );
                assert!(
                    p.potential[i] <= Fixed::MAX_ATTR,
                    "potential above 99 at attr {i}"
                );
            }
        }
    }

    #[test]
    fn different_seeds_produce_different_players() {
        let a = generate_player(1, &choices(PrimaryPosition::ST));
        let b = generate_player(2, &choices(PrimaryPosition::ST));
        let any_diff = (0..NUM_ATTRS).any(|i| a.potential[i] != b.potential[i]);
        assert!(any_diff, "different seeds must produce different players");
    }

    #[test]
    fn chosen_position_favored_in_familiarity() {
        let p = generate_player(42, &choices(PrimaryPosition::ST));
        let has_natural_forward = [
            RoleId::InsideForward,
            RoleId::TargetForward,
            RoleId::PressingForward,
            RoleId::CompleteForward,
            RoleId::Trequartista,
        ]
        .iter()
        .any(|&r| p.familiarity[r as usize] == FamiliarityTier::Natural);
        assert!(
            has_natural_forward,
            "forward should have at least one natural forward role"
        );
    }

    #[test]
    fn same_seed_same_player() {
        let c = choices(PrimaryPosition::CB);
        let a = generate_player(777, &c);
        let b = generate_player(777, &c);
        for i in 0..NUM_ATTRS {
            assert_eq!(a.current[i], b.current[i]);
            assert_eq!(a.potential[i], b.potential[i]);
        }
        for r in 0..NUM_ROLES {
            assert_eq!(a.familiarity[r], b.familiarity[r]);
        }
    }

    #[test]
    fn forward_key_attrs_outrank_zero_weight_attrs() {
        // For a forward (ST), Finishing must have higher potential than Marking.
        use crate::attrs::AttrId;
        let p = generate_player(42, &choices(PrimaryPosition::ST));
        let fin = p.potential[AttrId::Finishing as usize];
        let mark = p.potential[AttrId::Marking as usize]; // zero weight for ST
        assert!(
            fin > mark,
            "Finishing potential ({}) must exceed Marking ({}) for a forward",
            fin.to_int(),
            mark.to_int()
        );
    }

    #[test]
    fn primary_position_set_correctly() {
        let fwd = generate_player(1, &choices(PrimaryPosition::ST));
        assert_eq!(fwd.primary_position, PrimaryPosition::ST);

        let mid = generate_player(1, &choices(PrimaryPosition::CM));
        assert_eq!(mid.primary_position, PrimaryPosition::CM);

        let def = generate_player(1, &choices(PrimaryPosition::CB));
        assert_eq!(def.primary_position, PrimaryPosition::CB);
    }

    /// Creation can now pick any of the 8 specific positions directly, not just the 3
    /// broad families the old picker exposed. A Winger (W) was never reachable through
    /// `default_primary()` (which only ever produced ST/CM/CB) — verify it shapes
    /// potentials via `POSITION_WEIGHT_TABLE[W]` and seeds Natural familiarity toward a
    /// Midfielder-family role (bible §4/§5.3).
    #[test]
    fn winger_position_reachable_directly_and_shapes_generation() {
        let p = generate_player(42, &choices(PrimaryPosition::W));
        assert_eq!(p.primary_position, PrimaryPosition::W);
        assert_eq!(p.primary_position.family(), PositionFamily::Midfielder);

        // Key attrs for W (Acceleration, SprintSpeed, CloseControl) must outrank a
        // zero-weight attr for W (e.g. Marking).
        use crate::attrs::AttrId;
        let acc = p.potential[AttrId::Acceleration as usize];
        let mark = p.potential[AttrId::Marking as usize];
        assert!(
            acc > mark,
            "Acceleration potential ({}) must exceed Marking ({}) for a winger",
            acc.to_int(),
            mark.to_int()
        );

        // Natural familiarity should land on a Midfielder-family role (W's family),
        // never a pure Forward or Defender role.
        let has_natural_midfielder = RoleId::ALL.iter().any(|&r| {
            p.familiarity[r as usize] == FamiliarityTier::Natural
                && ROLE_POSITION_FAMILY[r as usize] == PositionFamily::Midfielder
        });
        assert!(
            has_natural_midfielder,
            "winger should have a natural Midfielder-family role"
        );
    }
}
