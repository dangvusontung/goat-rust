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
///
/// Design round 4, Slice 5 §5.3 re-check: this was tuned back when the league was the
/// only orbit competition, so 3 buffered soft events implied several quiet weeks. Now
/// that League + Domestic Cup + a continental tier (+ a national-team call-up) can all
/// be live at once, 3 events can arrive from one busy multi-competition week instead —
/// see `spec_round4_slice5_congestion_sanity.rs`. Confirmed still mechanically correct;
/// the FEEL question is unresolved and belongs to a real playtesting pass, not here.
pub const SOFT_FLUSH_THRESHOLD: usize = 3;

/// Minimum days between two of the PC's orbit fixtures — conflict-resolution reschedules
/// (`resolveFixturesForDay`, `docs/MAIN.md:1090-1117`) never place a bumped fixture closer
/// than this to another already-scheduled orbit fixture.
///
/// TUNABLE — no bible-specified value; a real fixture-congestion feel pass belongs to a
/// later round. 3 days is a conservative floor (never a next-day replay).
pub const MIN_REST_GAP_DAYS: u32 = 3;
