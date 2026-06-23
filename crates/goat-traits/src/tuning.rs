//! Tuning constants for the trait system.
//!
//! All values are labelled TUNABLE — §11 defers final numbers to playtesting.
//! These are placeholders illustrating the intended distribution.

// ── Trait ceiling roll distribution ─────────────────────────────────────────
//
// At character creation, each trait's ceiling tier is rolled from 0..99.
// Breakdown of the four outcomes (must sum to 100):
//
//   None      : 54%  — player has no aptitude for this trait
//   Raw       : 25%  — will never exceed Raw mastery
//   Proficient: 15%  — can become genuinely capable (most "skilled" players)
//   Mastery   : 6%   — rare gift; the top-of-the-art ceiling
//
// TUNABLE — rarity of Mastery is a major feel lever: too high dilutes the
// "this player is special" signal; too low starves the beat library of variety.

/// Percentage of trait-ceiling rolls that land at Mastery.  TUNABLE.
pub const CEIL_MASTERY_PER_100: u32 = 6;

/// Percentage of rolls that land at Proficient.  TUNABLE.
pub const CEIL_PROFICIENT_PER_100: u32 = 15;

/// Percentage of rolls that land at Raw.  TUNABLE.
pub const CEIL_RAW_PER_100: u32 = 25;
// None = 100 − 6 − 15 − 25 = 54

// ── Starting tier bump ───────────────────────────────────────────────────────

/// Percentage chance that a Mastery-ceiling player starts at Raw rather than None.
///
/// The "raised on it from childhood" prodigy roll (§A.3). Deliberately rare so
/// a player arriving at Mastery is a jackpot story, not the default.  TUNABLE.
pub const START_RAW_IF_MASTERY_CEIL: u32 = 10; // 10% of Mastery-ceiling players
