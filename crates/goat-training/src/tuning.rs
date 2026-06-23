//! All tunable constants for the training subsystem.
//!
//! Every number here is labelled TUNABLE — bible §11 defers final values to
//! playtesting. These are defensible placeholders illustrating shape, not finished
//! game-balance numbers. Do NOT inline any of these in logic code.

use goat_fixed::Fixed;

// ── Energy ────────────────────────────────────────────────────────────────────

/// Maximum energy. 100.0 fixed-point.
pub const ENERGY_MAX: Fixed = Fixed::raw(100_000);

/// Energy spent per training day at Light intensity.
/// Net positive with rest: Light + rest days should sustain 100 energy.
/// TUNABLE
pub const ENERGY_COST_LIGHT: Fixed = Fixed::raw(3_000); // 3.0 / day

/// Energy spent per training day at Moderate intensity.
/// Net-neutral: roughly sustainable with one rest day per week.
/// TUNABLE
pub const ENERGY_COST_MODERATE: Fixed = Fixed::raw(8_000); // 8.0 / day

/// Energy spent per training day at Hard intensity.
/// Drains ~15/day; depletes from full in ~6.7 days of no rest.
/// TUNABLE
pub const ENERGY_COST_HARD: Fixed = Fixed::raw(15_000); // 15.0 / day

/// Energy recovered per rest day (no training, no match).
/// TUNABLE
pub const ENERGY_RECOVERY_REST: Fixed = Fixed::raw(12_000); // 12.0 / day

/// Energy recovered on a match day (handled by match subsystem later; placeholder here).
/// For Phase 1, match days yield same recovery as rest days.
/// TUNABLE
pub const ENERGY_RECOVERY_MATCH: Fixed = Fixed::raw(5_000); // 5.0 / day

// ── Growth base ───────────────────────────────────────────────────────────────

/// Base attribute gain per training day, before all multipliers.
/// Calibrated so a season (~200 training days) moves a young Technical attr
/// from 60 → ~85 at Moderate intensity with full energy and 1.0 facility mult.
/// TUNABLE
pub const BASE_GROWTH_PER_DAY: Fixed = Fixed::raw(40); // 0.040 / day

// ── Intensity factors (multiplier on BASE_GROWTH_PER_DAY) ─────────────────────

/// TUNABLE
pub const INTENSITY_FACTOR_LIGHT: Fixed = Fixed::raw(600); // 0.600

/// TUNABLE
pub const INTENSITY_FACTOR_MODERATE: Fixed = Fixed::raw(1_000); // 1.000

/// TUNABLE
pub const INTENSITY_FACTOR_HARD: Fixed = Fixed::raw(1_500); // 1.500

// ── Energy factor on growth (low energy → smaller gains) ─────────────────────

/// Growth factor at full energy (100). No penalty.
pub const ENERGY_FACTOR_FULL: Fixed = Fixed::raw(1_000); // 1.000

/// Growth factor at zero energy. Growth is not zero, just reduced.
/// TUNABLE
pub const ENERGY_FACTOR_EMPTY: Fixed = Fixed::raw(600); // 0.600

// ── Headroom scaling ──────────────────────────────────────────────────────────

/// When the attribute is this many points below potential, full growth rate applies.
/// Below this threshold, growth scales down linearly toward 0 at the ceiling.
/// Prevents runaway over-shooting and creates natural diminishing returns.
/// TUNABLE
pub const HEADROOM_FULL_THRESHOLD: Fixed = Fixed::from_int(30); // 30.0 attribute points

// ── Growth variance ───────────────────────────────────────────────────────────

/// Additive jitter band: ± this many raw Fixed units on the final daily gain.
/// Provides week-to-week feel without breaking determinism (seeded).
/// TUNABLE
pub const GROWTH_VARIANCE_RAW: u64 = 15; // ±0.015

// ── Trainability curves (archetype × age band) ────────────────────────────────
//
// Three DISTINCT curves (§5.1). Deliberately NOT a single scaled curve —
// collapsing them would erase the late-career mental reinvention arc (§5.2).
//
// Physical  → peaks early (~22–24), declines after ~28; low ceiling on gains.
// Technical → peaks mid-career (~25–29), broad plateau; mild decline.
// Mental    → appreciates with age/experience; peaks 26–33; barely declines (§5.2).

// Physical — low overall trainability (§5.1 "Physical: low trainability").
// Physical attrs already start at 85% of potential (PHYSICAL_START_PCT in goat-core),
// so headroom is small AND trainability is low. This means physical gains are always
// modest — even at peak age — which is why Technical always outgains Physical (AC-02).
pub const PHYS_TRAIN_YOUNG: Fixed = Fixed::raw(500); // ≤17 yrs: 0.500
pub const PHYS_TRAIN_EARLY: Fixed = Fixed::raw(400); // 18–22 yrs: 0.400
pub const PHYS_TRAIN_MID: Fixed = Fixed::raw(200); // 23–27 yrs: 0.200
                                                   // 28+ yrs: Fixed::ZERO — too old to improve physically via training

// Technical
pub const TECH_TRAIN_YOUNG: Fixed = Fixed::raw(700); // ≤17 yrs: 0.700
pub const TECH_TRAIN_PRIME: Fixed = Fixed::raw(1_000); // 18–27 yrs: 1.000 — peak
pub const TECH_TRAIN_LATE: Fixed = Fixed::raw(600); // 28–31 yrs: 0.600
pub const TECH_TRAIN_SENIOR: Fixed = Fixed::raw(300); // 32–35 yrs: 0.300
pub const TECH_TRAIN_OLD: Fixed = Fixed::raw(100); // 36+ yrs: 0.100

// Mental — appreciates with age; peaks 26–33 (§5.2 late-career reinvention arc).
pub const MENT_TRAIN_YOUNG: Fixed = Fixed::raw(500); // ≤17 yrs: 0.500
pub const MENT_TRAIN_EARLY: Fixed = Fixed::raw(700); // 18–25 yrs: 0.700
pub const MENT_TRAIN_PEAK: Fixed = Fixed::raw(900); // 26–33 yrs: 0.900 — highest
pub const MENT_TRAIN_SENIOR: Fixed = Fixed::raw(500); // 34–37 yrs: 0.500
pub const MENT_TRAIN_OLD: Fixed = Fixed::raw(200); // 38+ yrs: 0.200

// ── Event thresholds ──────────────────────────────────────────────────────────

/// Player is flagged Overtrained when energy drops below this while training Hard.
/// TUNABLE
pub const OVERTRAINED_ENERGY_THRESHOLD: Fixed = Fixed::raw(20_000); // 20.0

/// Attribute milestones that trigger a Breakthrough SoftFlashpoint.
/// Values are raw Fixed integer parts (multiply by 1 000 to get Fixed::raw).
/// TUNABLE
pub const BREAKTHROUGH_MILESTONES: [i32; 5] = [
    40_000, // 40.0
    50_000, // 50.0
    60_000, // 60.0
    70_000, // 70.0
    80_000, // 80.0
];

// ── Domain hash constant ──────────────────────────────────────────────────────

/// FNV-1a 64-bit hash of "training" — precomputed at compile time.
/// XOR'd with a per-day calendar roll to derive a training-specific day RNG seed,
/// isolating training variance from all other subsystem rolls (§9).
pub const TRAINING_DOMAIN_HASH: u64 = fnv1a_const(b"training");

const fn fnv1a_const(s: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut i = 0;
    while i < s.len() {
        h ^= s[i] as u64;
        h = h.wrapping_mul(0x100000000001b3);
        i += 1;
    }
    h
}
