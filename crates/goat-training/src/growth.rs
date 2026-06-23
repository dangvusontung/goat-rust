//! Growth formula, trainability curves, and event detection.
//!
//! All math is in goat-fixed. No `f32`/`f64` anywhere in this module (§9).
//! Ceiling clamp uses goat-core's `PlayerStore::set_current` enforcement path
//! (debug_assert + callers clamp to potential) — not a looser re-implementation (§2.4).

use goat_core::attrs::AgeCurveArchetype;
use goat_fixed::Fixed;
use goat_rng::RngSource;

use crate::models::{DevelopmentInput, Intensity, TrainingEvent};
use crate::tuning::{
    BASE_GROWTH_PER_DAY, BREAKTHROUGH_MILESTONES, ENERGY_FACTOR_EMPTY, ENERGY_FACTOR_FULL,
    GROWTH_VARIANCE_RAW, HEADROOM_FULL_THRESHOLD, INTENSITY_FACTOR_HARD, INTENSITY_FACTOR_LIGHT,
    INTENSITY_FACTOR_MODERATE, MENT_TRAIN_EARLY, MENT_TRAIN_OLD, MENT_TRAIN_PEAK,
    MENT_TRAIN_SENIOR, MENT_TRAIN_YOUNG, OVERTRAINED_ENERGY_THRESHOLD, PHYS_TRAIN_EARLY,
    PHYS_TRAIN_MID, PHYS_TRAIN_YOUNG, TECH_TRAIN_LATE, TECH_TRAIN_OLD, TECH_TRAIN_PRIME,
    TECH_TRAIN_SENIOR, TECH_TRAIN_YOUNG,
};

/// Compute the attribute gain for one training day.
///
/// Returns zero when `current >= potential` (ceiling already reached — §2.4).
/// The caller must still clamp the final stored value via `PlayerStore::set_current`
/// (which contains the debug_assert). Do not write a looser ceiling check here.
///
/// Formula: `base × trainable × energy_mod × headroom_mod + jitter`, then
/// clamped to `[0, headroom]` so a single day can never overshoot the ceiling.
pub fn compute_growth(input: &DevelopmentInput, rng: &mut impl RngSource) -> Fixed {
    if input.current >= input.potential {
        return Fixed::ZERO; // ceiling clamp (§2.4)
    }

    let headroom = input.potential - input.current;

    let base = BASE_GROWTH_PER_DAY * intensity_factor(input.intensity);
    let trainable = trainability(input.attr_archetype, input.age_days);

    if trainable == Fixed::ZERO {
        return Fixed::ZERO; // archetype has aged out of improvability
    }

    let energy_mod = energy_factor(input.energy);
    let headroom_mod = headroom_scaled(headroom);
    let facility = input.facility_mult;

    let mid = base * trainable * energy_mod * headroom_mod * facility;

    // Additive jitter: ±GROWTH_VARIANCE_RAW thousandths, seeded for determinism.
    let jitter_raw =
        rng.next_range_u64(0, GROWTH_VARIANCE_RAW * 2) as i32 - GROWTH_VARIANCE_RAW as i32;
    let raw = mid + Fixed::raw(jitter_raw);

    raw.clamp(Fixed::ZERO, headroom)
}

/// Multiplier on `BASE_GROWTH_PER_DAY` for the given intensity tier.
fn intensity_factor(intensity: Intensity) -> Fixed {
    match intensity {
        Intensity::Light => INTENSITY_FACTOR_LIGHT,
        Intensity::Moderate => INTENSITY_FACTOR_MODERATE,
        Intensity::Hard => INTENSITY_FACTOR_HARD,
    }
}

/// Trainability for an attribute archetype at a given age (bible §5.1).
///
/// Three DISTINCT curves — NOT a single scaled curve. Collapsing them into one
/// would erase the late-career mental reinvention arc (§5.2) that the whole design
/// hangs on.
///
/// Returns `Fixed::ZERO` when the player is too old to improve that archetype.
pub fn trainability(archetype: AgeCurveArchetype, age_days: u32) -> Fixed {
    let age_years = age_days / 365;
    match archetype {
        AgeCurveArchetype::Physical => match age_years {
            0..=17 => PHYS_TRAIN_YOUNG,  // 1.000 — peak physical trainability
            18..=22 => PHYS_TRAIN_EARLY, // 0.700
            23..=27 => PHYS_TRAIN_MID,   // 0.350
            _ => Fixed::ZERO,            // 28+ — too old to improve physically via training
        },
        AgeCurveArchetype::Technical => match age_years {
            0..=17 => TECH_TRAIN_YOUNG,   // 0.700
            18..=27 => TECH_TRAIN_PRIME,  // 1.000 — prime technical years
            28..=31 => TECH_TRAIN_LATE,   // 0.600
            32..=35 => TECH_TRAIN_SENIOR, // 0.300
            _ => TECH_TRAIN_OLD,          // 36+ — 0.100, never fully stops
        },
        AgeCurveArchetype::Mental => match age_years {
            // Mental appreciates with age/experience. Peaks 26–33.
            // This is what preserves the late-career reinvention arc (§5.2):
            // a 30-year-old can reinvent as a deeper midfielder through mental growth
            // even as physical attributes fade.
            0..=17 => MENT_TRAIN_YOUNG, // 0.500 — slow start; experience-gated
            18..=25 => MENT_TRAIN_EARLY, // 0.700
            26..=33 => MENT_TRAIN_PEAK, // 0.900 — highest mental trainability
            34..=37 => MENT_TRAIN_SENIOR, // 0.500
            _ => MENT_TRAIN_OLD,        // 38+ — 0.200, fades slowly
        },
    }
}

/// Energy multiplier on growth: 1.0 at full energy, `ENERGY_FACTOR_EMPTY` at zero.
/// Linear interpolation so low-energy training still yields some gain (never zero).
fn energy_factor(energy: Fixed) -> Fixed {
    let pct = energy / Fixed::from_int(100); // 0.0 – 1.0
    ENERGY_FACTOR_EMPTY + (ENERGY_FACTOR_FULL - ENERGY_FACTOR_EMPTY) * pct
}

/// Scale growth by how far below the potential ceiling the player currently sits.
///
/// Returns 1.0 when `headroom >= HEADROOM_FULL_THRESHOLD` (30 attr points).
/// Below that, scales linearly to 0 at the ceiling, creating natural diminishing
/// returns without a hard cliff.
/// TUNABLE — this constant shapes how much growth slows near the ceiling.
fn headroom_scaled(headroom: Fixed) -> Fixed {
    (headroom / HEADROOM_FULL_THRESHOLD).clamp(Fixed::ZERO, Fixed::ONE)
}

/// Detect a notable training event to surface as a SoftFlashpoint.
///
/// Returns the event (if any) based on the attribute's before/after values
/// and the player's energy at the start of the day.
///
/// Called after `compute_growth` has been computed but BEFORE applying to store,
/// so `before` is still the actual current value.
pub fn detect_event(
    before: Fixed,
    after: Fixed,
    energy: Fixed,
    intensity: Intensity,
) -> Option<TrainingEvent> {
    // Overtrained: pushed Hard training through critically low energy.
    if intensity == Intensity::Hard && energy < OVERTRAINED_ENERGY_THRESHOLD {
        return Some(TrainingEvent::Overtrained);
    }

    // Breakthrough: attribute crossed a notable milestone threshold.
    for &raw_milestone in &BREAKTHROUGH_MILESTONES {
        let milestone = Fixed::raw(raw_milestone);
        if before < milestone && after >= milestone {
            return Some(TrainingEvent::Breakthrough);
        }
    }

    None
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DevelopmentInput;
    use goat_core::attrs::AgeCurveArchetype;
    use goat_fixed::Fixed;
    use goat_rng::GoatRng;

    fn make_input(
        archetype: AgeCurveArchetype,
        age_days: u32,
        current: i32,
        potential: i32,
        intensity: Intensity,
        energy: i32,
    ) -> DevelopmentInput {
        DevelopmentInput {
            attr_archetype: archetype,
            age_days,
            current: Fixed::from_int(current),
            potential: Fixed::from_int(potential),
            intensity,
            energy: Fixed::from_int(energy),
            facility_mult: Fixed::ONE,
        }
    }

    fn rng() -> GoatRng {
        GoatRng::new(42)
    }

    // Property test: growth is exactly zero when current == potential (§2.4).
    #[test]
    fn growth_zero_at_ceiling() {
        let input = make_input(
            AgeCurveArchetype::Technical,
            6_570,
            85,
            85,
            Intensity::Hard,
            100,
        );
        let delta = compute_growth(&input, &mut rng());
        assert_eq!(delta, Fixed::ZERO, "no growth when at ceiling");
    }

    // Property test: growth is zero when current > potential (guard, though shouldn't happen).
    #[test]
    fn growth_zero_above_ceiling() {
        let input = make_input(
            AgeCurveArchetype::Technical,
            6_570,
            90,
            85,
            Intensity::Hard,
            100,
        );
        let delta = compute_growth(&input, &mut rng());
        assert_eq!(delta, Fixed::ZERO);
    }

    // Property test: growth never exceeds headroom (no single-day ceiling overshoot).
    #[test]
    fn growth_never_exceeds_headroom() {
        let mut r = GoatRng::new(99);
        for intensity in [Intensity::Light, Intensity::Moderate, Intensity::Hard] {
            for energy in [0i32, 20, 50, 100] {
                for (cur, pot) in [(60, 85), (80, 85), (84, 85)] {
                    let input = make_input(
                        AgeCurveArchetype::Technical,
                        6_570,
                        cur,
                        pot,
                        intensity,
                        energy,
                    );
                    let delta = compute_growth(&input, &mut r);
                    let headroom = Fixed::from_int(pot - cur);
                    assert!(
                        delta <= headroom,
                        "growth {delta:?} exceeded headroom {headroom:?}"
                    );
                }
            }
        }
    }

    // Property test: growth is monotonically non-decreasing in intensity at fixed seed/energy.
    #[test]
    fn growth_monotonic_in_intensity() {
        let age_days = 6_570; // ~18 years
        let energy = 100;

        // Use fixed-jitter seed where all three produce the same jitter offset
        // by running three separate rng instances from the same seed.
        let input_l = make_input(
            AgeCurveArchetype::Technical,
            age_days,
            60,
            85,
            Intensity::Light,
            energy,
        );
        let input_m = make_input(
            AgeCurveArchetype::Technical,
            age_days,
            60,
            85,
            Intensity::Moderate,
            energy,
        );
        let input_h = make_input(
            AgeCurveArchetype::Technical,
            age_days,
            60,
            85,
            Intensity::Hard,
            energy,
        );

        // Use seed 0 which gives deterministic jitter = 0 (next_u64 XOR domain).
        // Actually test with a seed where noise is near the center so ordering holds.
        // Use seed 12345 and check that ordering is consistent — the intensity gaps
        // are large enough that jitter (±0.015) can't flip the order.
        let mut r1 = GoatRng::new(12345);
        let mut r2 = GoatRng::new(12345);
        let mut r3 = GoatRng::new(12345);

        let gain_l = compute_growth(&input_l, &mut r1);
        let gain_m = compute_growth(&input_m, &mut r2);
        let gain_h = compute_growth(&input_h, &mut r3);

        assert!(
            gain_l <= gain_m,
            "light ({gain_l:?}) must be <= moderate ({gain_m:?})"
        );
        assert!(
            gain_m <= gain_h,
            "moderate ({gain_m:?}) must be <= hard ({gain_h:?})"
        );
    }

    // Property test: energy stays in [0, 100] is enforced by the subsystem (tested in training.rs tests).
    // Here just verify energy_factor is bounded.
    #[test]
    fn energy_factor_bounded() {
        // energy_factor is private; test via compute_growth output bounds indirectly.
        // At any energy, growth should be in [0, headroom].
        let mut r = GoatRng::new(7);
        for energy in [0i32, 1, 10, 50, 99, 100] {
            let input = make_input(
                AgeCurveArchetype::Mental,
                10_950,
                50,
                85,
                Intensity::Moderate,
                energy,
            );
            let delta = compute_growth(&input, &mut r);
            assert!(delta >= Fixed::ZERO);
            assert!(delta <= Fixed::from_int(85 - 50));
        }
    }

    // AC-02: Technical out-gains Physical at the same age and intensity (young player).
    #[test]
    fn technical_outgains_physical_at_young_age() {
        let age_days = 6_205; // 17 years (17 * 365)
        let mut r1 = GoatRng::new(1);
        let mut r2 = GoatRng::new(1);

        let input_tech = make_input(
            AgeCurveArchetype::Technical,
            age_days,
            60,
            85,
            Intensity::Moderate,
            100,
        );
        let input_phys = make_input(
            AgeCurveArchetype::Physical,
            age_days,
            60,
            85,
            Intensity::Moderate,
            100,
        );

        let gain_tech = compute_growth(&input_tech, &mut r1);
        let gain_phys = compute_growth(&input_phys, &mut r2);

        assert!(
            gain_tech > gain_phys,
            "technical ({gain_tech:?}) must outgain physical ({gain_phys:?}) at young age"
        );
    }

    // AC-02: Mental out-gains Physical at older age (26–33 range).
    #[test]
    fn mental_outgains_physical_at_peak_mental_age() {
        let age_days = 10_220; // 28 years (28 * 365)
        let mut r1 = GoatRng::new(5);
        let mut r2 = GoatRng::new(5);

        let input_ment = make_input(
            AgeCurveArchetype::Mental,
            age_days,
            60,
            85,
            Intensity::Moderate,
            100,
        );
        let input_phys = make_input(
            AgeCurveArchetype::Physical,
            age_days,
            60,
            85,
            Intensity::Moderate,
            100,
        );

        let gain_ment = compute_growth(&input_ment, &mut r1);
        let gain_phys = compute_growth(&input_phys, &mut r2);

        // Physical is zero at 28 (too old), mental is at peak.
        assert_eq!(gain_phys, Fixed::ZERO, "physical growth must be zero at 28");
        assert!(gain_ment > Fixed::ZERO, "mental must still grow at 28");
    }

    // Detect breakthrough fires when crossing a milestone.
    #[test]
    fn breakthrough_fires_on_milestone_cross() {
        let before = Fixed::from_int(59);
        let after = Fixed::from_int(60);
        let event = detect_event(before, after, Fixed::from_int(80), Intensity::Moderate);
        assert_eq!(event, Some(TrainingEvent::Breakthrough));
    }

    // No breakthrough when not crossing a milestone.
    #[test]
    fn no_breakthrough_below_milestone() {
        let before = Fixed::from_int(61);
        let after = Fixed::raw(62_100);
        let event = detect_event(before, after, Fixed::from_int(80), Intensity::Moderate);
        assert_eq!(event, None);
    }

    // Overtrained fires when energy is low and intensity is Hard.
    #[test]
    fn overtrained_fires_at_low_energy_hard() {
        let event = detect_event(
            Fixed::from_int(70),
            Fixed::from_int(70),
            Fixed::from_int(15), // below OVERTRAINED_ENERGY_THRESHOLD (20)
            Intensity::Hard,
        );
        assert_eq!(event, Some(TrainingEvent::Overtrained));
    }

    // Overtrained does NOT fire at Moderate intensity even with low energy.
    #[test]
    fn no_overtrained_at_moderate_intensity() {
        let event = detect_event(
            Fixed::from_int(70),
            Fixed::from_int(70),
            Fixed::from_int(10),
            Intensity::Moderate,
        );
        assert_eq!(event, None);
    }
}
