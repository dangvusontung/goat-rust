//! Team tactical identity — a third instance of the "shared substrate + reweighting lens"
//! pattern already used twice in this codebase (`derive::role_rating` reweighting the
//! 30-attribute substrate per role; `pantheon::School::score` reweighting the 8-axis
//! `LegacyAxes` substrate per school). A team's tactical identity reweights the player's
//! *existing*, already-trained per-role `role_rating` — nothing new trains here. A player
//! naturally reads as "Natural at club, Awkward for country" purely because the same
//! trained familiarity is read through two different `TacticalIdentity` lenses (Design
//! round 2, Doc B §B.1).

use crate::attrs::NUM_ATTRS;
use crate::derive::role_rating;
use crate::roles::{FamiliarityTier, RoleId, NUM_ROLES};
use crate::tuning::{TEAM_FIT_COMPETENT_PCT, TEAM_FIT_NATURAL_PCT, TEAM_FIT_UNCONVINCING_PCT};
use goat_fixed::Fixed;
use goat_rng::{GoatRng, RngSource};

/// A team's (club's or national team's) style bias over the 14 outfield roles — how much
/// that team's system rewards each role, independent of any specific player. Seed-derived
/// at genesis, analogous to `Club::strength` — a static per-team number, not a
/// trained/mutable one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TacticalIdentity {
    /// Sums to `NUM_ROLES * Fixed::ONE` (average weight exactly 1.0), mirroring
    /// `ROLE_WEIGHT_TABLE`'s "shape, not scale" convention.
    pub role_weight: [Fixed; NUM_ROLES],
}

impl TacticalIdentity {
    /// Generate a tactical identity deterministically from a seed (e.g.
    /// `world_seed ^ club_seed(id)` for a club, `world_seed ^ nation_seed(id)` for a
    /// national team — same "generated but consistent" pattern as every other per-team
    /// generated value in `goat-world`).
    pub fn generate(seed: u64) -> Self {
        let mut rng = GoatRng::new(seed);
        let mut raw = [0i64; NUM_ROLES];
        let mut sum: i64 = 0;
        for r in raw.iter_mut() {
            // Weight in [0.400, 1.600] — wide enough that a team can meaningfully favor
            // or disfavor a role, narrow enough that no role is ever fully zeroed out
            // (every role stays reachable, just more or less rewarded).
            let w = rng.next_range_u32(400, 1600) as i64;
            *r = w;
            sum += w;
        }
        let target = NUM_ROLES as i64 * 1000;
        let mut role_weight = [Fixed::ZERO; NUM_ROLES];
        for (i, &w) in raw.iter().enumerate() {
            role_weight[i] = Fixed::raw(((w * target) / sum) as i32);
        }
        Self { role_weight }
    }
}

/// A player's fit for a specific team's tactical identity, in the *same* 4-tier
/// vocabulary as role familiarity (Natural/Competent/Unconvincing/Awkward) — deliberately
/// reusing `FamiliarityTier` rather than inventing a parallel tier enum, so the existing
/// bench-likelihood/selection logic reads one vocabulary, not two.
pub fn team_fit(
    current: &[Fixed; NUM_ATTRS],
    familiarity: &[FamiliarityTier; NUM_ROLES],
    identity: &TacticalIdentity,
) -> FamiliarityTier {
    // Best-role-under-this-lens score, mirroring derive::ovr's "best role rating" pattern
    // but weighted by `identity.role_weight` instead of a neutral pick.
    let mut best = Fixed::ZERO;
    for &r in &RoleId::ALL {
        let score =
            role_rating(current, r, familiarity[r as usize]) * identity.role_weight[r as usize];
        if score > best {
            best = score;
        }
    }
    bucket_into_tier(best)
}

fn bucket_into_tier(best: Fixed) -> FamiliarityTier {
    if best >= TEAM_FIT_NATURAL_PCT {
        FamiliarityTier::Natural
    } else if best >= TEAM_FIT_COMPETENT_PCT {
        FamiliarityTier::Competent
    } else if best >= TEAM_FIT_UNCONVINCING_PCT {
        FamiliarityTier::Unconvincing
    } else {
        FamiliarityTier::Awkward
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attrs::AttrId;

    fn uniform_attrs(val: i32) -> [Fixed; NUM_ATTRS] {
        [Fixed::from_int(val); NUM_ATTRS]
    }

    #[test]
    fn identity_deterministic_per_seed() {
        assert_eq!(TacticalIdentity::generate(7), TacticalIdentity::generate(7));
    }

    #[test]
    fn identity_differs_across_seeds() {
        assert_ne!(TacticalIdentity::generate(1), TacticalIdentity::generate(2));
    }

    #[test]
    fn identity_weights_sum_to_average_one() {
        let id = TacticalIdentity::generate(42);
        let sum: i32 = id.role_weight.iter().map(|w| w.to_raw()).sum();
        // Allow small integer-rounding slack from the normalization division.
        assert!(
            (sum - NUM_ROLES as i32 * 1000).abs() <= NUM_ROLES as i32,
            "role weights should sum to ~{}, got {sum}",
            NUM_ROLES * 1000
        );
    }

    #[test]
    fn natural_and_awkward_fit_are_both_reachable() {
        // A team that lopsidedly favors CentreBack (near the 1.600 ceiling) vs one that
        // disfavors it (near the 0.400 floor) — same elite CentreBack player, different
        // team lens should be able to swing Natural vs Awkward at the extremes.
        let mut favor = TacticalIdentity::generate(1);
        let mut disfavor = TacticalIdentity::generate(1);
        favor.role_weight[RoleId::CentreBack as usize] = Fixed::raw(1_600);
        disfavor.role_weight[RoleId::CentreBack as usize] = Fixed::raw(400);
        // Zero out every other role so `team_fit`'s max-across-roles can only read
        // CentreBack — isolates the lens effect from "some other role saves the score".
        for r in 0..NUM_ROLES {
            if r != RoleId::CentreBack as usize {
                favor.role_weight[r] = Fixed::ZERO;
                disfavor.role_weight[r] = Fixed::ZERO;
            }
        }
        let mut cur = uniform_attrs(50);
        cur[AttrId::StandingTackle as usize] = Fixed::from_int(95);
        cur[AttrId::Marking as usize] = Fixed::from_int(95);
        cur[AttrId::Interceptions as usize] = Fixed::from_int(95);
        cur[AttrId::Heading as usize] = Fixed::from_int(95);
        cur[AttrId::Strength as usize] = Fixed::from_int(95);
        cur[AttrId::Composure as usize] = Fixed::from_int(95);
        let fam = [FamiliarityTier::Natural; NUM_ROLES];

        let fit_favor = team_fit(&cur, &fam, &favor);
        let fit_disfavor = team_fit(&cur, &fam, &disfavor);
        assert!(
            fit_favor > fit_disfavor,
            "the favoring lens must rate this player's fit higher: {fit_favor:?} vs {fit_disfavor:?}"
        );
        assert_eq!(fit_favor, FamiliarityTier::Natural);
        assert_eq!(fit_disfavor, FamiliarityTier::Awkward);
    }
}
