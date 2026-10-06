//! Compact dated NPC inputs. Health is an expectation, never a fabricated diagnosis.
use goat_fixed::Fixed;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NpcExposure {
    pub start_week: u32,
    /// Previous season league appearances; proxy only, not invented match minutes.
    pub workload_apps: u16,
    pub facilities: Fixed,
    /// Estimated weekly post-workload energy in PC units (0..100).
    pub energy: Fixed,
    pub intensity: u8,
    pub focus_share: Fixed,
    pub lifestyle: u8,
    pub injury_duration_pct: i32,
}

impl NpcExposure {
    pub fn balanced(start_week: u32, facilities: Fixed) -> Self {
        Self {
            start_week,
            workload_apps: 0,
            facilities,
            energy: Fixed::from_int(75),
            intensity: 1,
            focus_share: Fixed::raw(700),
            lifestyle: 1,
            injury_duration_pct: 1000,
        }
    }
    pub(crate) fn valid(self) -> bool {
        self.energy >= Fixed::ZERO
            && self.energy <= Fixed::from_int(100)
            && self.facilities >= Fixed::ZERO
            && self.facilities <= Fixed::from_int(10)
            && self.focus_share >= Fixed::ZERO
            && self.focus_share <= Fixed::ONE
            && self.intensity <= 2
            && self.lifestyle <= 2
            && (100..=3000).contains(&self.injury_duration_pct)
    }
}

/// Long-run expectation for a background NPC, not a sampled injury diagnosis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExpectedHealth {
    pub injury_per_1000: u32,
    pub mean_recovery_weeks: Fixed,
    pub healthy_share: Fixed,
}

pub(crate) fn expected_health(e: NpcExposure, age: u32, durability: u8) -> ExpectedHealth {
    use goat_core::{
        tuning::{ENERGY_AUTO_DOWNGRADE, INJURY_WEEKS_MAX, INJURY_WEEKS_MIN},
        week::{injury_prob, Intensity},
    };
    let intensity = if e.energy < ENERGY_AUTO_DOWNGRADE {
        Intensity::Low
    } else {
        match e.intensity {
            0 => Intensity::Low,
            2 => Intensity::High,
            _ => Intensity::Medium,
        }
    };
    let risk = injury_prob(e.energy, intensity, age, e.lifestyle, durability).min(1000);
    // Average the same individually rounded duration outcomes as PC, including physio.
    let duration_sum: i32 = (INJURY_WEEKS_MIN..=INJURY_WEEKS_MAX)
        .map(|weeks| (weeks as i32 * e.injury_duration_pct / 1000).max(1))
        .sum();
    let duration_raw = duration_sum * 1000 / (INJURY_WEEKS_MAX - INJURY_WEEKS_MIN + 1) as i32;
    // Onset skips development too. Stationary availability = (1-p)/(1+p*d).
    let healthy =
        ((1000 - risk) as u64 * 1_000_000 / (1_000_000 + risk as u64 * duration_raw as u64)) as i32;
    ExpectedHealth {
        injury_per_1000: risk,
        mean_recovery_weeks: Fixed::raw(duration_raw),
        healthy_share: Fixed::raw(healthy),
    }
}

/// Linked SoA segments: one head per NPC, allocation only when inputs change.
#[derive(Clone, Debug, Default)]
pub(crate) struct ExposureColumns {
    pub heads: Vec<Option<usize>>,
    starts: Vec<u32>,
    workload: Vec<u16>,
    facilities: Vec<Fixed>,
    energy: Vec<Fixed>,
    intensity: Vec<u8>,
    focus: Vec<Fixed>,
    lifestyle: Vec<u8>,
    duration: Vec<i32>,
    previous: Vec<Option<usize>>,
}
impl ExposureColumns {
    pub fn push(&mut self, player: usize, e: NpcExposure) {
        let previous = self.heads[player];
        self.heads[player] = Some(self.starts.len());
        self.starts.push(e.start_week);
        self.workload.push(e.workload_apps);
        self.facilities.push(e.facilities);
        self.energy.push(e.energy);
        self.intensity.push(e.intensity);
        self.focus.push(e.focus_share);
        self.lifestyle.push(e.lifestyle);
        self.duration.push(e.injury_duration_pct);
        self.previous.push(previous);
    }
    pub fn history(&self, player: usize) -> Vec<NpcExposure> {
        let mut out = Vec::new();
        let mut head = self.heads[player];
        while let Some(i) = head {
            out.push(NpcExposure {
                start_week: self.starts[i],
                workload_apps: self.workload[i],
                facilities: self.facilities[i],
                energy: self.energy[i],
                intensity: self.intensity[i],
                focus_share: self.focus[i],
                lifestyle: self.lifestyle[i],
                injury_duration_pct: self.duration[i],
            });
            head = self.previous[i];
        }
        out.reverse();
        out
    }
    pub fn len(&self) -> usize {
        self.starts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use goat_core::tuning::{INJURY_WEEKS_MAX, INJURY_WEEKS_MIN};
    use goat_rng::{GoatRng, RngSource};

    #[test]
    fn expected_availability_matches_seeded_health_reference() {
        for duration_pct in [700, 1000, 1500] {
            let mut exposure = NpcExposure::balanced(0, Fixed::ONE);
            exposure.injury_duration_pct = duration_pct;
            let expected = expected_health(exposure, 25, 10);
            let mut rng = GoatRng::new(42);
            let mut remaining = 0;
            let mut healthy_weeks = 0;
            for _ in 0..200_000 {
                if remaining > 0 {
                    remaining -= 1;
                } else if rng.next_range_u32(0, 999) < expected.injury_per_1000 {
                    remaining = (rng.next_range_u8(INJURY_WEEKS_MIN, INJURY_WEEKS_MAX) as i32
                        * duration_pct
                        / 1000)
                        .max(1);
                } else {
                    healthy_weeks += 1;
                }
            }
            let observed_raw = healthy_weeks * 1000 / 200_000;
            assert!((observed_raw - expected.healthy_share.to_raw()).abs() <= 10);
        }
    }
}

crate::checkpoint::fields!(ExposureColumns {
    heads,
    starts,
    workload,
    facilities,
    energy,
    intensity,
    focus,
    lifestyle,
    duration,
    previous
});

impl ExposureColumns {
    pub(crate) fn checkpoint_valid(&self, players: usize) -> bool {
        let n = self.starts.len();
        if self.heads.len() != players
            || [
                self.workload.len(),
                self.facilities.len(),
                self.energy.len(),
                self.intensity.len(),
                self.focus.len(),
                self.lifestyle.len(),
                self.duration.len(),
                self.previous.len(),
            ]
            .iter()
            .any(|&len| len != n)
        {
            return false;
        }
        for i in 0..n {
            if self.previous[i].is_some_and(|p| p >= i || self.starts[p] > self.starts[i]) {
                return false;
            }
            if !(NpcExposure {
                start_week: self.starts[i],
                workload_apps: self.workload[i],
                facilities: self.facilities[i],
                energy: self.energy[i],
                intensity: self.intensity[i],
                focus_share: self.focus[i],
                lifestyle: self.lifestyle[i],
                injury_duration_pct: self.duration[i],
            })
            .valid()
            {
                return false;
            }
        }
        self.heads.iter().all(|h| h.is_none_or(|i| i < n))
    }
}
