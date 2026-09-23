//! Beat data model for the Match Flow engine.
//!
//! A beat = commentary situation → choice → contest → outcome, where the outcome
//! carries the match-flow transitions (possession / zone / momentum / chain) that
//! drive the next tick. All types here are assembled at runtime from the JSON
//! pools in `beats_data`.

use goat_core::attrs::AttrId;
use goat_core::roles::PitchZone;

// ── Possession ────────────────────────────────────────────────────────────────

/// Which side holds the ball on this tick of the match flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Possession {
    /// The PC's team.
    Own,
    /// The opposition.
    Opp,
}

impl Possession {
    pub fn flip(self) -> Self {
        match self {
            Possession::Own => Possession::Opp,
            Possession::Opp => Possession::Own,
        }
    }
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

// ── Goal credits (PA2 M3) ─────────────────────────────────────────────────────

/// Who a goal is attributed to. `Npc(None)` = a named squad player whose
/// population identity is unknown (stub sheets in harnesses).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalActor {
    Pc,
    Npc(Option<u32>),
}

/// Attribution of one scored goal — pure bookkeeping derived from draws the
/// commentary already makes (adding this never touches the RNG stream, so the
/// match flow is unchanged).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoalCredit {
    pub event: ScoreEvent,
    pub scorer: GoalActor,
    /// `Some(Pc)` when the PC's delivery was finished by a teammate.
    pub assist: Option<GoalActor>,
}

// ── Headspace delta ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Default)]
pub struct HeadspaceDelta {
    pub confidence: i8,
    pub frustration: i8,
    pub flow: i8,
}

// ── Generated beat types (assembled at runtime from the JSON pools) ───────────

#[derive(Debug, Clone)]
pub struct GeneratedOutcome {
    pub text: String,
    pub output_delta: i16,
    pub headspace: HeadspaceDelta,
    pub score_event: Option<ScoreEvent>,
    pub stamina_cost: u8,
    /// Momentum swing applied to the match flow (clamped to ±100 total).
    pub momentum_delta: i8,
    /// Possession after this beat. `None` = unchanged.
    pub possession_to: Option<Possession>,
    /// Where the ball moves after this beat. `None` = unchanged.
    pub zone_to: Option<PitchZone>,
    /// Immediately auto-resolve a follow-up beat on this side (chain cap applies).
    pub chain: Option<Possession>,
}

#[derive(Debug, Clone)]
pub struct GeneratedChoice {
    pub text: String,
    pub primary: AttrId,
    /// Effective difficulty after opponent-stat scaling (set when the beat is built).
    pub difficulty: u8,
    /// Chance (per 100) this action draws a foul review.
    pub foul_chance: u8,
    /// True = any resulting card is for a serious offence.
    pub foul_serious: bool,
    pub success: GeneratedOutcome,
    pub failure: GeneratedOutcome,
    /// Goal attribution when the success/failure outcome carries a score event
    /// (PA2 M3). Computed at beat-build time from the raw (pre-slot-fill) text.
    pub success_credit: Option<GoalCredit>,
    pub failure_credit: Option<GoalCredit>,
}

#[derive(Debug, Clone)]
pub struct GeneratedBeat {
    pub situation_id: String,
    /// Commentary setup line presented to the player.
    pub setup: String,
    pub zone: PitchZone,
    pub side: Possession,
    pub choices: Vec<GeneratedChoice>,
    /// Name of the opposition danger man when THIS beat's matchup is him
    /// (strongest man in the matchup pool above the absolute bar —
    /// `SquadSheet::danger_man_in`). Counter/recap only: computing it is a
    /// deterministic scan that consumes no RNG, so it cannot perturb a match.
    pub danger_man: Option<String>,
}
