//! Sub-attribute definitions, family groupings, and age-curve archetypes.
//!
//! Thirty outfield sub-attributes across six families. Goalkeeping family is
//! parked (bible §11) — placeholder documented but not implemented.

use goat_fixed::Fixed;

/// Total number of sub-attributes implemented.
pub const NUM_ATTRS: usize = 30;

/// Which age curve an attribute follows (bible §5.1).
///
/// Determines the starting-current percentage at age 16 and how the attribute
/// peaks and declines across a career.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgeCurveArchetype {
    /// Peaks early (~22–24), declines after ~28. High starting %.
    Physical,
    /// Peaks mid-career (~25–29), mild decline. Medium starting %.
    Technical,
    /// Peaks late (~28–32), barely declines. Low starting % — matures slowly.
    Mental,
}

/// Unique identifier for each sub-attribute (C.1 attribute set).
///
/// The `usize` discriminant is the column index in `PlayerStore` arrays.
/// **Order here is load-bearing** — do not reorder without migrating all tables.
/// Indices 0–29 are frozen; only the semantic meaning of some slots changed in
/// the C.1 restructure (e.g. [12] Dribbling → CloseControl, [21] DefAwareness → Curve).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum AttrId {
    // ── Pace (0–1) ────────────────────────────────────────────────────────────
    Acceleration = 0,
    SprintSpeed = 1,
    // ── Shooting (2–6, plus AttPositioning[22]) ───────────────────────────────
    Finishing = 2,
    LongShots = 3,
    ShotPower = 4,
    Volleys = 5,
    Penalties = 6,
    // ── Passing (7–11, plus Curve[21]) ────────────────────────────────────────
    ShortPassing = 7,
    LongPassing = 8,
    Vision = 9,
    Crossing = 10,
    FreeKickAcc = 11,
    // ── Dribbling (12–16, plus Composure[27]) ─────────────────────────────────
    CloseControl = 12,
    BallControl = 13,
    Agility = 14,
    Balance = 15,
    Reactions = 16,
    // ── Defending (17–20, plus SlidingTackle[29]) ─────────────────────────────
    StandingTackle = 17,
    Marking = 18,
    Interceptions = 19,
    Heading = 20,
    // ── Passing cont. (slot 21) ───────────────────────────────────────────────
    Curve = 21,
    // ── Shooting cont. (slot 22) ──────────────────────────────────────────────
    AttPositioning = 22,
    // ── Physical (23–26, plus Bravery[28]) ────────────────────────────────────
    Strength = 23,
    Stamina = 24,
    Aggression = 25,
    Jumping = 26,
    // ── Dribbling cont. (slot 27) ─────────────────────────────────────────────
    Composure = 27,
    // ── Physical cont. (slot 28) ──────────────────────────────────────────────
    Bravery = 28,
    // ── Defending cont. (slot 29) ─────────────────────────────────────────────
    SlidingTackle = 29,
}

impl AttrId {
    /// All 30 attributes in discriminant order. Used for safe u8 → AttrId conversion.
    pub const ALL: [AttrId; NUM_ATTRS] = [
        AttrId::Acceleration,
        AttrId::SprintSpeed,
        AttrId::Finishing,
        AttrId::LongShots,
        AttrId::ShotPower,
        AttrId::Volleys,
        AttrId::Penalties,
        AttrId::ShortPassing,
        AttrId::LongPassing,
        AttrId::Vision,
        AttrId::Crossing,
        AttrId::FreeKickAcc,
        AttrId::CloseControl,
        AttrId::BallControl,
        AttrId::Agility,
        AttrId::Balance,
        AttrId::Reactions,
        AttrId::StandingTackle,
        AttrId::Marking,
        AttrId::Interceptions,
        AttrId::Heading,
        AttrId::Curve,
        AttrId::AttPositioning,
        AttrId::Strength,
        AttrId::Stamina,
        AttrId::Aggression,
        AttrId::Jumping,
        AttrId::Composure,
        AttrId::Bravery,
        AttrId::SlidingTackle,
    ];
}

/// Age-curve archetype for each attribute, indexed by `AttrId as usize`.
///
/// Determines starting current value and growth/decline trajectory.
pub const ATTR_ARCHETYPES: [AgeCurveArchetype; NUM_ATTRS] = [
    // Pace [0–1]
    AgeCurveArchetype::Physical, // Acceleration
    AgeCurveArchetype::Physical, // SprintSpeed
    // Shooting [2–6]
    AgeCurveArchetype::Technical, // Finishing
    AgeCurveArchetype::Technical, // LongShots
    AgeCurveArchetype::Technical, // ShotPower
    AgeCurveArchetype::Technical, // Volleys
    AgeCurveArchetype::Technical, // Penalties
    // Passing [7–11]
    AgeCurveArchetype::Technical, // ShortPassing
    AgeCurveArchetype::Technical, // LongPassing
    AgeCurveArchetype::Mental,    // Vision
    AgeCurveArchetype::Technical, // Crossing
    AgeCurveArchetype::Technical, // FreeKickAcc
    // Dribbling [12–16]
    AgeCurveArchetype::Technical, // CloseControl
    AgeCurveArchetype::Technical, // BallControl
    AgeCurveArchetype::Physical,  // Agility
    AgeCurveArchetype::Physical,  // Balance
    AgeCurveArchetype::Mental,    // Reactions
    // Defending [17–20]
    AgeCurveArchetype::Technical, // StandingTackle
    AgeCurveArchetype::Mental,    // Marking  ← C.1: Mental (reads the game, not physical)
    AgeCurveArchetype::Mental,    // Interceptions  ← C.1: Mental
    AgeCurveArchetype::Physical,  // Heading  ← C.1: Physical (jump timing / aerial power)
    // Passing cont. [21]
    AgeCurveArchetype::Technical, // Curve  ← C.1: Technical
    // Shooting cont. [22]
    AgeCurveArchetype::Mental, // AttPositioning  ← C.1: Mental (reading space)
    // Physical [23–26]
    AgeCurveArchetype::Physical, // Strength
    AgeCurveArchetype::Physical, // Stamina
    AgeCurveArchetype::Mental,   // Aggression
    AgeCurveArchetype::Physical, // Jumping
    // Dribbling cont. [27]
    AgeCurveArchetype::Mental, // Composure
    // Physical cont. [28]
    AgeCurveArchetype::Mental, // Bravery  ← C.1: Mental
    // Defending cont. [29]
    AgeCurveArchetype::Technical, // SlidingTackle  ← C.1: Technical
];

/// Human-readable name for each attribute, for display in the TUI.
pub const ATTR_NAMES: [&str; NUM_ATTRS] = [
    "Acceleration",     // [0]
    "Sprint Speed",     // [1]
    "Finishing",        // [2]
    "Long Shots",       // [3]
    "Shot Power",       // [4]
    "Volleys",          // [5]
    "Penalties",        // [6]
    "Short Pass",       // [7]
    "Long Pass",        // [8]
    "Vision",           // [9]
    "Crossing",         // [10]
    "FK Accuracy",      // [11]
    "Close Control",    // [12]
    "Ball Control",     // [13]
    "Agility",          // [14]
    "Balance",          // [15]
    "Reactions",        // [16]
    "Standing Tackle",  // [17]
    "Marking",          // [18]
    "Interceptions",    // [19]
    "Heading",          // [20]
    "Curve",            // [21]
    "Att. Positioning", // [22]
    "Strength",         // [23]
    "Stamina",          // [24]
    "Aggression",       // [25]
    "Jumping",          // [26]
    "Composure",        // [27]
    "Bravery",          // [28]
    "Sliding Tackle",   // [29]
];

/// Attribute family: a derived display value (average of sub-attrs; never stored as truth).
#[derive(Debug, Clone, Copy)]
pub struct AttrFamily {
    pub pace: Fixed,
    pub shooting: Fixed,
    pub passing: Fixed,
    pub dribbling: Fixed,
    pub defending: Fixed,
    pub physical: Fixed,
}

/// Sub-attribute indices belonging to each display family (C.1).
///
/// These drive the six family-average bars shown in the TUI. They are NOT the
/// per-position weight tables used for OVR — see `positions::POSITION_WEIGHT_TABLE`.
pub const PACE_ATTRS: &[usize] = &[0, 1];
pub const SHOOTING_ATTRS: &[usize] = &[2, 3, 4, 5, 6, 22]; // +AttPositioning
pub const PASSING_ATTRS: &[usize] = &[7, 8, 9, 10, 11, 21]; // +Curve
pub const DRIBBLING_ATTRS: &[usize] = &[12, 13, 14, 15, 16, 27]; // +Composure
pub const DEFENDING_ATTRS: &[usize] = &[17, 18, 19, 20, 29]; // +SlidingTackle; -Curve,-AttPositioning
pub const PHYSICAL_ATTRS: &[usize] = &[23, 24, 25, 26, 28]; // +Bravery; -Composure,-SlidingTackle
