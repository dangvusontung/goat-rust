#![forbid(unsafe_code)]
//! `goat-training` — day-granularity training subsystem on the calendar tick.
//!
//! Implements:
//! - §5.1  Age-curve archetypes: Physical / Technical / Mental grow at different rates
//! - §5.2  Late-career mental reinvention arc (Mental trainability peaks 26–33)
//! - §5.4  Training mechanics: energy-gated growth toward potential
//! - §2.2  Manage-by-exception: only flashpoints (Breakthrough, Overtrained) surface
//! - §2.4  Talent ceiling law: current can NEVER exceed potential
//! - §9    Determinism: no floats, no wall-clock, RNG domain-isolated per day
//!
//! # Rules enforced
//! - No `f32`/`f64`, no `SystemTime`, no `Instant`, no `chrono`.
//! - All randomness via the `rng` argument to `Subsystem::on_day`, domain-forked
//!   per day so training draws are isolated from calendar and match rolls (§9).
//! - `#![forbid(unsafe_code)]` in this crate.

pub mod growth;
pub mod models;
pub mod training;
pub mod tuning;

pub use models::{
    AttrTarget, DevelopmentInput, EnergyState, Intensity, TrainingEvent, TrainingRoutine,
};
pub use training::TrainingSubsystem;
