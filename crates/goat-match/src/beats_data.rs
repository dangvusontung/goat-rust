//! Raw data types that deserialise from beats.json.
//!
//! These are the authoring-time building blocks for the Match Flow engine.
//! Situations are pure commentary (text + metadata); actions are the player's
//! options; outcomes carry the stat deltas and the match-flow transitions that
//! decide what happens next.

use serde::Deserialize;

// ── Raw situation (commentary template) ───────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct RawSituation {
    pub id: String,
    /// Commentary line shown when this situation arises.
    pub text: String,
    /// "defense" | "midfield" | "attack_wide" | "attack_central" | "any".
    #[serde(default = "default_any")]
    pub zone: String,
    /// "attack" (own possession) | "defend" (opp possession) | "any".
    #[serde(default = "default_any")]
    pub side: String,
    /// Match contexts where this may appear: "level" | "leading" | "trailing" |
    /// "late" | "key" | "any". A situation with "late" also needs the trailing/
    /// leading/level entry it belongs with (contexts are OR-matched).
    #[serde(default)]
    pub context: Vec<String>,
    /// Tactical styles this situation belongs to ("pressing" | "possession" |
    /// "counter" | "wing_play"). Empty = neutral to both teams' profiles.
    #[serde(default)]
    pub style: Vec<String>,
    /// Base selection weight (before style/context multipliers).
    #[serde(default = "default_weight")]
    pub weight: u8,
}

fn default_any() -> String {
    "any".to_string()
}
fn default_weight() -> u8 {
    3
}

// ── Raw action ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct RawAction {
    pub id: String,
    /// What the player sees as the choice option.
    pub text: String,
    /// Attribute name — matched to AttrId at load time.
    pub attr: String,
    /// Base difficulty 1–99, before opponent-stat scaling.
    pub difficulty: u8,
    /// Zones where this action is available.
    pub zones: Vec<String>,
    /// Role families allowed: "defender" | "midfielder" | "forward".
    pub roles: Vec<String>,
    /// "attack" | "defend" — which possession side this action belongs to.
    pub side: String,
    /// Chance (per 100) the action draws a foul review, regardless of success.
    #[serde(default)]
    pub foul_chance: u8,
    /// True = the foul is a booking-worthy (serious) offence.
    #[serde(default)]
    pub foul_serious: bool,
}

// ── Raw outcome ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct RawOutcome {
    pub id: String,
    pub text: String,
    /// "success" | "failure" | "any" — which contest result this attaches to.
    #[serde(default = "default_polarity")]
    pub polarity: String,
    /// "attack" | "defend" | "any" — keeps text coherent with the action's side.
    #[serde(default = "default_any")]
    pub side: String,
    #[serde(default)]
    pub output_delta: i16,
    #[serde(default)]
    pub confidence: i8,
    #[serde(default)]
    pub frustration: i8,
    #[serde(default)]
    pub flow: i8,
    /// null | "goal_for" | "goal_against"
    #[serde(default)]
    pub score_event: Option<String>,
    /// True = neutral commentary for goals the PC was NOT involved in (used by
    /// the auto-beat goal roll). Player-facing goal outcomes leave this false
    /// so their 2nd-person text never appears without the PC on the ball.
    #[serde(default)]
    pub auto_commentary: bool,
    #[serde(default)]
    pub stamina_cost: u8,
    /// Momentum swing (−100..100 scale); applied then clamped.
    #[serde(default)]
    pub momentum_delta: i8,
    /// null | "own" | "opp" — possession after this beat.
    #[serde(default)]
    pub possession_to: Option<String>,
    /// null | zone string — where the ball moves after this beat.
    #[serde(default)]
    pub zone_to: Option<String>,
    /// null | "attack" | "defend" — immediately auto-resolve a follow-up beat
    /// on that side (max CHAIN_MAX per tick).
    #[serde(default)]
    pub chain: Option<String>,
}

fn default_polarity() -> String {
    "any".to_string()
}

// ── Root library ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct RawBeatLibrary {
    pub situations: Vec<RawSituation>,
    pub actions: Vec<RawAction>,
    pub outcomes: Vec<RawOutcome>,
}

impl RawBeatLibrary {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
