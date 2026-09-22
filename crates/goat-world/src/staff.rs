//! Club-assigned staff (design C, world side).
//!
//! Six roles come with the club; the PC cannot choose them. Their quality is
//! NOT rolled per role — it derives from club stature (`club_strength`, the
//! `facilities_mult` lineage: strong clubs hire strong staff), per the design
//! notes' "facade đơn giản trước". All roles share one quality number; the
//! per-domain effects are produced by `goat_core::staff::StaffMods`.

use goat_core::staff::StaffMods;

/// The six club-assigned staff roles (design C nhóm 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClubStaffRole {
    /// Tactics — already encoded in the club's TacticalProfile.
    HeadCoach,
    /// Stamina / match-load management.
    FitnessCoach,
    /// Dressing-room headspace (confidence / frustration).
    Psychologist,
    /// Injury recovery speed.
    Physio,
    /// Set pieces (FreeKickAcc / Heading contests).
    SetPieceCoach,
    /// Parked until goalkeepers are playable.
    GkCoach,
}

/// Quality of a club staff role: the club's strength, unrolled (design C).
pub fn role_quality(club_strength: u8, _role: ClubStaffRole) -> u8 {
    club_strength
}

/// The full match/week effect bundle of a club's staff.
pub fn club_staff_mods(club_strength: u8) -> StaffMods {
    StaffMods::from_quality(club_strength)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strong_clubs_have_better_staff() {
        let minnow = club_staff_mods(40);
        let elite = club_staff_mods(90);
        assert!(elite.stamina_cost_pct < minnow.stamina_cost_pct);
        assert!(elite.injury_duration_pct < minnow.injury_duration_pct);
    }

    #[test]
    fn all_roles_share_club_quality() {
        for role in [
            ClubStaffRole::HeadCoach,
            ClubStaffRole::FitnessCoach,
            ClubStaffRole::Psychologist,
            ClubStaffRole::Physio,
            ClubStaffRole::SetPieceCoach,
            ClubStaffRole::GkCoach,
        ] {
            assert_eq!(role_quality(77, role), 77);
        }
    }
}
