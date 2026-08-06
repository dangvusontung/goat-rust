#![forbid(unsafe_code)]

//! `goat-training` — the training subsystem (docs/TRAINING.md, Phase 1).
//!
//! The first content-bearing subsystem on the calendar's day-tick: pushes the
//! orbit player's `current → potential` (bible §5.4), gated by age-curve
//! archetypes (§5.1) and paid for in energy (§5.4). Phase 1 scope: a
//! single-attribute routine, energy spend/recovery, deterministic growth,
//! Breakthrough/Overtrained soft flashpoints. Family targets, facility
//! multipliers from the club model, injury coupling, and form dips are
//! Phase 2 (out of scope).
//!
//! Integration notes (Phase 1 realities, kept explicit):
//! - RNG: the subsystem owns a stream forked ONCE at construction via
//!   `goat_calendar::rng_stream::RngStream::fork("training")` and never draws
//!   from the `rng` passed into `Subsystem::on_day` — that argument is the
//!   calendar's own stream today (goat-calendar's engine calls
//!   `sys.on_day(&ctx, &mut self.calendar_rng)`), and the spec's hard rule is
//!   "never the calendar or match stream". The internal fork satisfies the
//!   rule without touching goat-calendar.
//! - `DayContext` exposes no player access and `StateMutation` is a Phase-1
//!   placeholder (`NoOp` only), so this crate keeps its Phase-1 player state
//!   (attributes + energy) internally. Wiring into `WorldState` is a later
//!   round — the same "logic first, wire later" shape as promotion.rs/A3.3.

pub mod growth;
pub mod models;
pub mod tuning;

pub use growth::{compute_growth, recover_energy, spend_energy, trainability};
pub use models::{
    AttrTarget, DevelopmentInput, EnergyState, Intensity, TrainingDayResult, TrainingEvent,
    TrainingRoutine,
};
