//! Shared development primitives; background projection uses expected exposure,
//! while the PC weekly path samples variance and individual injuries.
use crate::attrs::AgeCurveArchetype;
use crate::tuning::{BASE_DECAY_PER_WEEK, BASE_GROWTH_PER_WEEK, GROWTH_SINGLE_WEEK_CAP};
use crate::week::{attr_decay_rate, attr_growth_rate};
use goat_fixed::Fixed;

/// The same pre-variance weekly growth used by the canonical PC update.
pub fn weekly_growth(
    archetype: AgeCurveArchetype,
    age: u32,
    intensity: Fixed,
    energy: Fixed,
    facilities: Fixed,
) -> Fixed {
    BASE_GROWTH_PER_WEEK * attr_growth_rate(archetype, age) * intensity * energy * facilities
}

/// Age-related decline is independent of whether the attribute was focused.
pub fn weekly_decay(archetype: AgeCurveArchetype, age: u32, lifestyle: Fixed) -> Fixed {
    BASE_DECAY_PER_WEEK * attr_decay_rate(archetype, age) * lifestyle
}

/// AI training plan in explicit units, reusable by headless player projections.
/// Expected exposure is an approximation, not a fabricated injury history.
#[derive(Clone, Copy, Debug)]
pub struct DevelopmentPlan {
    pub intensity: Fixed,
    pub energy: Fixed,
    pub facilities: Fixed,
    pub focus_share: Fixed,
    pub healthy_share: Fixed,
    pub ceiling: Fixed,
    pub decline: Fixed,
}

/// Aggregate an attribute from age 16, splitting exactly at annual rate changes.
/// Constant-plan intervals avoid daily stepping without multiplying across age bands.
pub fn project_attribute(
    start: Fixed,
    potential: Fixed,
    archetype: AgeCurveArchetype,
    age_weeks: u32,
    plan: DevelopmentPlan,
) -> Fixed {
    project_attribute_interval(start, potential, archetype, 16 * 52, age_weeks, plan)
}

/// Apply only this age interval; existing growth is never recalculated with a new plan.
pub fn project_attribute_interval(
    start: Fixed,
    potential: Fixed,
    archetype: AgeCurveArchetype,
    start_age_weeks: u32,
    end_age_weeks: u32,
    plan: DevelopmentPlan,
) -> Fixed {
    let age_weeks = end_age_weeks;
    let mut current = start;
    let mut week = start_age_weeks;
    let ceiling = (potential * plan.ceiling).max(Fixed::MIN_ATTR);
    while week < age_weeks {
        let age = week / 52;
        let end = age_weeks.min((age + 1) * 52);
        let growth = (weekly_growth(archetype, age, plan.intensity, plan.energy, plan.facilities)
            * plan.focus_share
            * plan.healthy_share)
            .clamp(Fixed::ZERO, GROWTH_SINGLE_WEEK_CAP);
        let decay = weekly_decay(archetype, age, plan.decline);
        // Reproduce the weekly order: growth clamps before decay. If net growth
        // is positive the last weekly decay remains below the growth ceiling.
        let delta = growth - decay;
        // Apply the first step exactly, including an initially excessive ceiling.
        current = ((current + growth).min(ceiling) - decay).max(Fixed::MIN_ATTR);
        let weeks = Fixed::from_int((end - week - 1) as i32);
        current = if delta >= Fixed::ZERO {
            (current + delta * weeks).min((ceiling - decay).max(Fixed::MIN_ATTR))
        } else {
            (current + delta * weeks).max(Fixed::MIN_ATTR)
        };
        week = end;
    }
    current.clamp(Fixed::MIN_ATTR, potential)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plan() -> DevelopmentPlan {
        DevelopmentPlan {
            intensity: Fixed::ONE,
            energy: Fixed::ONE,
            facilities: Fixed::ONE,
            focus_share: Fixed::ONE,
            healthy_share: Fixed::ONE,
            ceiling: Fixed::ONE,
            decline: Fixed::ONE,
        }
    }
    #[test]
    fn initial_value_above_training_ceiling_matches_weekly_clamping() {
        let p = DevelopmentPlan {
            ceiling: Fixed::raw(800),
            ..plan()
        };
        let start = Fixed::from_int(89);
        let potential = Fixed::from_int(90);
        let mut current = start;
        for week in 16 * 52..17 * 52 {
            let growth = weekly_growth(
                AgeCurveArchetype::Physical,
                week / 52,
                Fixed::ONE,
                Fixed::ONE,
                Fixed::ONE,
            )
            .clamp(Fixed::ZERO, GROWTH_SINGLE_WEEK_CAP);
            current = ((current + growth).min(potential * p.ceiling)
                - weekly_decay(AgeCurveArchetype::Physical, week / 52, Fixed::ONE))
            .max(Fixed::MIN_ATTR);
            assert_eq!(
                project_attribute(start, potential, AgeCurveArchetype::Physical, week + 1, p),
                current
            );
        }
    }

    #[test]
    fn batching_equals_weekly_law_across_age_boundaries() {
        for kind in [
            AgeCurveArchetype::Physical,
            AgeCurveArchetype::Technical,
            AgeCurveArchetype::Mental,
        ] {
            let mut current = Fixed::from_int(50);
            let potential = Fixed::from_int(90);
            for week in 16 * 52..40 * 52 {
                let growth = weekly_growth(kind, week / 52, Fixed::ONE, Fixed::ONE, Fixed::ONE)
                    .clamp(Fixed::ZERO, GROWTH_SINGLE_WEEK_CAP);
                current = (current + growth).min(potential);
                current =
                    (current - weekly_decay(kind, week / 52, Fixed::ONE)).max(Fixed::MIN_ATTR);
                if week % 52 == 51 {
                    assert_eq!(
                        project_attribute(Fixed::from_int(50), potential, kind, week + 1, plan()),
                        current
                    );
                }
            }
        }
    }
    #[test]
    fn exposure_and_facilities_change_development_without_exceeding_talent() {
        let base = plan();
        let project = |p| {
            project_attribute(
                Fixed::from_int(40),
                Fixed::from_int(90),
                AgeCurveArchetype::Technical,
                18 * 52,
                p,
            )
        };
        assert!(
            project(base)
                > project(DevelopmentPlan {
                    healthy_share: Fixed::raw(500),
                    ..base
                })
        );
        assert!(
            project(DevelopmentPlan {
                facilities: Fixed::raw(1500),
                ..base
            }) > project(base)
        );
        assert!(project(base) <= Fixed::from_int(90));
    }
}
