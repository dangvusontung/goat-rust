//! All tunable numbers for the training subsystem (docs/TRAINING.md §11:
//! "Numbers are illustrative placeholders (final tuning deferred)").
//!
//! Every constant here is a **TUNABLE placeholder** — defensible starting
//! values, never final. Nothing in the sim may hard-code a numeric literal;
//! if a number drives behavior, it lives here.
//!
//! Wiring note for the later integration round: `goat-core`'s week loop has
//! its own weekly training numbers (`goat_core::tuning::*`). This crate's
//! per-DAY constants are deliberately independent while Phase 1 stands alone;
//! reconcile the two when the subsystem is wired into the live loop.

use goat_fixed::Fixed;

// ── Energy (bible §5.4) — TUNABLE ────────────────────────────────────────────

/// Energy ceiling. Matches the game's 0–100 resource scale.
pub const ENERGY_MAX: Fixed = Fixed::from_int(100);
/// Energy floor.
pub const ENERGY_MIN: Fixed = Fixed::ZERO;

/// Per-day energy cost of a training session, by intensity.
pub const ENERGY_SPEND_LIGHT: Fixed = Fixed::from_int(4);
pub const ENERGY_SPEND_MODERATE: Fixed = Fixed::from_int(7);
pub const ENERGY_SPEND_HARD: Fixed = Fixed::from_int(12);

/// Per-day energy recovered on a pure rest day (no training, no match).
pub const ENERGY_RECOVER_REST: Fixed = Fixed::from_int(9);

/// Growth multiplier at zero energy — tired players still gain something, but
/// much less. Linear ramp to 1.0 at full energy (same shape as the legacy week
/// loop's "full energy = 1.0, zero = 0.6").
pub const ENERGY_FACTOR_AT_ZERO_X1000: i32 = 600;
/// Divisor mapping energy (0..100) onto the factor ramp's span (0.4 = 1.0-0.6).
pub const ENERGY_FACTOR_SPAN_X1000: i32 = 400;

// ── Intensity (bible §5.4) — TUNABLE ─────────────────────────────────────────

/// Growth multiplier per intensity tier (×1000: 1.0 = Moderate baseline).
pub const INTENSITY_FACTOR_LIGHT_X1000: i32 = 500;
pub const INTENSITY_FACTOR_MODERATE_X1000: i32 = 1_000;
pub const INTENSITY_FACTOR_HARD_X1000: i32 = 1_500;

// ── Growth (bible §5.4) — TUNABLE ────────────────────────────────────────────

/// Base per-day growth before all modifiers (0.07/day ≈ 0.5/week at Moderate
/// with full energy — deliberately close to the legacy week loop's pace so the
/// later wiring round doesn't change felt development speed).
pub const BASE_GROWTH_PER_DAY: Fixed = Fixed::raw(70);

/// Headroom taper: growth runs at full speed until this many points from the
/// ceiling, then scales down linearly — approaching potential slows naturally
/// instead of hitting a wall.
pub const HEADROOM_FULL_SPEED_POINTS: i32 = 20;

/// Seeded per-day jitter, ± this many raw thousandths (deterministic variance,
/// same idiom as the legacy loop's GROWTH_VARIANCE_RAW).
pub const GROWTH_JITTER_RAW: i32 = 20;

/// Hard cap on one day's growth (0.200) — a single day can never leap.
pub const GROWTH_DAY_CAP: Fixed = Fixed::raw(200);

/// Days per year for age-band lookup (calendar runs Mon-Sun weeks; the spec's
/// DevelopmentInput carries age in days).
pub const DAYS_PER_YEAR: u32 = 365;

// ── Trainability curves (bible §5.1) — TUNABLE ───────────────────────────────
// Three DISTINCT archetype curves, ×1000 multipliers by age band. Do NOT
// collapse these into one scaled curve: the late-career reinvention arc
// (§5.2) hangs on Mental appreciating while Physical fades.

/// Physical: low trainability, declines early (pace goes first).
/// Bands: ≤17 / 18–22 / 23–26 / 27–29 / 30+.
pub const TRAINABILITY_PHYSICAL_X1000: [i32; 5] = [700, 550, 350, 150, 0];
/// Technical: high, broad mid-career plateau.
/// Bands: ≤17 / 18–27 / 28–31 / 32–35 / 36+.
pub const TRAINABILITY_TECHNICAL_X1000: [i32; 5] = [800, 1_000, 700, 400, 150];
/// Mental: grows with experience/age, not raw training input — peaks latest.
/// Bands: ≤17 / 18–25 / 26–33 / 34–37 / 38+.
pub const TRAINABILITY_MENTAL_X1000: [i32; 5] = [400, 650, 1_000, 700, 300];

/// Age band edges in years for each archetype's table above (4 edges → 5 bands).
pub const AGE_BAND_EDGES_PHYSICAL: [u32; 4] = [17, 22, 26, 29];
pub const AGE_BAND_EDGES_TECHNICAL: [u32; 4] = [17, 27, 31, 35];
pub const AGE_BAND_EDGES_MENTAL: [u32; 4] = [17, 25, 33, 37];

// ── Events (US-05) — TUNABLE ─────────────────────────────────────────────────

/// Breakthrough milestone: crossing a multiple of this many raw points (5.000)
/// in a day raises the Breakthrough soft flashpoint.
pub const BREAKTHROUGH_MILESTONE_RAW: i32 = 5_000;

/// Overtrained threshold: training while energy is below this (20.000) raises
/// the Overtrained soft flashpoint.
pub const OVERTRAINED_ENERGY_RAW: i32 = 20_000;
