//! Individual NPC weekly state. Dates and independent RNG streams make replay order-safe.
use crate::exposure::NpcExposure;
use goat_core::{
    attrs::{AttrId, ATTR_ARCHETYPES, NUM_ATTRS},
    development::{weekly_decay, weekly_growth},
    tuning::*,
    week::{energy_growth_factor, injury_prob, Intensity},
};
use goat_fixed::Fixed;
use goat_rng::{GoatRng, RngSource};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NpcHealthState {
    pub energy: Fixed,
    pub injury_weeks: u32,
    pub(crate) available_mask: u64,
    pub(crate) elapsed_weeks: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NpcTrainingWeek {
    pub week: u32,
    pub focus_mask: u32,
    pub effective_intensity: u8,
    pub energy_before: Fixed,
    pub energy_after: Fixed,
    pub injury_before: u32,
    pub injury_after: u32,
    pub new_injury: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NpcInjuryEpisode {
    pub onset_week: u32,
    pub expected_recovery_week: u32,
    pub recovered_week: Option<u32>,
    pub duration_weeks: u32,
}

/// Three position-specific drills and up to two rotating general drills, no duplicates.
pub fn focus_mask(position: u8, week: u32, share: Fixed) -> u32 {
    use AttrId::*;
    let preferred = match position {
        0 => [StandingTackle, Marking, Interceptions],
        1 => [ShortPassing, Vision, BallControl],
        _ => [Finishing, AttPositioning, Composure],
    };
    let count = ((share.to_raw().max(0) as u32 * NUM_ATTRS as u32).div_ceil(1000)).min(5);
    let mut mask = 0u32;
    for a in preferred.into_iter().take(count as usize) {
        mask |= 1 << a as u32;
    }
    let mut cursor = week as usize % NUM_ATTRS;
    while mask.count_ones() < count {
        mask |= 1 << cursor;
        cursor = (cursor + 1) % NUM_ATTRS;
    }
    mask
}

/// Shared PC energy, injury, ceilings and decay. Match load is a seasonal proxy,
/// not invented minutes. RNG is keyed by individual and absolute week.
#[allow(clippy::too_many_arguments)] // numeric identity context plus mutable simulation state
pub(crate) fn tick(
    seed: u64,
    week: u32,
    age: u32,
    position: u8,
    durability: u8,
    e: NpcExposure,
    health: &mut NpcHealthState,
    attrs: Option<(&mut [Fixed; NUM_ATTRS], &[Fixed; NUM_ATTRS])>,
) -> NpcTrainingWeek {
    let before = *health;
    let mut record = NpcTrainingWeek {
        week,
        focus_mask: 0,
        effective_intensity: 3,
        energy_before: before.energy,
        energy_after: before.energy,
        injury_before: before.injury_weeks,
        injury_after: before.injury_weeks,
        new_injury: false,
    };
    if health.injury_weeks > 0 {
        health.injury_weeks -= 1;
        health.energy = (health.energy + ENERGY_RECOVERY_INJURED).min(ENERGY_MAX);
    } else {
        // 20 energy per appearance is a first calibration assumption, not real minutes.
        let load = Fixed::raw(e.workload_apps as i32 * 20_000 / 52);
        let energy = (health.energy - load).max(Fixed::ZERO);
        let intensity = if energy < ENERGY_AUTO_DOWNGRADE {
            0
        } else {
            e.intensity
        };
        let (kind, cost, growth, ceiling) = match intensity {
            0 => (
                Intensity::Low,
                ENERGY_COST_LOW,
                GROWTH_MULT_LOW,
                INTENSITY_CEILING_LOW,
            ),
            2 => (
                Intensity::High,
                ENERGY_COST_HIGH,
                GROWTH_MULT_HIGH,
                INTENSITY_CEILING_HIGH,
            ),
            _ => (
                Intensity::Medium,
                ENERGY_COST_MED,
                GROWTH_MULT_MED,
                INTENSITY_CEILING_MED,
            ),
        };
        health.energy = (energy - cost + ENERGY_PASSIVE_RECOVERY).clamp(Fixed::ZERO, ENERGY_MAX);
        let mut rng = GoatRng::new(
            seed ^ 0x4E50_4348_4541_4C54 ^ (week as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15),
        );
        if rng.next_range_u32(0, 999)
            < injury_prob(health.energy, kind, age, e.lifestyle, durability)
        {
            let duration = rng.next_range_u8(INJURY_WEEKS_MIN, INJURY_WEEKS_MAX) as i32;
            health.injury_weeks = (duration * e.injury_duration_pct / 1000).max(1) as u32;
            record.new_injury = true;
        } else {
            record.effective_intensity = intensity;
            record.focus_mask = focus_mask(position, week, e.focus_share);
            if let Some((current, potential)) = attrs {
                let lifestyle_ceiling = match e.lifestyle {
                    0 => LIFESTYLE_CEILING_PRO,
                    2 => LIFESTYLE_CEILING_FLASHY,
                    _ => LIFESTYLE_CEILING_BALANCED,
                };
                // Commitment ceiling uses requested intensity, even on exhausted weeks.
                let ceiling = if intensity == e.intensity {
                    ceiling
                } else {
                    match e.intensity {
                        0 => INTENSITY_CEILING_LOW,
                        2 => INTENSITY_CEILING_HIGH,
                        _ => INTENSITY_CEILING_MED,
                    }
                };
                let decline = match e.lifestyle {
                    0 => DECLINE_LIFESTYLE_PRO,
                    2 => DECLINE_LIFESTYLE_FLASHY,
                    _ => DECLINE_LIFESTYLE_BALANCED,
                };
                let mut training_rng = GoatRng::new(
                    seed ^ 0x4E50_4354_5241_494E
                        ^ (week as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F),
                );
                for a in 0..NUM_ATTRS {
                    if record.focus_mask & (1 << a) != 0
                        && goat_core::week::attr_growth_rate(ATTR_ARCHETYPES[a], age) > Fixed::ZERO
                    {
                        let variance =
                            training_rng.next_range_u32(0, (GROWTH_VARIANCE_RAW * 2) as u32) as i32
                                - GROWTH_VARIANCE_RAW;
                        let delta = (weekly_growth(
                            ATTR_ARCHETYPES[a],
                            age,
                            growth,
                            energy_growth_factor(health.energy),
                            e.facilities
                                * match e.lifestyle {
                                    0 => Fixed::raw(1100),
                                    2 => Fixed::raw(900),
                                    _ => Fixed::ONE,
                                },
                        ) + Fixed::raw(variance))
                        .clamp(Fixed::ZERO, GROWTH_SINGLE_WEEK_CAP);
                        current[a] = (current[a] + delta).clamp(
                            Fixed::MIN_ATTR,
                            (potential[a] * ceiling * lifestyle_ceiling).max(Fixed::MIN_ATTR),
                        );
                    }
                    current[a] = (current[a] - weekly_decay(ATTR_ARCHETYPES[a], age, decline))
                        .max(Fixed::MIN_ATTR);
                }
            }
        }
    }
    health.available_mask = ((health.available_mask << 1)
        | u64::from(before.injury_weeks == 0 && health.injury_weeks == 0))
        & ((1u64 << 52) - 1);
    health.elapsed_weeks += 1;
    record.energy_after = health.energy;
    record.injury_after = health.injury_weeks;
    record
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn five_position_drills_are_unique_and_rotate() {
        for pos in 0..3 {
            for week in 0..52 {
                assert_eq!(focus_mask(pos, week, Fixed::raw(700)).count_ones(), 5);
            }
            assert_ne!(
                focus_mask(pos, 0, Fixed::raw(700)),
                focus_mask(pos, 15, Fixed::raw(700))
            );
        }
        assert_eq!(focus_mask(0, 0, Fixed::ZERO), 0);
    }
    #[test]
    fn injured_week_recovers_without_training_or_decay() {
        let mut state = NpcHealthState {
            energy: Fixed::from_int(20),
            injury_weeks: 2,
            available_mask: 0,
            elapsed_weeks: 0,
        };
        let mut attrs = [Fixed::from_int(50); NUM_ATTRS];
        let potential = [Fixed::from_int(99); NUM_ATTRS];
        let w = tick(
            42,
            10,
            35,
            0,
            10,
            NpcExposure::balanced(0, Fixed::ONE),
            &mut state,
            Some((&mut attrs, &potential)),
        );
        assert_eq!(attrs, [Fixed::from_int(50); NUM_ATTRS]);
        assert_eq!(w.focus_mask, 0);
        assert_eq!(state.injury_weeks, 1);
        assert_eq!(state.energy, Fixed::from_int(38));
    }
    #[test]
    fn healthy_growth_and_energy_match_pc_week_law() {
        use goat_core::{
            player::{PlayerStore, PlayerView},
            week::{advance_week, Routine},
        };
        struct ReferenceRng(GoatRng);
        impl RngSource for ReferenceRng {
            fn next_u64(&mut self) -> u64 {
                self.0.next_u64()
            }
            fn next_range_u64(&mut self, lo: u64, hi: u64) -> u64 {
                if lo == 0 && hi == (GROWTH_VARIANCE_RAW * 2) as u64 {
                    self.0.next_range_u64(lo, hi)
                } else {
                    hi
                } // prevent injuries/breakthroughs in this conditional reference
            }
        }
        let seed = (1..1000)
            .find(|&seed| {
                let mut h = NpcHealthState {
                    energy: Fixed::from_int(75),
                    injury_weeks: 0,
                    available_mask: 0,
                    elapsed_weeks: 0,
                };
                !tick(
                    seed,
                    10,
                    25,
                    1,
                    10,
                    NpcExposure::balanced(0, Fixed::ONE),
                    &mut h,
                    None,
                )
                .new_injury
            })
            .unwrap();
        let mut state = NpcHealthState {
            energy: Fixed::from_int(75),
            injury_weeks: 0,
            available_mask: 0,
            elapsed_weeks: 0,
        };
        let mut attrs = [Fixed::from_int(50); NUM_ATTRS];
        let potential = [Fixed::from_int(99); NUM_ATTRS];
        let e = NpcExposure::balanced(0, Fixed::ONE);
        let w = tick(
            seed,
            10,
            25,
            1,
            10,
            e,
            &mut state,
            Some((&mut attrs, &potential)),
        );
        let mut store = PlayerStore::new();
        let id = store.push(PlayerView {
            current: [Fixed::from_int(50); NUM_ATTRS],
            potential,
            age_weeks: 25 * 52,
            energy: Fixed::from_int(75),
            ..PlayerView::default()
        });
        let routine = Routine {
            intensity: Intensity::Medium,
            focus_attrs: AttrId::ALL
                .into_iter()
                .filter(|a| w.focus_mask & (1 << *a as u32) != 0)
                .collect(),
        };
        let mut rng = ReferenceRng(GoatRng::new(
            seed ^ 0x4E50_4354_5241_494E ^ 10u64.wrapping_mul(0xC2B2_AE3D_27D4_EB4F),
        ));
        advance_week(&mut store, id, &routine, Fixed::ONE, 1, 1000, &mut rng);
        assert_eq!(attrs, store.snapshot(id).current);
        assert_eq!(state.energy, store.get_energy(id));
    }
}
