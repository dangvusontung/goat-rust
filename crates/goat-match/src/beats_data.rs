//! Raw data types that deserialise from beats.json.
//!
//! These are the authoring-time building blocks. At match start the generator
//! assembles them into `GeneratedBeat`s that the sim engine consumes.

use serde::Deserialize;

// ── Raw situation ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct RawSituation {
    pub id: String,
    /// "attack" | "defend" | "setpiece" | "positioning" | "key"
    pub phase: String,
    /// Weight per match period [early, mid, late]. Higher = more likely.
    pub bias: [u8; 3],
    pub setup: String,
    /// Tags used to filter which choices are eligible for this situation.
    pub tags: Vec<String>,
}

// ── Raw choice ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct RawChoice {
    pub id: String,
    pub text: String,
    /// Attribute name — matched to AttrId at load time.
    pub attr: String,
    /// Base difficulty 1–99.
    pub difficulty: u8,
    /// Tags — a choice is eligible for a situation when they share at least one tag.
    pub tags: Vec<String>,
}

// ── Raw outcome ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct RawOutcome {
    pub id: String,
    pub text: String,
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
    #[serde(default)]
    pub stamina_cost: u8,
    /// Optional: id of a situation to chain into next (overrides random pick).
    #[serde(default)]
    pub next_situation: Option<String>,
    /// "success" | "failure" | "any" — controls which pool this outcome sits in.
    #[serde(default = "default_polarity")]
    pub polarity: String,
}

fn default_polarity() -> String {
    "any".to_string()
}

// ── Root library ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct RawBeatLibrary {
    pub situations: Vec<RawSituation>,
    pub choices: Vec<RawChoice>,
    pub outcomes: Vec<RawOutcome>,
}

impl RawBeatLibrary {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
