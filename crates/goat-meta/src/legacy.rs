//! Legacy evidence and axis computation.
//!
//! Evidence is accumulated during play (stored in WorldState counters).
//! Axes are computed on demand — never stored as truth, always re-derived.

use goat_fixed::Fixed;

/// Raw accumulated career evidence. All fields are simple counters from WorldState.
#[derive(Debug, Clone, Copy, Default)]
pub struct LegacyEvidence {
    pub career_goals: u32,
    pub career_matches: u32,
    /// Sum of per-match output scores (0–100 each).
    pub career_output_sum: i64,
    /// Best single-season average output (0–100).
    pub best_season_avg_output: i32,
    pub seasons_played: u32,
    /// Goals or assists in finals or decisive moments.
    pub decisive_moments: u32,
    /// Player-of-year award wins.
    pub player_of_year_wins: u32,
    /// League titles won.
    pub league_titles: u32,
    /// Distinct clubs served.
    pub clubs_served: u32,
    /// Longest consecutive tenure at one club (in seasons).
    pub longest_club_tenure: u32,
}

/// The 7+1 legacy axes (all 0–100 Fixed).
///
/// Icon and HeadToHead are stubs until Phase 9/10.
#[derive(Debug, Clone, Copy)]
pub struct LegacyAxes {
    /// Trophy haul + decisive contributions to winning.
    pub winning: Fixed,
    /// Awards and nominations received.
    pub accolades: Fixed,
    /// Career-long performance level.
    pub output: Fixed,
    /// Career duration and availability.
    pub longevity: Fixed,
    /// Performance in decisive moments and finals.
    pub decisive: Fixed,
    /// One-club loyalty and club tenure.
    pub loyalty: Fixed,
    /// Public profile and marketability (stub at 50).
    pub icon: Fixed,
    /// Head-to-head vs contemporaries (stub at 50).
    pub head_to_head: Fixed,
}

impl LegacyAxes {
    pub const STUB_50: Fixed = Fixed::raw(50_000);

    /// Index order used by school weight arrays.
    pub fn as_array(&self) -> [Fixed; 8] {
        [
            self.winning,
            self.accolades,
            self.output,
            self.longevity,
            self.decisive,
            self.loyalty,
            self.icon,
            self.head_to_head,
        ]
    }
}

/// Derive the 7+1 axes from accumulated evidence.
///
/// All formulas use integer arithmetic to stay deterministic.
pub fn compute_axes(ev: &LegacyEvidence) -> LegacyAxes {
    // ── Winning ───────────────────────────────────────────────────────────────
    // League titles carry heavy weight; decisive moments round it out.
    let winning_raw = (ev.league_titles * 20 + ev.decisive_moments * 5).min(100);
    let winning = Fixed::from_int(winning_raw as i32);

    // ── Accolades ─────────────────────────────────────────────────────────────
    let accolades_raw =
        (ev.player_of_year_wins * 35 + ev.league_titles * 5 + ev.decisive_moments * 2).min(100);
    let accolades = Fixed::from_int(accolades_raw as i32);

    // ── Output ────────────────────────────────────────────────────────────────
    // Average career output + best season avg, equally weighted.
    let avg_output = if ev.career_matches > 0 {
        (ev.career_output_sum / ev.career_matches as i64).clamp(0, 100) as i32
    } else {
        0
    };
    let output_raw = ((avg_output + ev.best_season_avg_output) / 2).clamp(0, 100);
    let output = Fixed::from_int(output_raw);

    // ── Longevity ─────────────────────────────────────────────────────────────
    // 300+ matches = 100; scales linearly.
    let longevity_raw = ((ev.career_matches as i32 * 100) / 300).clamp(0, 100);
    let longevity = Fixed::from_int(longevity_raw);

    // ── Decisive ──────────────────────────────────────────────────────────────
    let decisive_raw = (ev.decisive_moments as i32 * 10).clamp(0, 100);
    let decisive = Fixed::from_int(decisive_raw);

    // ── Loyalty ───────────────────────────────────────────────────────────────
    // One club = 100; each additional club costs 20; tenure bonus scales it up.
    let clubs_pen = (ev.clubs_served.saturating_sub(1) * 20).min(80) as i32;
    let tenure_bonus = (ev.longest_club_tenure as i32 * 5).min(20);
    let loyalty_raw = (100 - clubs_pen + tenure_bonus).clamp(0, 100);
    let loyalty = Fixed::from_int(loyalty_raw);

    LegacyAxes {
        winning,
        accolades,
        output,
        longevity,
        decisive,
        loyalty,
        icon: LegacyAxes::STUB_50,
        head_to_head: LegacyAxes::STUB_50,
    }
}

/// Axis names for TUI display, indexed by LegacyAxes::as_array() order.
pub const AXIS_NAMES: [&str; 8] = [
    "Winning",
    "Accolades",
    "Output",
    "Longevity",
    "Decisive",
    "Loyalty",
    "Icon",
    "Head-to-Head",
];
