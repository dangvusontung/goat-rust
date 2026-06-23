//! Domain types for the training subsystem.
//!
//! These are separate from goat-core's `Routine`/`Intensity` — training operates
//! per-day via the calendar tick, not per-week via `advance_week`.

use goat_core::attrs::{AgeCurveArchetype, AttrId};
use goat_fixed::Fixed;

/// Training intensity. Different names from goat-core's per-week tiers:
/// Light/Moderate/Hard map to the bible's §5.4 energy/growth trade-off.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intensity {
    /// Net energy gain; gentle growth. Sustainable indefinitely.
    Light,
    /// Energy neutral; standard growth rate.
    #[default]
    Moderate,
    /// Energy drain; accelerated growth but overtraining risk.
    Hard,
}

/// Which attribute the routine targets.
///
/// Phase 1: `Single` only. `Family` (train a whole family) is Phase 2.
#[derive(Debug, Clone, Copy)]
pub enum AttrTarget {
    Single(AttrId),
    // Family(FamilyId) — Phase 2
}

/// Standing training instruction: target + intensity. Persists across days until changed.
///
/// A default routine exists so a non-intervening player still develops (US-01).
/// `target = None` signals a pure rest day — energy recovery, no growth.
#[derive(Debug, Clone)]
pub struct TrainingRoutine {
    /// Attribute to push. `None` = rest day (energy recovery, no growth).
    pub target: Option<AttrTarget>,
    pub intensity: Intensity,
}

impl TrainingRoutine {
    /// Returns true when no attribute is actively being trained (rest).
    pub fn is_rest(&self) -> bool {
        self.target.is_none()
    }

    /// Convenience: a pure rest day routine.
    pub fn rest() -> Self {
        Self {
            target: None,
            intensity: Intensity::Light,
        }
    }
}

impl Default for TrainingRoutine {
    /// Default: Finishing at Moderate intensity. Non-intervening player still develops (US-01).
    fn default() -> Self {
        Self {
            target: Some(AttrTarget::Single(AttrId::Finishing)),
            intensity: Intensity::Moderate,
        }
    }
}

/// Per-player energy snapshot used in the daily computation. 0–100 fixed-point.
#[derive(Debug, Clone, Copy)]
pub struct EnergyState {
    pub value: Fixed,
}

/// Input bundle assembled for one training day and passed to `compute_growth`.
pub struct DevelopmentInput {
    /// Age-curve archetype of the attribute being trained (bible §5.1).
    pub attr_archetype: AgeCurveArchetype,
    /// Player age in in-game days (derived from `age_weeks * 7`).
    /// Spec uses u16; u32 used here — a player at 50 yrs = 18 250 days (fits u16 too).
    pub age_days: u32,
    /// Current attribute value. Growth is bounded above by `potential`.
    pub current: Fixed,
    /// Potential ceiling — growth can NEVER push current past this (§2.4).
    pub potential: Fixed,
    pub intensity: Intensity,
    pub energy: Fixed,
    /// Facility / coach multiplier. Default 1.0 (club model is out of scope Phase 1).
    pub facility_mult: Fixed,
}

/// Notable events that the training subsystem raises as SoftFlashpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrainingEvent {
    /// Attribute crossed a notable milestone threshold — positive news.
    Breakthrough,
    /// Player trained at Hard intensity while critically low on energy — warning.
    Overtrained,
    // FormDip, ReturnFromInjury — Phase 2
}
