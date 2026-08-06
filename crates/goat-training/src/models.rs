//! Domain models for the training subsystem (docs/TRAINING.md §Data Models).

use goat_core::attrs::{AgeCurveArchetype, AttrId};
use goat_fixed::Fixed;

/// Training intensity tiers (bible §5.4: intensity costs energy).
///
/// Phase 1 ships exactly these three illustrative tiers from the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intensity {
    Light,
    Moderate,
    Hard,
}

/// Which attribute(s) a routine pushes. Phase 1: single-attribute targets only;
/// `Family(...)` arrives in Phase 2 (spec §Data Models).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrTarget {
    Single(AttrId),
}

/// A standing instruction: which attribute to push, at what intensity
/// (US-01). Persists across days until changed — the calendar auto-runs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrainingRoutine {
    pub target: AttrTarget,
    pub intensity: Intensity,
}

impl Default for TrainingRoutine {
    /// The spec (§Assumptions) requires a default routine so a never-intervening
    /// player still develops. Placeholder pick, TUNABLE: short passing at
    /// moderate intensity — a safe technical attribute that every position uses.
    fn default() -> Self {
        TrainingRoutine {
            target: AttrTarget::Single(AttrId::ShortPassing),
            intensity: Intensity::Moderate,
        }
    }
}

/// Per-player energy, 0..=100 fixed-point (bible §5.4: tired players gain less
/// and injure more; rest recovers energy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnergyState {
    pub value: Fixed,
}

impl Default for EnergyState {
    fn default() -> Self {
        EnergyState {
            value: crate::tuning::ENERGY_MAX,
        }
    }
}

/// Assembled per training day and fed to the growth function (spec §Data
/// Models). The seeded jitter stream is passed separately (a `&mut GoatRng`
/// forked once per subsystem lifetime), not stored here.
#[derive(Debug, Clone, Copy)]
pub struct DevelopmentInput {
    /// From `goat_core::attrs::ATTR_ARCHETYPES` for the routine's target attr.
    pub attr_archetype: AgeCurveArchetype,
    pub age_days: u32,
    pub current: Fixed,
    pub potential: Fixed,
    pub intensity: Intensity,
    pub energy: Fixed,
    /// Development-speed multiplier from the club model (bible §4.2). Phase 1
    /// default: 1.0 — the club model itself is out of scope.
    pub facility_mult: Fixed,
}

/// What a training day produced — the crate-internal report; the `Subsystem`
/// impl maps it onto the calendar's `DayReport` (spec §Data Models note).
#[derive(Debug, Clone)]
pub struct TrainingDayResult {
    /// Growth applied this day (one entry per trained attribute — exactly one
    /// in Phase 1's single-target routines).
    pub attr_deltas: Vec<(AttrId, Fixed)>,
    pub energy_delta: Fixed,
    /// `None` on a routine day (manage-by-exception, §2.2).
    pub event: Option<TrainingEvent>,
}

/// Events worth surfacing as a soft flashpoint (US-05). Phase 1 detects
/// `Breakthrough` and `Overtrained` only; `FormDip` and `ReturnFromInjury` are
/// spec-listed for completeness and arrive with their subsystems (Phase 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrainingEvent {
    /// The trained attribute crossed a notable threshold.
    Breakthrough,
    /// Trained while energy was critically low.
    Overtrained,
    /// Phase 2 — needs the form model.
    FormDip,
    /// Phase 2 — needs the injury subsystem.
    ReturnFromInjury,
}
