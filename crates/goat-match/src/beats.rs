//! Beat data model.
//!
//! A beat = situation → choices → contest → ripple.
//!
//! `Beat` / `BeatChoice` / `Outcome` are the legacy static-library types (still used by
//! `B_RECKLESS_CHALLENGE` and the TUI library path).
//!
//! `GeneratedBeat` / `GeneratedChoice` / `GeneratedOutcome` are the owned equivalents
//! assembled at match-start from the JSON beat library.

use goat_core::attrs::AttrId;

// ── Phase of play ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchPhase {
    OpenPlayAttack,
    OpenPlayDefend,
    OneOnOne,
    SetPiece,
    Positioning,
    KeyMoment,
}

// ── Discipline events ─────────────────────────────────────────────────────────

/// A card shown during a beat — emerges from choices and frustration, not a separate roll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisciplineEvent {
    YellowCard,
    RedCard,
}

// ── Score events ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreEvent {
    /// The PC's team scores. May or may not be credited directly to the player.
    GoalFor,
    /// The opposition scores — bad positioning or lost contest.
    GoalAgainst,
}

// ── Headspace delta ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Default)]
pub struct HeadspaceDelta {
    pub confidence: i8,
    pub frustration: i8,
    pub flow: i8,
}

// ── Beat outcome ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct Outcome {
    /// Short flavor text describing what happened (max ~80 chars).
    pub text: &'static str,
    /// Next beat to fire. `None` = let the selector pick the next situation.
    pub next: Option<usize>,
    /// Change to the player's output score (−20 to +20 per beat).
    pub output_delta: i16,
    pub headspace: HeadspaceDelta,
    pub score_event: Option<ScoreEvent>,
    /// Extra stamina cost beyond the base per-beat drain.
    pub stamina_cost: u8,
}

// ── Beat choice ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct BeatChoice {
    /// What the player sees as the choice option.
    pub text: &'static str,
    /// The primary attribute checked in the contest.
    pub primary: AttrId,
    /// Base difficulty (1–99). Adjusted at runtime by match context.
    pub difficulty: u8,
    pub success: Outcome,
    pub failure: Outcome,
}

// ── Beat ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct Beat {
    /// Index into `BEATS` — must equal position in the array.
    pub id: usize,
    pub phase: MatchPhase,
    /// Selector weights by match period [Early 0–29, Mid 30–59, Late 60–90].
    /// Higher = more likely to be picked in that period.
    pub phase_bias: [u8; 3],
    /// Narrative setup text (1–3 sentences) presented to the player.
    pub setup: &'static str,
    pub choices: &'static [BeatChoice],
}

// ── Generated (owned) equivalents — assembled at match-start from JSON data ──

#[derive(Debug, Clone)]
pub struct GeneratedOutcome {
    pub text: String,
    pub output_delta: i16,
    pub headspace: HeadspaceDelta,
    pub score_event: Option<ScoreEvent>,
    pub stamina_cost: u8,
    /// Situation id to chain into next (overrides random pick).
    pub next_situation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GeneratedChoice {
    pub text: String,
    pub primary: AttrId,
    pub difficulty: u8,
    pub success: GeneratedOutcome,
    pub failure: GeneratedOutcome,
}

#[derive(Debug, Clone)]
pub struct GeneratedBeat {
    pub situation_id: String,
    pub phase: MatchPhase,
    pub setup: String,
    pub choices: Vec<GeneratedChoice>,
}
