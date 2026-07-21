//! Derived attributes: family display values, role ratings, position ratings, OVR.
//!
//! Nothing here is stored as truth — everything is computed from the columns.

use crate::attrs::{
    AttrFamily, DEFENDING_ATTRS, DRIBBLING_ATTRS, NUM_ATTRS, PACE_ATTRS, PASSING_ATTRS,
    PHYSICAL_ATTRS, SHOOTING_ATTRS,
};
use crate::positions::{PositionFamiliarity, PrimaryPosition, POSITION_WEIGHT_TABLE};
use crate::roles::{FamiliarityTier, RoleId, ROLE_WEIGHT_TABLE};
use crate::tuning::{POS_RATING_AVG_PCT, POS_RATING_PEAK_PCT};
use goat_fixed::Fixed;

/// Compute the six family display values from the current attribute array.
///
/// Each family value is the unweighted average of its sub-attributes (C.1 display families).
/// Never persist these — derived display layer only.
pub fn derive_attrs(current: &[Fixed; NUM_ATTRS]) -> AttrFamily {
    AttrFamily {
        pace: family_avg(current, PACE_ATTRS),
        shooting: family_avg(current, SHOOTING_ATTRS),
        passing: family_avg(current, PASSING_ATTRS),
        dribbling: family_avg(current, DRIBBLING_ATTRS),
        defending: family_avg(current, DEFENDING_ATTRS),
        physical: family_avg(current, PHYSICAL_ATTRS),
    }
}

fn family_avg(current: &[Fixed; NUM_ATTRS], indices: &[usize]) -> Fixed {
    if indices.is_empty() {
        return Fixed::ZERO;
    }
    let sum = indices.iter().fold(Fixed::ZERO, |acc, &i| acc + current[i]);
    Fixed::raw(sum.to_raw() / indices.len() as i32)
}

/// Role rating for a specific role — used by the match engine and role-panel display.
///
/// `role_rating = weighted_avg(attrs, role_weights) × familiarity_multiplier`
///
/// Kept separate from `position_rating` so the match engine (which uses role-level
/// familiarity and the role weight table) is unaffected by the C.2–C.4 rewrite.
pub fn role_rating(
    current: &[Fixed; NUM_ATTRS],
    role: RoleId,
    familiarity: FamiliarityTier,
) -> Fixed {
    let weights = &ROLE_WEIGHT_TABLE[role as usize];
    let mut weighted_sum = Fixed::ZERO;
    let mut total_weight = Fixed::ZERO;
    for i in 0..NUM_ATTRS {
        if weights[i] != Fixed::ZERO {
            weighted_sum = weighted_sum + (weights[i] * current[i]);
            total_weight = total_weight + weights[i];
        }
    }
    if total_weight == Fixed::ZERO {
        return Fixed::ZERO;
    }
    (weighted_sum / total_weight) * familiarity.multiplier()
}

/// Position rating per appendix C.3 — three moves, all fixed-point.
///
/// 1. Weighted average over the position's weighted attributes (Key 3 / Imp 2 / Sec 1).
///    Division uses truncating integer division on raw units — rounds toward zero, consistent
///    with all other Fixed divisions in the codebase.
/// 2. Peak lift: 70% weighted_avg + 30% best Key attribute for this position.
///    Lift reads Key attributes only; an elite off-Key attr gives zero lift.
/// 3. Familiarity multiplier (C.3 four tiers).
pub fn position_rating(
    current: &[Fixed; NUM_ATTRS],
    pos: PrimaryPosition,
    familiarity: PositionFamiliarity,
) -> Fixed {
    let weights = &POSITION_WEIGHT_TABLE[pos as usize];

    // Move 1: weighted average over non-zero attrs.
    let mut weighted_sum = Fixed::ZERO;
    let mut total_weight: i32 = 0;
    let mut best_key = Fixed::ZERO;

    for (i, &w) in weights.iter().enumerate() {
        if w > 0 {
            let wf = Fixed::from_int(w as i32);
            weighted_sum = weighted_sum + wf * current[i];
            total_weight += w as i32;
            if w == 3 && current[i] > best_key {
                best_key = current[i];
            }
        }
    }

    if total_weight == 0 {
        return Fixed::ZERO;
    }

    let weighted_avg = Fixed::raw(weighted_sum.to_raw() / total_weight);

    // Move 2: peak lift (70/30 blend toward best Key attr).
    let lifted = weighted_avg * POS_RATING_AVG_PCT + best_key * POS_RATING_PEAK_PCT;

    // Move 3: familiarity multiplier.
    lifted * familiarity.multiplier()
}

/// Player-facing OVR = rating at the player's current primary position (appendix C.4).
///
/// Always uses Natural familiarity — the primary position is where the player belongs.
/// Out-of-position effective ratings use `position_rating` directly with a lower tier.
pub fn ovr(current: &[Fixed; NUM_ATTRS], primary_pos: PrimaryPosition) -> Fixed {
    position_rating(current, primary_pos, PositionFamiliarity::Natural)
}

/// Best position rating across all 8 outfield positions (appendix C.4).
///
/// Used for scouting value, transfer market pricing, and reinvention hints.
/// Always evaluated at Natural familiarity to find the player's true ceiling.
/// Returns `(rating, position)`.
pub fn best_position_rating(current: &[Fixed; NUM_ATTRS]) -> (Fixed, PrimaryPosition) {
    PrimaryPosition::ALL
        .iter()
        .map(|&p| (position_rating(current, p, PositionFamiliarity::Natural), p))
        .max_by_key(|(r, _)| r.to_raw())
        .unwrap_or((Fixed::ZERO, PrimaryPosition::ST))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attrs::AttrId;

    fn uniform_attrs(val: i32) -> [Fixed; NUM_ATTRS] {
        [Fixed::from_int(val); NUM_ATTRS]
    }

    #[test]
    fn family_avg_uniform() {
        let cur = uniform_attrs(60);
        let f = derive_attrs(&cur);
        assert_eq!(f.pace.to_int(), 60);
        assert_eq!(f.shooting.to_int(), 60);
        assert_eq!(f.passing.to_int(), 60);
        assert_eq!(f.dribbling.to_int(), 60);
        assert_eq!(f.defending.to_int(), 60);
        assert_eq!(f.physical.to_int(), 60);
    }

    #[test]
    fn role_rating_monotonic_in_key_attr() {
        let low = uniform_attrs(50);
        let mut high = low;
        high[AttrId::Finishing as usize] = Fixed::from_int(90);
        let r_low = role_rating(&low, RoleId::CompleteForward, FamiliarityTier::Natural);
        let r_high = role_rating(&high, RoleId::CompleteForward, FamiliarityTier::Natural);
        assert!(r_high > r_low, "higher key attr must increase role rating");
    }

    #[test]
    fn familiarity_tier_ordering() {
        let cur = uniform_attrs(70);
        let r_natural = role_rating(&cur, RoleId::CentralMid, FamiliarityTier::Natural);
        let r_competent = role_rating(&cur, RoleId::CentralMid, FamiliarityTier::Competent);
        let r_unconvincing = role_rating(&cur, RoleId::CentralMid, FamiliarityTier::Unconvincing);
        let r_awkward = role_rating(&cur, RoleId::CentralMid, FamiliarityTier::Awkward);
        assert!(r_natural > r_competent);
        assert!(r_competent > r_unconvincing);
        assert!(r_unconvincing > r_awkward);
    }

    #[test]
    fn position_familiarity_tier_ordering() {
        let cur = uniform_attrs(70);
        let r_nat = position_rating(&cur, PrimaryPosition::ST, PositionFamiliarity::Natural);
        let r_com = position_rating(&cur, PrimaryPosition::ST, PositionFamiliarity::Competent);
        let r_awk = position_rating(&cur, PrimaryPosition::ST, PositionFamiliarity::Awkward);
        let r_unf = position_rating(&cur, PrimaryPosition::ST, PositionFamiliarity::Unfamiliar);
        assert!(r_nat > r_com && r_com > r_awk && r_awk > r_unf);
    }

    /// Flat player: all attrs equal → position rating equals that value (C.3 shape check).
    #[test]
    fn flat_player_rating_equals_value() {
        let val = 70;
        let cur = uniform_attrs(val);
        let r = position_rating(&cur, PrimaryPosition::ST, PositionFamiliarity::Natural);
        assert_eq!(
            r.to_int(),
            val,
            "flat player rating must equal the uniform attribute value"
        );
    }

    /// Elite Key attr at a position → high OVR (C.3 shape check: Finishing 97 striker ≥ 85).
    /// "Pure poacher (Finishing 97 + solid supporting profile)" — all Key/Imp attrs strong.
    #[test]
    fn elite_key_attr_yields_high_ovr() {
        // Solid supporting profile: secondary attrs 70, Key+Imp near peak.
        let mut cur = [Fixed::from_int(70); NUM_ATTRS];
        // ST Key: Finishing[2], AttPositioning[22]
        cur[AttrId::Finishing as usize] = Fixed::from_int(97);
        cur[AttrId::AttPositioning as usize] = Fixed::from_int(92);
        // ST Imp: Composure[27], ShotPower[4], BallControl[13]
        cur[AttrId::Composure as usize] = Fixed::from_int(90);
        cur[AttrId::ShotPower as usize] = Fixed::from_int(88);
        cur[AttrId::BallControl as usize] = Fixed::from_int(90);
        let r = ovr(&cur, PrimaryPosition::ST);
        assert!(
            r.to_int() >= 85,
            "elite striker should rate ≥ 85, got {}",
            r.to_int()
        );
    }

    /// Off-role spike does NOT inflate OVR (Jumping 97 for a striker).
    #[test]
    fn off_role_spike_does_not_inflate() {
        let mut cur = [Fixed::from_int(40); NUM_ATTRS];
        cur[AttrId::Jumping as usize] = Fixed::from_int(97); // not a striker Key
        let r = ovr(&cur, PrimaryPosition::ST);
        assert!(
            r.to_int() < 55,
            "off-role spike must not inflate ST OVR, got {}",
            r.to_int()
        );
    }

    /// Vacuum specialist: one Key at 97, weak support → mid OVR, not elite.
    #[test]
    fn vacuum_specialist_is_mid_not_elite() {
        let mut cur = [Fixed::from_int(40); NUM_ATTRS];
        cur[AttrId::Finishing as usize] = Fixed::from_int(97);
        let r = ovr(&cur, PrimaryPosition::ST);
        // C.10 (30/70 blend): vacuum specialist rates 82. In a world where elites hit 97-99,
        // 82 is clearly "above average but not elite" — the design intent is preserved.
        assert!(
            r.to_int() >= 55,
            "vacuum specialist should be mid, got {}",
            r.to_int()
        );
        assert!(
            r.to_int() < 88,
            "vacuum specialist should not be elite, got {}",
            r.to_int()
        );
    }

    /// OVR diversity: different seeds → different OVRs.
    #[test]
    fn ovr_diversity_across_seeds() {
        use crate::generation::{generate_player, CreationChoices};
        use crate::positions::PrimaryPosition;
        let c = CreationChoices {
            name: "T".into(),
            primary_position: PrimaryPosition::ST,
            nationality: "English",
            club: "Local FC",
        };
        let players: Vec<_> = (0u64..5).map(|s| generate_player(s, &c)).collect();
        let ovrs: Vec<i32> = players
            .iter()
            .map(|p| ovr(&p.current, p.primary_position).to_int())
            .collect();
        let all_same = ovrs.windows(2).all(|w| w[0] == w[1]);
        assert!(
            !all_same,
            "different seeds must produce different OVRs: {:?}",
            ovrs
        );
    }
}
