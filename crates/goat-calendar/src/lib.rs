#![forbid(unsafe_code)]
//! `goat-calendar` — time-orchestration spine for GOAT.
//!
//! Phase 1: single-competition, deterministic tick loop.
//! Implements §2.2 (manage-by-exception) and §2.3 (seed-determinism).
//!
//! # Non-negotiable rules enforced in this crate
//! - No `DateTime::now()`, `SystemTime`, `Instant`, `chrono`, or wall-clock reads.
//! - No `f32` / `f64` anywhere in simulation state or logic.
//! - All randomness flows through the injected `calendar_rng` (domain-forked from root).
//! - Subsystem poll order is a fixed `Vec` — never a `HashMap`/`HashSet` iteration.
//! - `epoch_day` is mutated in exactly one place: `CalendarEngine::tick_one_day`.

pub mod clock;
pub mod engine;
pub mod rng_stream;
pub mod subsystem;
pub mod tuning;

pub use clock::{
    CalendarWindow, Competition, CompetitionId, CompetitionKind, Fixture, FixtureId,
    FixtureImportance, GameClock, Season, SeasonId, SuspensionLedger, WindowKind,
};
pub use engine::{should_flush_soft, CalendarEngine, StopResult};
pub use rng_stream::RngStream;
pub use subsystem::{DayContext, DayReport, StateMutation, StopClass, Subsystem, SubsystemId};
pub use tuning::SIM_VERSION;
