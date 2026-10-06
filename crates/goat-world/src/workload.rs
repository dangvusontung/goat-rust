//! Dated league workload. Observed fixture minutes override the generated plan.
use crate::{
    calendar::{week_day_offset, week_to_rounds, WEEK_MATCH_COUNTS},
    exposure::NpcExposure,
};
use goat_core::{calendar_loop::LEAGUE_COMPETITION_ID, history::NpcMatchLoad};
use goat_rng::{GoatRng, RngSource};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchDose {
    pub competition_id: u32,
    pub fixture_id: u64,
    pub epoch_day: u32,
    pub minutes: u16,
    pub observed: bool,
}

pub fn league_fixture_id(seed: u64, season: u32, round: usize, slot: usize) -> u64 {
    GoatRng::new(
        seed ^ ((LEAGUE_COMPETITION_ID as u64) << 48)
            ^ ((season as u64) << 24)
            ^ ((round as u64) << 8)
            ^ slot as u64,
    )
    .next_u64()
}

pub(crate) struct Schedule {
    observed: Vec<NpcMatchLoad>,
    ids: Vec<u64>,
}
impl Schedule {
    pub fn new(mut observed: Vec<NpcMatchLoad>) -> Self {
        observed.sort_by_key(|l| (l.epoch_day, l.fixture_id));
        let mut ids: Vec<u64> = observed.iter().map(|l| l.fixture_id).collect();
        ids.sort_unstable();
        Self { observed, ids }
    }
    pub fn week(&self, seed: u64, world_seed: u64, week: u32, e: NpcExposure) -> Vec<MatchDose> {
        let local_week = week as usize % 52;
        let count = WEEK_MATCH_COUNTS.get(local_week).copied().unwrap_or(0);
        let mut out = Vec::with_capacity(2);
        if count > 0 {
            let season = week / 52 + 1;
            for (slot, round) in week_to_rounds(local_week).enumerate() {
                let fixture_id = league_fixture_id(world_seed, season, round, slot);
                if self.ids.binary_search(&fixture_id).is_ok() {
                    continue;
                }
                let mut rng = GoatRng::new(seed ^ fixture_id ^ 0x4E50_434D_494E_5554);
                let regular = e.workload_apps >= 20 || (e.workload_apps == 0 && seed % 25 < 11);
                let minutes = if rng.next_range_u32(0, 99) < if regular { 79 } else { 21 } {
                    if regular {
                        rng.next_range_u32(75, 90) as u16
                    } else {
                        rng.next_range_u32(10, 30) as u16
                    }
                } else {
                    0
                };
                out.push(MatchDose {
                    competition_id: LEAGUE_COMPETITION_ID,
                    fixture_id,
                    epoch_day: (season - 1) * 364 + week_day_offset(local_week, slot),
                    minutes,
                    observed: false,
                });
            }
        }
        let start = self.observed.partition_point(|l| l.epoch_day < week * 7);
        let end = self
            .observed
            .partition_point(|l| l.epoch_day < (week + 1) * 7);
        out.extend(self.observed[start..end].iter().map(|l| MatchDose {
            competition_id: l.competition_id,
            fixture_id: l.fixture_id,
            epoch_day: l.epoch_day,
            minutes: l.minutes,
            observed: true,
        }));
        out.sort_by_key(|l| (l.epoch_day, l.fixture_id));
        out
    }
    pub fn week_with_plan(
        &self,
        seed: u64,
        week: u32,
        e: NpcExposure,
        plan: &[MatchDose],
    ) -> Vec<MatchDose> {
        let mut out = Vec::with_capacity(2);
        for template in plan {
            if self.ids.binary_search(&template.fixture_id).is_ok() {
                continue;
            }
            let mut dose = *template;
            let mut rng = GoatRng::new(seed ^ dose.fixture_id ^ 0x4E50_434D_494E_5554);
            let regular = e.workload_apps >= 20 || (e.workload_apps == 0 && seed % 25 < 11);
            dose.minutes = if rng.next_range_u32(0, 99) < if regular { 79 } else { 21 } {
                if regular {
                    rng.next_range_u32(75, 90) as u16
                } else {
                    rng.next_range_u32(10, 30) as u16
                }
            } else {
                0
            };
            out.push(dose);
        }
        let start = self.observed.partition_point(|l| l.epoch_day < week * 7);
        let end = self
            .observed
            .partition_point(|l| l.epoch_day < (week + 1) * 7);
        out.extend(self.observed[start..end].iter().map(|l| MatchDose {
            competition_id: l.competition_id,
            fixture_id: l.fixture_id,
            epoch_day: l.epoch_day,
            minutes: l.minutes,
            observed: true,
        }));
        out.sort_by_key(|l| (l.epoch_day, l.fixture_id));
        out
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct LoadColumns {
    pub heads: Vec<Option<usize>>,
    ids: Vec<u64>,
    days: Vec<u32>,
    minutes: Vec<u16>,
    competitions: Vec<u32>,
    previous: Vec<Option<usize>>,
}
impl LoadColumns {
    pub fn history(&self, idx: usize) -> Vec<NpcMatchLoad> {
        let mut out = Vec::new();
        let mut head = self.heads[idx];
        while let Some(i) = head {
            out.push(NpcMatchLoad {
                competition_id: self.competitions[i],
                pop_idx: idx as u32,
                fixture_id: self.ids[i],
                epoch_day: self.days[i],
                minutes: self.minutes[i],
            });
            head = self.previous[i];
        }
        out.sort_by_key(|l| (l.epoch_day, l.fixture_id));
        out
    }
    pub fn push(&mut self, l: NpcMatchLoad) {
        let idx = l.pop_idx as usize;
        self.previous.push(self.heads[idx]);
        self.heads[idx] = Some(self.ids.len());
        self.ids.push(l.fixture_id);
        self.competitions.push(l.competition_id);
        self.days.push(l.epoch_day);
        self.minutes.push(l.minutes);
    }
}
