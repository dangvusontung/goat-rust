//! Derived medical readout. Returning is advisory, not a new availability rule.
use goat_fixed::Fixed;

pub const RETURN_WINDOW_WEEKS: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryPhase {
    Healthy,
    Injured { remaining_weeks: u32 },
    Returning { weeks_since_return: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MedicalStatus {
    pub energy: Fixed,
    pub last_return_week: Option<u32>,
    pub phase: RecoveryPhase,
}

impl MedicalStatus {
    pub fn at_week(week: u32, energy: Fixed, injury_weeks: u32, last_return: Option<u32>) -> Self {
        let last_return_week = last_return.filter(|&returned| returned <= week);
        let phase = if injury_weeks > 0 {
            RecoveryPhase::Injured {
                remaining_weeks: injury_weeks,
            }
        } else if let Some(returned) = last_return_week.filter(|&r| week - r < RETURN_WINDOW_WEEKS)
        {
            RecoveryPhase::Returning {
                weeks_since_return: week - returned,
            }
        } else {
            RecoveryPhase::Healthy
        };
        Self {
            energy,
            last_return_week,
            phase,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn return_window_uses_observed_dates_and_new_injury_takes_priority() {
        let status = |week, remaining, returned| {
            MedicalStatus::at_week(week, Fixed::from_int(65), remaining, returned)
        };
        assert_eq!(status(8, 0, Some(9)).phase, RecoveryPhase::Healthy);
        assert_eq!(
            status(9, 0, Some(9)).phase,
            RecoveryPhase::Returning {
                weeks_since_return: 0
            }
        );
        assert_eq!(
            status(10, 0, Some(9)).phase,
            RecoveryPhase::Returning {
                weeks_since_return: 1
            }
        );
        assert_eq!(status(11, 0, Some(9)).phase, RecoveryPhase::Healthy);
        assert_eq!(
            status(10, 4, Some(9)).phase,
            RecoveryPhase::Injured { remaining_weeks: 4 }
        );
    }
}
