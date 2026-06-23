//! Tunable constants for the calendar system.
//!
//! All numbers here are named so they can be adjusted without touching logic.
//! Bible numbers are illustrative starting values; real tuning happens against a prototype.

/// Subsystem registration ABI version.
/// NEVER reorder the subsystem list — that breaks save determinism.
/// When a new subsystem must be added, APPEND it and bump SIM_VERSION.
pub const SIM_VERSION: u32 = 1;

/// Soft-flashpoint buffer: flush to the renderer when this many soft events accumulate.
///
/// TUNABLE — the real policy (count-threshold, weight-threshold, or fixed Monday cadence)
/// is a design open question from the spec (§2.2). This constant is the Phase 1 stand-in.
/// Do not finalise the rule here; iterate against a prototype.
pub const SOFT_FLUSH_THRESHOLD: usize = 3;
