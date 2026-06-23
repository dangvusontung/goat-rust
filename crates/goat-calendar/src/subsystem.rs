//! Subsystem interface, DayContext, DayReport, and related enums.
//!
//! Every game system (match, training, transfer, …) implements `Subsystem` and
//! is registered in a FIXED ORDER in the `CalendarEngine`. The registration order
//! is an ABI: reordering it breaks determinism for all existing saves.
//! It is versioned by `SIM_VERSION` in `tuning`.

use crate::clock::{Fixture, SeasonId, WindowKind};
use goat_rng::RngSource;

/// Canonical subsystem identities — ORDER IS ABI-LOCKED at SIM_VERSION 1.
///
/// Rule: NEVER reorder. To add a new subsystem, APPEND a new variant and bump
/// `SIM_VERSION`. Old saves will use the default (silent) behaviour for the new slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SubsystemId {
    Match = 0,
    Training = 1,
    Transfer = 2,
    Media = 3,
    Life = 4,
    Injury = 5,
    International = 6,
    Contract = 7,
}

/// How urgently a day's report requires the player's attention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StopClass {
    /// Day passes without surfacing anything to the player.
    Silent,
    /// Minor event worth batching; surfaces when the buffer fills or hits a cadence.
    SoftFlashpoint,
    /// Must interrupt immediately: match day, transfer offer, serious injury, call-up.
    HardStop,
}

/// Opaque state change returned by a subsystem after handling a day.
///
/// Phase 1: placeholder only. Real subsystems will add meaningful variants.
/// Applied by the engine in subsystem registration order (never re-sorted).
#[derive(Debug, Clone)]
pub enum StateMutation {
    /// No-op stand-in for Phase 1. Real mutations added per-subsystem in later phases.
    NoOp,
}

/// What a subsystem reports after handling one in-game day.
#[derive(Debug, Clone)]
pub struct DayReport {
    pub source: SubsystemId,
    pub stop_class: StopClass,
    /// Renderer-facing event text (template + slot, never free-form at runtime). None on silent days.
    pub payload: Option<String>,
    /// State changes to apply in registration order. Phase 1: all NoOp.
    pub mutations: Vec<StateMutation>,
}

impl DayReport {
    /// Convenience constructor for a silent, no-op report.
    pub fn silent(source: SubsystemId) -> Self {
        DayReport {
            source,
            stop_class: StopClass::Silent,
            payload: None,
            mutations: vec![],
        }
    }
}

/// Read-only snapshot published to every subsystem on each tick.
///
/// Fixtures are already conflict-resolved for the day (Phase 2+).
/// All values are deterministic given (seed, epoch_day, season).
#[derive(Debug)]
pub struct DayContext {
    pub epoch_day: u32,
    pub season_id: SeasonId,
    /// Orbit fixtures scheduled for today (conflict-resolved).
    pub todays_fixtures: Vec<Fixture>,
    /// Calendar windows active on this day (transfer, intl break, off-season).
    pub active_windows: Vec<WindowKind>,
    /// Days until the next orbit fixture this season. `u32::MAX` if none remain.
    pub days_until_next_fixture: u32,
    /// Count of orbit fixtures in the next 10 days — congestion indicator.
    /// (Spec names this `double`; we use `u32` — no floats in sim state.)
    pub congestion_score: u32,
}

/// Interface that every game subsystem must implement.
///
/// `on_day` is called once per tick in the fixed ABI order from `SIM_VERSION`.
/// Subsystems must draw randomness only from the `rng` argument — never from
/// global state, wall-clock, or unordered-map iteration (§9).
pub trait Subsystem {
    fn on_day(&mut self, ctx: &DayContext, rng: &mut dyn RngSource) -> DayReport;
    fn id(&self) -> SubsystemId;
}
