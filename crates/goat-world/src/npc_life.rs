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
    pub(crate) last_match_day: Option<u32>,
    pub(crate) season_apps: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NpcTrainingWeek {
    pub epoch_day: u32,
    pub week: u32,
    pub focus_mask: u32,
    pub effective_intensity: u8,
    pub energy_before: Fixed,
    pub energy_after: Fixed,
    pub injury_before: u32,
    pub injury_after: u32,
    pub new_injury: bool,
    pub minutes_played: u16,
    pub match_count: u8,
    pub league_match_count: u8,
    pub min_rest_days: Option<u32>,
    pub injury_risk_per_1000: u32,
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
    tick_internal(
        seed, week, age, position, durability, e, health, attrs, None, true, None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn tick_scheduled(
    seed: u64,
    week: u32,
    age: u32,
    position: u8,
    durability: u8,
    e: NpcExposure,
    health: &mut NpcHealthState,
    attrs: Option<(&mut [Fixed; NUM_ATTRS], &[Fixed; NUM_ATTRS])>,
    doses: &[crate::workload::MatchDose],
) -> NpcTrainingWeek {
    tick_internal(
        seed,
        week,
        age,
        position,
        durability,
        e,
        health,
        attrs,
        Some(doses),
        true,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn tick_dated(
    seed: u64,
    week: u32,
    age: u32,
    position: u8,
    durability: u8,
    e: NpcExposure,
    health: &mut NpcHealthState,
    attrs: Option<(&mut [Fixed; NUM_ATTRS], &[Fixed; NUM_ATTRS])>,
    doses: &[crate::workload::MatchDose],
    life_days: (u32, u32),
) -> NpcTrainingWeek {
    tick_internal(
        seed,
        week,
        age,
        position,
        durability,
        e,
        health,
        attrs,
        Some(doses),
        false,
        Some(life_days),
    )
}

#[allow(clippy::too_many_arguments)]
fn tick_internal(
    seed: u64,
    week: u32,
    age: u32,
    position: u8,
    durability: u8,
    e: NpcExposure,
    health: &mut NpcHealthState,
    attrs: Option<(&mut [Fixed; NUM_ATTRS], &[Fixed; NUM_ATTRS])>,
    doses: Option<&[crate::workload::MatchDose]>,
    reset_52: bool,
    life_days: Option<(u32, u32)>,
) -> NpcTrainingWeek {
    let start_day = life_days.map_or(week * 7, |(entry, _)| entry.max(week * 7));
    let end_day = life_days.map_or((week + 1) * 7, |(_, end)| end.min((week + 1) * 7));
    let active_days = end_day.saturating_sub(start_day);
    let fraction = Fixed::raw(active_days as i32 * 1000 / 7);
    let before = *health;
    let mut record = NpcTrainingWeek {
        week,
        epoch_day: start_day,
        focus_mask: 0,
        effective_intensity: 3,
        energy_before: before.energy,
        energy_after: before.energy,
        injury_before: before.injury_weeks,
        injury_after: before.injury_weeks,
        new_injury: false,
        minutes_played: 0,
        match_count: 0,
        league_match_count: 0,
        min_rest_days: None,
        injury_risk_per_1000: 0,
    };
    if health.injury_weeks > 0 {
        health.injury_weeks -= 1;
        health.energy = (health.energy + ENERGY_RECOVERY_INJURED * fraction).min(ENERGY_MAX);
        if let Some(doses) = doses {
            for dose in doses.iter().filter(|d| {
                d.observed && d.minutes > 0 && d.epoch_day >= start_day && d.epoch_day < end_day
            }) {
                record.minutes_played += dose.minutes;
                record.match_count += 1;
                if dose.competition_id == goat_core::calendar_loop::LEAGUE_COMPETITION_ID {
                    record.league_match_count += 1;
                }
                health.energy = (health.energy - Fixed::raw(dose.minutes as i32 * 20_000 / 90))
                    .max(Fixed::ZERO);
                health.last_match_day = Some(dose.epoch_day);
            }
        }
    } else {
        let energy = if let Some(doses) = doses {
            let mut energy = health.energy;
            for day in start_day..end_day {
                let mut played = false;
                for dose in doses.iter().filter(|d| d.epoch_day == day && d.minutes > 0) {
                    if let Some(last) = health.last_match_day {
                        let rest = day.saturating_sub(last);
                        record.min_rest_days =
                            Some(record.min_rest_days.map_or(rest, |r| r.min(rest)));
                    }
                    health.last_match_day = Some(day);
                    record.minutes_played += dose.minutes;
                    record.match_count += 1;
                    if dose.competition_id == goat_core::calendar_loop::LEAGUE_COMPETITION_ID {
                        record.league_match_count += 1;
                    }
                    energy =
                        (energy - Fixed::raw(dose.minutes as i32 * 20_000 / 90)).max(Fixed::ZERO);
                    played = true;
                }
                if !played {
                    energy = (energy + Fixed::from_int(6)).min(ENERGY_MAX);
                }
            }
            energy
        } else {
            let load = Fixed::raw(e.workload_apps as i32 * 20_000 / 52);
            (health.energy - load).max(Fixed::ZERO)
        };
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
        health.energy = (energy - cost * fraction
            + if doses.is_some() {
                Fixed::ZERO
            } else {
                ENERGY_PASSIVE_RECOVERY
            })
        .clamp(Fixed::ZERO, ENERGY_MAX);
        let mut rng = GoatRng::new(
            seed ^ 0x4E50_4348_4541_4C54 ^ (week as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15),
        );
        let base_risk = injury_prob(health.energy, kind, age, e.lifestyle, durability);
        let pressure = if doses.is_some() {
            1000 + record.minutes_played.saturating_sub(90) as u32 * 500 / 90
                + if record.min_rest_days.is_some_and(|r| r < 3) {
                    500
                } else {
                    0
                }
        } else {
            1000
        };
        record.injury_risk_per_1000 = (base_risk * pressure / 1000 * active_days / 7).min(999);
        if rng.next_range_u32(0, 999) < record.injury_risk_per_1000 {
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
                        current[a] = (current[a] + delta * fraction).clamp(
                            Fixed::MIN_ATTR,
                            (potential[a] * ceiling * lifestyle_ceiling).max(Fixed::MIN_ATTR),
                        );
                    }
                    current[a] = (current[a]
                        - weekly_decay(ATTR_ARCHETYPES[a], age, decline) * fraction)
                        .max(Fixed::MIN_ATTR);
                }
            }
        }
    }
    if doses.is_some() {
        if reset_52 && week.is_multiple_of(52) {
            health.season_apps = 0;
        }
        health.season_apps += record.league_match_count as u32;
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
            last_match_day: None,
            season_apps: 0,
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
                    last_match_day: None,
                    season_apps: 0,
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
            last_match_day: None,
            season_apps: 0,
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

#[cfg(test)]
mod workload_tests {
    use super::*;
    use crate::workload::MatchDose;
    fn health() -> NpcHealthState {
        NpcHealthState {
            energy: Fixed::from_int(75),
            injury_weeks: 0,
            available_mask: 0,
            elapsed_weeks: 0,
            last_match_day: None,
            season_apps: 0,
        }
    }
    fn dose(day: u32, minutes: u16) -> MatchDose {
        MatchDose {
            competition_id: 1,
            fixture_id: day as u64 + 1,
            epoch_day: day,
            minutes,
            observed: true,
        }
    }
    #[test]
    fn minutes_and_short_rest_raise_fatigue_and_risk_without_offseason_phantom_load() {
        let e = NpcExposure::balanced(0, Fixed::ONE);
        let run = |doses: &[MatchDose]| {
            let mut h = health();
            tick_scheduled(42, 0, 25, 1, 10, e, &mut h, None, doses)
        };
        let rested = run(&[]);
        let short = run(&[dose(5, 30)]);
        let full = run(&[dose(5, 90)]);
        let spread = run(&[dose(1, 90), dose(5, 90)]);
        let tight = run(&[dose(4, 90), dose(5, 90)]);
        assert!(rested.energy_after > short.energy_after);
        assert!(short.energy_after > full.energy_after);
        assert!(full.energy_after > spread.energy_after);
        assert_eq!(spread.energy_after, tight.energy_after);
        assert!(tight.injury_risk_per_1000 > spread.injury_risk_per_1000);
        assert_eq!(tight.min_rest_days, Some(1));
        assert_eq!(rested.minutes_played, 0);
        assert_eq!(spread.minutes_played, 180);
    }
    #[test]
    fn cup_minutes_affect_health_but_do_not_create_league_appearances() {
        let mut h = health();
        let mut cup = dose(5, 90);
        cup.competition_id = 2;
        let r = tick_scheduled(
            42,
            0,
            25,
            1,
            10,
            NpcExposure::balanced(0, Fixed::ONE),
            &mut h,
            None,
            &[cup],
        );
        assert_eq!(r.match_count, 1);
        assert_eq!(r.league_match_count, 0);
        assert_eq!(h.season_apps, 0);
    }
    #[test]
    fn unavailable_players_skip_plans_but_observed_minutes_are_authoritative() {
        let mut h = health();
        h.injury_weeks = 2;
        let mut planned = dose(5, 90);
        planned.observed = false;
        let e = NpcExposure::balanced(0, Fixed::ONE);
        let r = tick_scheduled(42, 0, 25, 1, 10, e, &mut h, None, &[planned]);
        assert_eq!(r.minutes_played, 0);
        assert_eq!(h.injury_weeks, 1);
        let r = tick_scheduled(42, 1, 25, 1, 10, e, &mut h, None, &[dose(12, 20)]);
        assert_eq!(r.minutes_played, 20);
    }
}
