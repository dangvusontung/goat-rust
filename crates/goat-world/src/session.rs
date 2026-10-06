//! Retained light-world replay with incremental deep-orbit observations.
//! Renderer-independent; the journal is authoritative, the cache is disposable.
use crate::{population::Population, promotion::ReplayCache, world::WorldGenesis};
use goat_core::{history::NpcMatchLoad, state::OrbitMatchRecord};
use std::collections::BTreeMap;

struct RetainedWorld {
    seed: u64,
    calendar: Option<goat_core::chronology::Chronology>,
    season: u32,
    world: WorldGenesis,
    replay: ReplayCache,
    records: Vec<OrbitMatchRecord>,
    loads: Vec<NpcMatchLoad>,
    deep_results: Vec<goat_core::deep::DeepFixtureResult>,
    // Undo the in-progress credits before the season pipeline applies them itself.
    originals: BTreeMap<usize, (u32, u32, i16)>,
}

/// One bounded population/world cache, not a cache of every queried date.
/// Legacy scheduled, dated and scored deep APIs explicitly choose their replay inputs.
#[derive(Default)]
pub struct SimulationSession {
    retained: Option<RetainedWorld>,
    rebuilds: u32,
    pub(crate) deep_progress: Option<crate::deep::DeepProgress>,
}
impl SimulationSession {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        self.retained = None;
        self.deep_progress = None;
    }
    /// Diagnostic count; never used to decide simulation outcomes.
    pub fn rebuild_count(&self) -> u32 {
        self.rebuilds
    }
    /// Synchronize canonical journals, then lazily query only the desired players.
    /// Appends in the current season reuse the population. Changed prefixes, a new
    /// seed, backwards time or late inputs to completed seasons rebuild safely.
    pub fn population(
        &mut self,
        seed: u64,
        season: u32,
        records: &[OrbitMatchRecord],
        loads: &[NpcMatchLoad],
    ) -> &Population {
        self.population_model(seed, None, season, records, loads, None)
    }
    pub fn population_dated(
        &mut self,
        seed: u64,
        base_year: u32,
        season: u32,
        records: &[OrbitMatchRecord],
        loads: &[NpcMatchLoad],
    ) -> &Population {
        self.population_model(
            seed,
            Some(goat_core::chronology::Chronology::new(base_year)),
            season,
            records,
            loads,
            None,
        )
    }
    /// V10 tiered replay: detailed scorelines are authoritative season inputs.
    pub fn population_deep(&mut self, state: &goat_core::state::WorldState) -> &Population {
        self.population_model(
            state.world_seed,
            Some(goat_core::chronology::Chronology::new(
                state.career_base_year,
            )),
            state.season_number,
            &state.orbit_records,
            &state.npc_match_loads,
            Some(&state.deep_results),
        )
    }
    pub(crate) fn deep_world(&self) -> Option<WorldGenesis> {
        self.retained.as_ref().map(|r| {
            let mut world = r.world.clone();
            for (league, clubs) in world.leagues.iter_mut().zip(r.replay.membership()) {
                league.clubs = clubs.clone();
            }
            world
        })
    }

    /// Close the scored season once through the same pipeline used by cold replay.
    /// Adapters use this membership instead of running a separate promotion model.
    pub fn finish_deep_season(
        &mut self,
        state: &goat_core::state::WorldState,
    ) -> Option<crate::deep::DeepSeasonClose> {
        let calendar = state.chronology()?;
        if state.season_number == 0
            || state.pc_epoch_day < calendar.frame(state.season_number).next_preparation_start
        {
            return None;
        }
        self.population_deep(state);
        let old = self.retained.as_ref().unwrap().replay.membership().to_vec();
        self.population_model(
            state.world_seed,
            Some(goat_core::chronology::Chronology::new(
                state.career_base_year,
            )),
            state.season_number + 1,
            &state.orbit_records,
            &state.npc_match_loads,
            Some(&state.deep_results),
        );
        let r = self.retained.as_ref().unwrap();
        let membership = r.replay.membership().to_vec();
        let mut from = vec![0; r.world.clubs.len()];
        for (league, clubs) in old.iter().enumerate() {
            for &club in clubs {
                from[club] = league;
            }
        }
        let mut events = Vec::new();
        for (league, clubs) in membership.iter().enumerate() {
            for &club in clubs {
                if from[club] != league {
                    events.push(crate::promotion::PromoRelegationEvent {
                        club,
                        season: state.season_number,
                        from_league: from[club],
                        to_league: league,
                        transition: if (r.world.leagues[league].tier as usize)
                            < r.world.leagues[from[club]].tier as usize
                        {
                            crate::promotion::TransitionType::DirectPromotion
                        } else {
                            crate::promotion::TransitionType::DirectRelegation
                        },
                    });
                }
            }
        }
        events.sort_by_key(|e| (e.from_league, e.to_league, e.club));
        Some(crate::deep::DeepSeasonClose { membership, events })
    }
    #[allow(clippy::too_many_arguments)]
    fn population_model(
        &mut self,
        seed: u64,
        calendar: Option<goat_core::chronology::Chronology>,
        season: u32,
        records: &[OrbitMatchRecord],
        loads: &[NpcMatchLoad],
        deep: Option<&[goat_core::deep::DeepFixtureResult]>,
    ) -> &Population {
        let season = season.max(1);
        let unchanged = self.retained.as_ref().is_some_and(|r| {
            r.seed == seed
                && r.calendar == calendar
                && r.season == season
                && r.records == records
                && r.loads == loads
                && r.deep_results == deep.unwrap_or(&[])
        });
        if unchanged {
            return self.retained.as_ref().unwrap().replay.pop();
        }
        let reset = self.retained.as_ref().is_none_or(|r| {
            if r.seed != seed || r.calendar != calendar || season < r.season {
                return true;
            }
            if let Some(scores) = deep {
                let old_loads = r
                    .loads
                    .iter()
                    .map(|l| ((l.pop_idx, l.fixture_id), l))
                    .collect::<BTreeMap<_, _>>();
                let old_records = r
                    .records
                    .iter()
                    .map(|v| ((v.season, v.round, v.div), v))
                    .collect::<BTreeMap<_, _>>();
                !journals_extend(&r.records, records, &r.loads, loads)
                    || !scores.starts_with(&r.deep_results)
                    || scores[r.deep_results.len().min(scores.len())..]
                        .iter()
                        .any(|v| v.season < r.season)
                    || records.iter().any(|v| {
                        v.season < r.season
                            && old_records
                                .get(&(v.season, v.round, v.div))
                                .is_none_or(|old| **old != *v)
                    })
                    || loads.iter().any(|v| {
                        v.epoch_day < calendar.unwrap().frame(r.season).preparation_start
                            && old_loads
                                .get(&(v.pop_idx, v.fixture_id))
                                .is_none_or(|old| **old != *v)
                    })
            } else {
                !records.starts_with(&r.records)
                    || !loads.starts_with(&r.loads)
                    || records[r.records.len()..]
                        .iter()
                        .any(|v| v.season < r.season)
                    || loads[r.loads.len()..].iter().any(|v| {
                        v.epoch_day
                            < r.calendar.map_or((r.season - 1) * 364, |c| {
                                c.frame(r.season).preparation_start
                            })
                    })
            }
        });
        if reset {
            let world = WorldGenesis::generate(seed);
            let replay = calendar.map_or_else(
                || ReplayCache::new_scheduled(&world, seed),
                |c| ReplayCache::new_dated(&world, seed, c.base_year),
            );
            self.retained = Some(RetainedWorld {
                seed,
                calendar,
                season: 1,
                world,
                replay,
                records: Vec::new(),
                loads: Vec::new(),
                deep_results: Vec::new(),
                originals: BTreeMap::new(),
            });
            self.rebuilds += 1;
        }
        let r = self.retained.as_mut().unwrap();
        let old_loads = r
            .loads
            .iter()
            .map(|l| ((l.pop_idx, l.fixture_id), *l))
            .collect::<BTreeMap<_, _>>();
        for load in loads
            .iter()
            .filter(|l| !old_loads.contains_key(&(l.pop_idx, l.fixture_id)))
        {
            r.replay.record_match_load(*load);
        }
        let advanced = season > r.season;
        if advanced {
            for (&idx, &(apps, goals, form)) in &r.originals {
                let pop = r.replay.population_mut();
                pop.career_apps[idx] = apps;
                pop.career_goals[idx] = goals;
                pop.form[idx] = form;
            }
            r.originals.clear();
            while r.season < season {
                // Newly created youth can now accept previously pending observations.
                for load in loads {
                    r.replay.record_match_load(*load);
                }
                r.replay
                    .advance_one_season_with_deep(&mut r.world, records, deep.unwrap_or(&[]));
                r.season += 1;
            }
            for load in loads {
                r.replay.record_match_load(*load);
            }
        }
        let previous = r
            .records
            .iter()
            .map(|rec| ((rec.season, rec.round, rec.div), rec))
            .collect::<BTreeMap<_, _>>();
        for (index, record) in records
            .iter()
            .enumerate()
            .filter(|(_, v)| v.season == season)
        {
            if deep.is_none() && !advanced && index < r.records.len() {
                continue;
            }
            let old = if advanced || deep.is_none() {
                None
            } else {
                previous
                    .get(&(record.season, record.round, record.div))
                    .copied()
            };
            let credits = record
                .credits
                .iter()
                .filter(|credit| {
                    old.is_none_or(|rec| !rec.credits.iter().any(|c| c.pop_idx == credit.pop_idx))
                })
                .cloned()
                .collect::<Vec<_>>();
            let pop = r.replay.population_mut();
            for credit in &credits {
                let idx = credit.pop_idx as usize;
                if idx < pop.len() {
                    r.originals.entry(idx).or_insert((
                        pop.career_apps[idx],
                        pop.career_goals[idx],
                        pop.form[idx],
                    ));
                }
            }
            crate::orbit::apply_orbit_record(
                pop,
                &OrbitMatchRecord {
                    credits,
                    ..record.clone()
                },
            );
        }
        r.records = records.to_vec();
        r.loads = loads.to_vec();
        r.deep_results = deep.unwrap_or(&[]).to_vec();
        r.replay.pop()
    }
}

fn journals_extend(
    old_records: &[OrbitMatchRecord],
    records: &[OrbitMatchRecord],
    old_loads: &[NpcMatchLoad],
    loads: &[NpcMatchLoad],
) -> bool {
    let map = loads
        .iter()
        .map(|l| ((l.pop_idx, l.fixture_id), l))
        .collect::<BTreeMap<_, _>>();
    let recs = records
        .iter()
        .map(|r| ((r.season, r.round, r.div), r))
        .collect::<BTreeMap<_, _>>();
    old_loads.iter().all(|l| {
        map.get(&(l.pop_idx, l.fixture_id))
            .is_some_and(|v| **v == *l)
    }) && old_records.iter().all(|r| {
        recs.get(&(r.season, r.round, r.div)).is_some_and(|new| {
            let credits = new
                .credits
                .iter()
                .map(|c| (c.pop_idx, c))
                .collect::<BTreeMap<_, _>>();
            r.credits
                .iter()
                .all(|c| credits.get(&c.pop_idx).is_some_and(|v| **v == *c))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use goat_core::state::NpcMatchCredit;

    fn record(season: u32, round: u32) -> OrbitMatchRecord {
        OrbitMatchRecord {
            season,
            round,
            div: 0,
            credits: vec![NpcMatchCredit {
                pop_idx: 0,
                goals: 1,
                assists: 0,
                result: 1,
            }],
        }
    }
    fn load(seed: u64) -> NpcMatchLoad {
        NpcMatchLoad {
            competition_id: 1,
            pop_idx: 0,
            fixture_id: crate::workload::league_fixture_id(seed, 1, 0, 0),
            epoch_day: 54,
            minutes: 30,
        }
    }
    fn compare(
        session: &mut SimulationSession,
        seed: u64,
        season: u32,
        records: &[OrbitMatchRecord],
        loads: &[NpcMatchLoad],
    ) {
        let cached = session.population(seed, season, records, loads);
        let fresh = crate::orbit::rebuild_population_scheduled(seed, season, records, loads);
        assert_eq!(cached.fingerprint(), fresh.fingerprint());
        assert_eq!(cached.career_fingerprint(), fresh.career_fingerprint());
        assert_eq!(cached.form, fresh.form);
        let week = (season - 1) * 52 + 9;
        for idx in [0, 7, cached.len() - 1] {
            assert_eq!(
                cached.observed_match_loads(idx),
                fresh.observed_match_loads(idx)
            );
            assert_eq!(cached.current_ovr(idx, week), fresh.current_ovr(idx, week));
            assert_eq!(
                cached.is_available(idx, week),
                fresh.is_available(idx, week)
            );
            assert_eq!(
                cached.training_history(idx, 0, week),
                fresh.training_history(idx, 0, week)
            );
            assert_eq!(cached.exposure_history(idx), fresh.exposure_history(idx));
        }
    }
    #[test]
    fn append_and_season_boundary_equal_fresh_without_double_credits() {
        let mut session = SimulationSession::new();
        compare(&mut session, 42, 1, &[], &[]);
        let mut records = vec![record(1, 0)];
        let loads = [load(42)];
        compare(&mut session, 42, 1, &records, &loads);
        compare(&mut session, 42, 1, &records, &loads);
        records.push(record(1, 1));
        compare(&mut session, 42, 1, &records, &loads);
        compare(&mut session, 42, 2, &records, &loads);
        records.push(record(2, 0));
        compare(&mut session, 42, 2, &records, &loads);
        compare(&mut session, 42, 3, &records, &loads);
        assert_eq!(session.rebuild_count(), 1);
    }
    #[test]
    fn rewritten_journals_backwards_time_and_seed_change_rebuild() {
        let mut session = SimulationSession::new();
        let mut loads = vec![load(42)];
        let mut records = vec![record(1, 0)];
        compare(&mut session, 42, 1, &records, &loads);
        loads[0].epoch_day += 28;
        loads[0].minutes = 0;
        compare(&mut session, 42, 1, &records, &loads);
        records[0].credits[0].goals = 2;
        compare(&mut session, 42, 2, &records, &loads);
        // A late result from an already completed season must rerun that season.
        records.push(record(1, 1));
        compare(&mut session, 42, 2, &records, &loads);
        compare(&mut session, 42, 1, &[], &[]);
        compare(&mut session, 43, 1, &[], &[]);
        assert_eq!(session.rebuild_count(), 6);
        session.clear();
        compare(&mut session, 43, 1, &[], &[]);
        assert_eq!(session.rebuild_count(), 7);
    }
    #[test]
    fn youth_observation_pending_until_intake_and_read_order_are_stable() {
        let mut session = SimulationSession::new();
        let youth = session.population(42, 1, &[], &[]).len();
        let loads = [NpcMatchLoad {
            pop_idx: youth as u32,
            epoch_day: 418,
            fixture_id: crate::workload::league_fixture_id(42, 2, 0, 0),
            ..load(42)
        }];
        compare(&mut session, 42, 1, &[], &loads);
        compare(&mut session, 42, 2, &[], &loads);
        let pop = session.population(42, 2, &[], &loads);
        assert!(pop.len() > youth);
        assert_eq!(pop.observed_match_loads(youth), loads);
        let later = pop.current_ovr(youth, 70);
        pop.current_ovr(youth, 53);
        assert_eq!(pop.current_ovr(youth, 70), later);
        compare(&mut session, 42, 2, &[], &loads);
        assert_eq!(session.rebuild_count(), 1);
    }
}

#[cfg(test)]
mod dated_tests {
    use super::*;
    #[test]
    fn dated_session_season_change_birthdays_and_year_key_equal_fresh() {
        let seed = 42;
        let c = goat_core::chronology::Chronology::new(2023);
        let first = crate::calendar::dated_fixture_day(c, 1, 0);
        let loads = [NpcMatchLoad {
            competition_id: 1,
            pop_idx: 0,
            fixture_id: crate::workload::league_fixture_id(seed, 1, 0, 0),
            epoch_day: first,
            minutes: 30,
        }];
        let records = [OrbitMatchRecord {
            season: 1,
            round: 0,
            div: 0,
            credits: vec![goat_core::state::NpcMatchCredit {
                pop_idx: 0,
                goals: 1,
                assists: 0,
                result: 1,
            }],
        }];
        let mut session = SimulationSession::new();
        session
            .population_dated(seed, 2023, 1, &records, &loads)
            .current_ovr(0, 8);
        let retained = session.population_dated(seed, 2023, 2, &records, &loads);
        let fresh = crate::orbit::rebuild_population_dated(seed, 2023, 2, &records, &loads);
        assert_eq!(retained.fingerprint(), fresh.fingerprint());
        assert_eq!(retained.career_fingerprint(), fresh.career_fingerprint());
        assert_eq!(retained.form, fresh.form);
        for idx in [0, retained.len() - 1] {
            let week = (c.frame(2).start_day / 7) + 2;
            assert_eq!(
                retained.training_history(idx, 0, week),
                fresh.training_history(idx, 0, week)
            );
            assert_eq!(
                retained.current_ovr(idx, week),
                fresh.current_ovr(idx, week)
            );
            assert_eq!(
                retained
                    .promote(idx, week, "NPC", &WorldGenesis::generate(seed))
                    .unwrap()
                    .age_weeks,
                fresh
                    .promote(idx, week, "NPC", &WorldGenesis::generate(seed))
                    .unwrap()
                    .age_weeks
            );
        }
        let youth = retained.len() - 1;
        let first_row = retained.training_history(youth, 0, retained.intake_week[youth] + 1);
        assert_eq!(first_row[0].epoch_day, c.frame(2).preparation_start);
        assert_eq!(session.rebuild_count(), 1);
        let changed = session.population_dated(seed, 2025, 2, &records, &[]);
        let fresh = crate::orbit::rebuild_population_dated(seed, 2025, 2, &records, &[]);
        assert_eq!(changed.fingerprint(), fresh.fingerprint());
        assert_eq!(changed.career_fingerprint(), fresh.career_fingerprint());
        assert_eq!(session.rebuild_count(), 2);
    }
}
