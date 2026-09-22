//! Staff quality model (design C — "facade đơn giản trước").
//!
//! Every staff role carries a single `quality` scalar (1–99). One shared
//! formula turns quality into the per-domain multipliers the sim consumes,
//! so club-assigned staff (quality = club strength, per the design notes)
//! and hired personal staff (quality bought with career earnings) feed the
//! exact same pipeline — the PC simply gets the better of the two.

/// Per-domain staff multipliers. `*_pct` fields are ×1000 fixed-point
/// (1000 = neutral); `setpiece_bonus` is percentage points off the difficulty
/// of set-piece contests (FreeKickAcc / Heading).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaffMods {
    /// Stamina cost of match actions (< 1000 = fitter squad).
    pub stamina_cost_pct: i32,
    /// Positive headspace gains (confidence/flow) multiplier.
    pub headspace_gain_pct: i32,
    /// Frustration gains multiplier (< 1000 = calmer player).
    pub frustration_gain_pct: i32,
    /// Injury duration multiplier (< 1000 = faster recovery).
    pub injury_duration_pct: i32,
    /// Difficulty reduction (pp) for set-piece contests.
    pub setpiece_bonus: i32,
}

impl Default for StaffMods {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

impl StaffMods {
    /// No staff influence.
    pub const NEUTRAL: StaffMods = StaffMods {
        stamina_cost_pct: 1000,
        headspace_gain_pct: 1000,
        frustration_gain_pct: 1000,
        injury_duration_pct: 1000,
        setpiece_bonus: 0,
    };

    /// Derive the full modifier set from one quality scalar (1–99).
    /// Neutral at quality 50, roughly ±8%/±16% at the extremes.
    pub fn from_quality(q: u8) -> Self {
        let q = q as i32;
        StaffMods {
            stamina_cost_pct: 1100 - 2 * q,
            headspace_gain_pct: 800 + 4 * q,
            frustration_gain_pct: 1200 - 4 * q,
            injury_duration_pct: 1200 - 4 * q,
            setpiece_bonus: q / 25,
        }
    }

    /// Domain-wise best of two staffs (club-provided vs personally hired —
    /// the design's "bù thêm / tốt hơn bản CLB" rule).
    pub fn best_of(self, other: StaffMods) -> StaffMods {
        StaffMods {
            stamina_cost_pct: self.stamina_cost_pct.min(other.stamina_cost_pct),
            headspace_gain_pct: self.headspace_gain_pct.max(other.headspace_gain_pct),
            frustration_gain_pct: self.frustration_gain_pct.min(other.frustration_gain_pct),
            injury_duration_pct: self.injury_duration_pct.min(other.injury_duration_pct),
            setpiece_bonus: self.setpiece_bonus.max(other.setpiece_bonus),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_at_quality_50() {
        let m = StaffMods::from_quality(50);
        assert_eq!(m.stamina_cost_pct, 1000);
        assert_eq!(m.headspace_gain_pct, 1000);
        assert_eq!(m.frustration_gain_pct, 1000);
        assert_eq!(m.injury_duration_pct, 1000);
    }

    #[test]
    fn better_quality_helps_everywhere() {
        let weak = StaffMods::from_quality(20);
        let strong = StaffMods::from_quality(90);
        assert!(strong.stamina_cost_pct < weak.stamina_cost_pct);
        assert!(strong.headspace_gain_pct > weak.headspace_gain_pct);
        assert!(strong.frustration_gain_pct < weak.frustration_gain_pct);
        assert!(strong.injury_duration_pct < weak.injury_duration_pct);
        assert!(strong.setpiece_bonus > weak.setpiece_bonus);
    }

    #[test]
    fn best_of_picks_domain_winners() {
        let a = StaffMods::from_quality(30);
        let b = StaffMods::from_quality(80);
        let best = a.best_of(b);
        assert_eq!(best, b);
        let best2 = b.best_of(StaffMods::NEUTRAL);
        // An 80-quality staff beats neutral everywhere except nothing.
        assert!(best2.stamina_cost_pct <= 1000);
        assert!(best2.headspace_gain_pct >= 1000);
    }
}
