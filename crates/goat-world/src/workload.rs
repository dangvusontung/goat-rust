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
    pub heads: Vec<Vec<u32>>,
    fixtures: Vec<(u64, u32, u32)>,
    fixture_index: std::collections::BTreeMap<(u64, u32, u32), u32>,
    rows: Vec<u32>,
    minutes: Vec<u16>,
}
impl LoadColumns {
    fn row(&self, idx: usize, row: u32) -> NpcMatchLoad {
        let i = row as usize;
        let (fixture_id, competition_id, epoch_day) = self.fixtures[self.rows[i] as usize];
        NpcMatchLoad {
            pop_idx: idx as u32,
            fixture_id,
            competition_id,
            epoch_day,
            minutes: self.minutes[i],
        }
    }
    pub fn find(&self, idx: usize, fixture_id: u64) -> Option<NpcMatchLoad> {
        self.heads[idx]
            .binary_search_by_key(&fixture_id, |&row| {
                self.fixtures[self.rows[row as usize] as usize].0
            })
            .ok()
            .map(|slot| self.row(idx, self.heads[idx][slot]))
    }
    pub fn history(&self, idx: usize) -> Vec<NpcMatchLoad> {
        let mut out: Vec<_> = self.heads[idx]
            .iter()
            .map(|&row| self.row(idx, row))
            .collect();
        out.sort_by_key(|l| (l.epoch_day, l.fixture_id));
        out
    }
    pub fn push(&mut self, load: NpcMatchLoad) {
        let key = (load.fixture_id, load.competition_id, load.epoch_day);
        let fixture = *self.fixture_index.entry(key).or_insert_with(|| {
            let index = u32::try_from(self.fixtures.len()).expect("fixture dictionary exceeds u32");
            self.fixtures.push(key);
            index
        });
        let idx = load.pop_idx as usize;
        let row = u32::try_from(self.rows.len()).expect("workload exceeds u32");
        let slot = self.heads[idx]
            .binary_search_by_key(&load.fixture_id, |&r| {
                self.fixtures[self.rows[r as usize] as usize].0
            })
            .unwrap_or_else(|i| i);
        self.heads[idx].insert(slot, row);
        self.rows.push(fixture);
        self.minutes.push(load.minutes);
    }
}

crate::checkpoint::fields!(LoadColumns {
    heads,
    fixtures,
    fixture_index,
    rows,
    minutes
});
crate::checkpoint::fields!(MatchDose {
    competition_id,
    fixture_id,
    epoch_day,
    minutes,
    observed
});

impl LoadColumns {
    pub(crate) fn checkpoint_valid(&self, players: usize) -> bool {
        if self.heads.len() != players
            || self.rows.len() != self.minutes.len()
            || self.fixture_index.len() != self.fixtures.len()
            || self.minutes.iter().any(|&m| m > 120)
            || self.rows.iter().any(|&r| r as usize >= self.fixtures.len())
        {
            return false;
        }
        for (i, &key) in self.fixtures.iter().enumerate() {
            if key.1 == 0 || self.fixture_index.get(&key) != Some(&(i as u32)) {
                return false;
            }
        }
        let mut seen = vec![false; self.rows.len()];
        for head in &self.heads {
            let mut last = None;
            for &row in head {
                let idx = row as usize;
                if idx >= seen.len() || seen[idx] {
                    return false;
                }
                seen[idx] = true;
                let id = self.fixtures[self.rows[idx] as usize].0;
                if last.is_some_and(|old| id <= old) {
                    return false;
                }
                last = Some(id);
            }
        }
        seen.into_iter().all(|v| v)
    }
}
