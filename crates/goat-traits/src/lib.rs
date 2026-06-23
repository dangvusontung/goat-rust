#![forbid(unsafe_code)]
//! `goat-traits` — Traits & Mastery system (MAIN.md Part 5 / §A).
//!
//! A trait is NOT an attribute. Attributes are numbers (1–99) that scale contests
//! quantitatively. Traits are discrete mastery tiers that bend the match engine
//! *qualitatively*: they summon beats, unlock choices, rewrite finishes, or colour
//! in-match psychology. Two players with identical OVR feel nothing alike when
//! their traits differ.
//!
//! # Phase 1 scope
//! - Core type system: `MasteryTier`, `TraitId`, `PlayerTraits`.
//! - Six beat-summoner traits (§A.2 hook type 2) fully wired into the beat selector.
//! - Ceiling roll at creation (§A.3 aptitude); hidden from the player (§A.4).
//! - Four trait identities whose hooks are scaffolded but land in Phase 2+.
//!
//! # Non-negotiable rules
//! - No `f32`/`f64`. No wall-clock. `#![forbid(unsafe_code)]`.
//! - `current[i] <= ceiling[i]` always — invariant mirroring §2.4 (talent ceiling is law).

pub mod models;
pub mod tuning;

pub use models::{MasteryTier, PlayerTraits, TraitId, NUM_TRAITS};
