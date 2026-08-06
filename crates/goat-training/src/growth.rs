//! Growth + energy math (docs/TRAINING.md §Core Loop, bible §5.1/§5.4).
//!
//! Everything here is pure fixed-point over named `tuning` constants. The
//! ceiling clamp at application sites uses goat-core's existing idiom —
//! `(cur + growth).clamp(Fixed::MIN_ATTR, potential)` (week.rs) — never a
//! re-implemented, looser one (§2.4 is sacred).

use crate::models::{DevelopmentInput, Intensity};
use crate::tuning::*;
use goat_core::attrs::AgeCurveArchetype;
use goat_fixed::Fixed;
use goat_rng::{GoatRng, RngSource};

/// Growth multiplier for an intensity tier.
pub fn intensity_factor(intensity: Intensity) -> Fixed {
    match intensity {
        Intensity::Light => Fixed::raw(INTENSITY_FACTOR_LIGHT_X1000),
        Intensity::Moderate => Fixed::raw(INTENSITY_FACTOR_MODERATE_X1000),
        Intensity::Hard => Fixed::raw(INTENSITY_FACTOR_HARD_X1000),
    }
}

/// Growth multiplier from current energy: a linear ramp from
/// `ENERGY_FACTOR_AT_ZERO_X1000` at empty to 1.0 at full (tired players gain
/// less, §5.4).
pub fn energy_factor(energy: Fixed) -> Fixed {
    let pct_x1000 = energy.to_raw() * 1_000 / ENERGY_MAX.to_raw(); // 0..1000
    Fixed::raw(ENERGY_FACTOR_AT_ZERO_X1000 + ENERGY_FACTOR_SPAN_X1000 * pct_x1000 / 1_000)
}

/// Trainability of an archetype at a given age (bible §5.1), looked up from
/// the per-archetype band tables. Three DISTINCT curves by design — Physical
/// fades early, Technical plateaus, Mental appreciates with experience.
pub fn trainability(archetype: AgeCurveArchetype, age_days: u32) -> Fixed {
    let age_years = age_days / DAYS_PER_YEAR;
    let (table, edges) = match archetype {
        AgeCurveArchetype::Physical => (&TRAINABILITY_PHYSICAL_X1000, &AGE_BAND_EDGES_PHYSICAL),
        AgeCurveArchetype::Technical => (&TRAINABILITY_TECHNICAL_X1000, &AGE_BAND_EDGES_TECHNICAL),
        AgeCurveArchetype::Mental => (&TRAINABILITY_MENTAL_X1000, &AGE_BAND_EDGES_MENTAL),
    };
    let band = edges.iter().filter(|&&edge| age_years > edge).count();
    Fixed::raw(table[band])
}

/// Headroom taper: full speed until HEADROOM_FULL_SPEED_POINTS from the
/// ceiling, then linear slowdown (growth eases into potential instead of
/// clipping hard against it).
fn headroom_factor(headroom: Fixed) -> Fixed {
    let full_raw = HEADROOM_FULL_SPEED_POINTS * 1_000;
    if headroom.to_raw() >= full_raw {
        Fixed::ONE
    } else {
        Fixed::raw(headroom.to_raw() / HEADROOM_FULL_SPEED_POINTS)
    }
}

/// The growth function (spec §Core Loop): gated by age-archetype trainability,
/// intensity, energy, and headroom; jittered deterministically from the
/// subsystem's own forked stream. Returns the delta for one training day.
///
/// The result is already capped so `current + delta <= potential` (headroom
/// clamp); application sites additionally clamp with goat-core's idiom.
pub fn compute_growth(input: &DevelopmentInput, rng: &mut GoatRng) -> Fixed {
    let headroom = input.potential - input.current;
    if headroom <= Fixed::ZERO {
        return Fixed::ZERO; // at/over the ceiling (§2.4) — never exceed potential
    }

    let base = BASE_GROWTH_PER_DAY
        * intensity_factor(input.intensity)
        * trainability(input.attr_archetype, input.age_days)
        * energy_factor(input.energy)
        * headroom_factor(headroom)
        * input.facility_mult;

    // Deterministic ±jitter (same idiom as the legacy week loop's variance).
    let jitter = rng.next_range_u64(0, (GROWTH_JITTER_RAW * 2) as u64) as i32 - GROWTH_JITTER_RAW;
    let raw = Fixed::raw(base.to_raw() + jitter);

    raw.clamp(Fixed::ZERO, GROWTH_DAY_CAP).min(headroom)
}

/// Energy after a training day at `intensity` (clamped to [0, ENERGY_MAX]).
pub fn spend_energy(energy: Fixed, intensity: Intensity) -> Fixed {
    let cost = match intensity {
        Intensity::Light => ENERGY_SPEND_LIGHT,
        Intensity::Moderate => ENERGY_SPEND_MODERATE,
        Intensity::Hard => ENERGY_SPEND_HARD,
    };
    (energy - cost).clamp(ENERGY_MIN, ENERGY_MAX)
}

/// Energy after a pure rest day (no training, no match).
pub fn recover_energy(energy: Fixed) -> Fixed {
    (energy + ENERGY_RECOVER_REST).clamp(ENERGY_MIN, ENERGY_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use goat_core::attrs::AttrId;

    fn input(archetype: AgeCurveArchetype, intensity: Intensity) -> DevelopmentInput {
        DevelopmentInput {
            attr_archetype: archetype,
            age_days: 17 * DAYS_PER_YEAR,
            current: Fixed::from_int(60),
            potential: Fixed::from_int(85),
            intensity,
            energy: ENERGY_MAX,
            facility_mult: Fixed::ONE,
        }
    }

    /// AC-01: no growth at (or past) the ceiling.
    #[test]
    fn growth_is_zero_at_the_ceiling() {
        let mut rng = GoatRng::new(1);
        let mut at_ceiling = input(AgeCurveArchetype::Technical, Intensity::Hard);
        at_ceiling.current = at_ceiling.potential;
        assert_eq!(compute_growth(&at_ceiling, &mut rng), Fixed::ZERO);
        at_ceiling.current = at_ceiling.potential + Fixed::from_int(3);
        assert_eq!(compute_growth(&at_ceiling, &mut rng), Fixed::ZERO);
    }

    /// AC-01: a near-ceiling day never overshoots potential.
    #[test]
    fn growth_never_exceeds_headroom() {
        let mut rng = GoatRng::new(7);
        for seed in 0..50u64 {
            let mut near = input(AgeCurveArchetype::Technical, Intensity::Hard);
            near.current = near.potential - Fixed::raw(10); // 0.010 of headroom
            let delta = compute_growth(&near, &mut GoatRng::new(seed));
            assert!(
                near.current + delta <= near.potential,
                "seed {seed}: current + delta must not pass potential"
            );
        }
        let _ = &mut rng;
    }

    /// Property (Step 2): growth is monotonic in intensity at fixed energy/age
    /// (same jitter seed per arm so the comparison is fair).
    #[test]
    fn growth_monotonic_in_intensity() {
        for seed in 0..20u64 {
            let light = compute_growth(
                &input(AgeCurveArchetype::Technical, Intensity::Light),
                &mut GoatRng::new(seed),
            );
            let moderate = compute_growth(
                &input(AgeCurveArchetype::Technical, Intensity::Moderate),
                &mut GoatRng::new(seed),
            );
            let hard = compute_growth(
                &input(AgeCurveArchetype::Technical, Intensity::Hard),
                &mut GoatRng::new(seed),
            );
            assert!(light <= moderate && moderate <= hard, "seed {seed}");
        }
    }

    /// AC-02: Technical out-gains Physical at a young age, same intensity.
    #[test]
    fn technical_outgains_physical_when_young() {
        for seed in 0..20u64 {
            let technical = compute_growth(
                &input(AgeCurveArchetype::Technical, Intensity::Moderate),
                &mut GoatRng::new(seed),
            );
            let physical = compute_growth(
                &input(AgeCurveArchetype::Physical, Intensity::Moderate),
                &mut GoatRng::new(seed),
            );
            assert!(
                technical > physical,
                "seed {seed}: technical {technical:?} vs physical {physical:?}"
            );
        }
    }

    /// AC-02 / §5.2: Mental appreciates with age — its trainability at 30 must
    /// beat its own teenage value, and it must still be growing when Physical
    /// has shut off entirely.
    #[test]
    fn mental_appreciates_with_age() {
        let young = trainability(AgeCurveArchetype::Mental, 17 * DAYS_PER_YEAR);
        let prime = trainability(AgeCurveArchetype::Mental, 30 * DAYS_PER_YEAR);
        assert!(prime > young, "mental trainability must rise into the 30s");
        let old_mental = trainability(AgeCurveArchetype::Mental, 38 * DAYS_PER_YEAR);
        let old_physical = trainability(AgeCurveArchetype::Physical, 30 * DAYS_PER_YEAR);
        assert_eq!(old_physical, Fixed::ZERO);
        assert!(old_mental > Fixed::ZERO, "mental still grows at 38");
    }

    /// AC-03: energy drains on training, recovers on rest, stays in [0, 100].
    #[test]
    fn energy_stays_in_bounds_through_any_sequence() {
        let mut e = ENERGY_MAX;
        for _ in 0..30 {
            e = spend_energy(e, Intensity::Hard);
            assert!((ENERGY_MIN..=ENERGY_MAX).contains(&e));
        }
        assert_eq!(e, ENERGY_MIN, "hard training bottoms out");
        for _ in 0..30 {
            e = recover_energy(e);
            assert!((ENERGY_MIN..=ENERGY_MAX).contains(&e));
        }
        assert_eq!(e, ENERGY_MAX, "rest tops back up");
    }

    /// AC-03: low energy reduces per-day gain versus the same training fresh.
    #[test]
    fn low_energy_reduces_growth() {
        for seed in 0..20u64 {
            let fresh = input(AgeCurveArchetype::Technical, Intensity::Moderate);
            let mut tired = fresh;
            tired.energy = Fixed::from_int(15);
            let fresh_growth = compute_growth(&fresh, &mut GoatRng::new(seed));
            let tired_growth = compute_growth(&tired, &mut GoatRng::new(seed));
            assert!(tired_growth < fresh_growth, "seed {seed}");
        }
    }

    /// The three curves are genuinely distinct shapes, not one scaled curve.
    #[test]
    fn archetype_curves_have_distinct_shapes() {
        let peak_physical = (16..=40u32)
            .map(|y| trainability(AgeCurveArchetype::Physical, y * DAYS_PER_YEAR))
            .max()
            .unwrap();
        let peak_technical = (16..=40u32)
            .map(|y| trainability(AgeCurveArchetype::Technical, y * DAYS_PER_YEAR))
            .max()
            .unwrap();
        let peak_mental = (16..=40u32)
            .map(|y| trainability(AgeCurveArchetype::Mental, y * DAYS_PER_YEAR))
            .max()
            .unwrap();
        assert!(peak_technical > peak_physical, "technical trains easiest");
        assert!(
            peak_mental >= peak_technical,
            "mental peaks highest but late"
        );
        // Peak TIMING differs: physical peaks before mental.
        let argmax = |a| {
            (16..=40u32)
                .max_by_key(|&y| trainability(a, y * DAYS_PER_YEAR))
                .unwrap()
        };
        assert!(argmax(AgeCurveArchetype::Physical) < argmax(AgeCurveArchetype::Mental));
    }

    /// Ceiling application uses goat-core's exact clamp idiom (§2.4): a sanity
    /// check that the idiom this crate's Step 3 will call behaves as expected.
    #[test]
    fn goat_core_clamp_idiom_holds() {
        let cur = Fixed::from_int(84);
        let pot = Fixed::from_int(85);
        let applied = (cur + Fixed::from_int(2)).clamp(Fixed::MIN_ATTR, pot);
        assert_eq!(applied, pot);
    }

    /// Quiet reference so AttrId is exercised in this module (used by callers).
    #[test]
    fn archetype_lookup_matches_goat_core_table() {
        let a = AttrId::Finishing as usize;
        assert_eq!(
            goat_core::attrs::ATTR_ARCHETYPES[a],
            AgeCurveArchetype::Technical
        );
    }
}
