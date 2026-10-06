//! Retained light-world replay with incremental deep-orbit observations.
//! Renderer-independent; the journal is authoritative, the cache is disposable.
use crate::{population::Population, promotion::ReplayCache, world::WorldGenesis};
use goat_core::{history::NpcMatchLoad, state::OrbitMatchRecord};
use std::collections::BTreeMap;

struct RetainedWorld {
    seed: u64,
    ranked: bool,
    calendar: Option<goat_core::chronology::Chronology>,
    season: u32,
    world: WorldGenesis,
    replay: ReplayCache,
    past_loads: Vec<u8>,
    past_records: Vec<u8>,
    records: Vec<OrbitMatchRecord>,
    loads: Vec<NpcMatchLoad>,
    deep_results: Vec<goat_core::deep::DeepFixtureResult>,
    // Undo the in-progress credits before the season pipeline applies them itself.
    originals: BTreeMap<usize, (u32, u32, i16)>,
}

impl RetainedWorld {
    fn active_start(&self) -> u32 {
        self.calendar.unwrap().frame(self.season).preparation_start
    }
    fn past_matches(&self, records: &[OrbitMatchRecord], loads: &[NpcMatchLoad]) -> bool {
        goat_core::journal::matches_loads(
            &self.past_loads,
            loads.iter().filter(|v| v.epoch_day < self.active_start()),
        ) && goat_core::journal::matches_records(
            &self.past_records,
            records.iter().filter(|v| v.season < self.season),
        )
    }
}

/// One bounded population/world cache, not a cache of every queried date.
/// Legacy scheduled, dated and scored deep APIs explicitly choose their replay inputs.
#[derive(Default)]
pub struct SimulationSession {
    #[cfg(feature = "capacity-bench")]
    capacity: Option<crate::capacity::CapacityProfile>,
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
        self.population_model(seed, None, season, records, loads, None, None)
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
            state.competition_calendar.as_ref(),
        );
        if state.competition_calendar.is_some() {
            let r = self.retained.as_mut().unwrap();
            let end = state
                .chronology()
                .unwrap()
                .frame(state.season_number)
                .end_day;
            r.replay.advance_market(
                &mut r.world,
                state.season_number,
                state.pc_epoch_day.min(end),
                state.competition_calendar.as_ref(),
            );
            r.replay.population_mut().apply_fixture_dates(
                state.competition_calendar.as_ref().unwrap(),
                state.season_number,
            );
        }
        if let Some(cal) = &state.competition_calendar {
            self.retained
                .as_mut()
                .unwrap()
                .replay
                .override_coefficients(&cal.coefficients);
        }
        self.retained.as_ref().unwrap().replay.pop()
    }
    /// Season-opening scores use completed history only, independent of menu reads.
    pub fn league_scores(
        &mut self,
        state: &goat_core::state::WorldState,
    ) -> Vec<crate::deep::LeagueScore> {
        self.population_deep(state);
        let r = self.retained.as_ref().unwrap();
        r.replay.league_scores(&r.world)
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
            state.competition_calendar.as_ref(),
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
        competitions: Option<&goat_core::competitions::CompetitionCalendar>,
    ) -> &Population {
        let season = season.max(1);
        if let (Some(r), Some(cal)) = (self.retained.as_mut(), competitions) {
            r.replay.population_mut().apply_fixture_dates(cal, r.season);
        }

        let unchanged = self.retained.as_ref().is_some_and(|r| {
            r.seed == seed
                && r.ranked == deep.is_some()
                && r.replay.pop().dated_rosters
                    == competitions.is_some_and(|cal| cal.market_enabled)
                && r.calendar == calendar
                && r.season == season
                && if r.ranked {
                    r.past_matches(records, loads)
                        && r.records
                            .iter()
                            .eq(records.iter().filter(|v| v.season >= r.season))
                        && r.loads
                            .iter()
                            .eq(loads.iter().filter(|v| v.epoch_day >= r.active_start()))
                } else {
                    r.records == records && r.loads == loads
                }
                && r.deep_results == deep.unwrap_or(&[])
        });
        if unchanged {
            return self.retained.as_ref().unwrap().replay.pop();
        }
        let reset = self.retained.as_ref().is_none_or(|r| {
            if r.seed != seed
                || r.ranked != deep.is_some()
                || r.replay.pop().dated_rosters
                    != competitions.is_some_and(|cal| cal.market_enabled)
                || r.calendar != calendar
                || season < r.season
            {
                return true;
            }
            if let Some(scores) = deep {
                let incoming_records = records
                    .iter()
                    .filter(|v| v.season >= r.season)
                    .cloned()
                    .collect::<Vec<_>>();
                let incoming_loads = loads
                    .iter()
                    .filter(|v| v.epoch_day >= r.active_start())
                    .copied()
                    .collect::<Vec<_>>();
                !r.past_matches(records, loads)
                    || !journals_extend(&r.records, &incoming_records, &r.loads, &incoming_loads)
                    || !scores.starts_with(&r.deep_results)
                    || scores[r.deep_results.len().min(scores.len())..]
                        .iter()
                        .any(|v| v.season < r.season)
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
            let world = self.generated_world(seed);
            let mut replay = calendar.map_or_else(
                || ReplayCache::new_scheduled(&world, seed),
                |c| {
                    if deep.is_some() {
                        ReplayCache::new_ranked(&world, seed, c.base_year)
                    } else {
                        ReplayCache::new_dated(&world, seed, c.base_year)
                    }
                },
            );
            if competitions.is_some_and(|cal| cal.market_enabled) {
                replay.enable_dated_market();
            }
            self.retained = Some(RetainedWorld {
                seed,
                ranked: deep.is_some(),
                calendar,
                season: 1,
                world,
                replay,
                past_loads: Vec::new(),
                past_records: Vec::new(),
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
        let active_start = if r.ranked { r.active_start() } else { 0 };
        for load in loads.iter().filter(|l| {
            l.epoch_day >= active_start
                && !old_loads.contains_key(&(l.pop_idx, l.fixture_id))
                && (competitions.is_none()
                    || season == r.season
                    || l.epoch_day < calendar.unwrap().frame(r.season).next_preparation_start)
        }) {
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
                for load in loads.iter().filter(|l| {
                    competitions.is_none()
                        || l.epoch_day < calendar.unwrap().frame(r.season).next_preparation_start
                }) {
                    r.replay.record_match_load(*load);
                }
                if let Some(cal) = competitions {
                    r.replay.population_mut().apply_fixture_dates(cal, r.season);
                }
                if let Some(cal) = competitions {
                    r.replay.advance_one_season_with_dated(
                        &mut r.world,
                        records,
                        deep.unwrap_or(&[]),
                        cal,
                    );
                } else {
                    r.replay.advance_one_season_with_deep(
                        &mut r.world,
                        records,
                        deep.unwrap_or(&[]),
                    );
                }
                r.season += 1;
            }
            for load in loads {
                r.replay.record_match_load(*load);
            }
        }
        if let Some(cal) = competitions {
            let c = calendar.unwrap();
            let frame = c.frame(season);
            r.replay.advance_market(
                &mut r.world,
                season,
                cal.resolved_day
                    .max(frame.preparation_start)
                    .min(frame.end_day),
                Some(cal),
            );
            // July intake precedes any new-cohort credits and pending observations.
            for load in loads {
                r.replay.record_match_load(*load);
            }
            r.replay.population_mut().apply_fixture_dates(cal, season);
        }
        if records
            .iter()
            .any(|rec| rec.round & goat_core::competitions::EXTRA_ROUND != 0)
        {
            for (&idx, &(apps, goals, form)) in &r.originals {
                let pop = r.replay.population_mut();
                pop.career_apps[idx] = apps;
                pop.career_goals[idx] = goals;
                pop.form[idx] = form;
            }
            r.originals.clear();
            for record in records.iter().filter(|rec| rec.season == season) {
                for credit in &record.credits {
                    let idx = credit.pop_idx as usize;
                    let pop = r.replay.population_mut();
                    if idx < pop.len() {
                        r.originals.entry(idx).or_insert((
                            pop.career_apps[idx],
                            pop.career_goals[idx],
                            pop.form[idx],
                        ));
                    }
                }
            }
            crate::orbit::apply_dated_records(
                r.replay.population_mut(),
                seed,
                calendar.unwrap(),
                season,
                records,
                loads,
            );
        } else {
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
                        old.is_none_or(|rec| {
                            !rec.credits.iter().any(|c| c.pop_idx == credit.pop_idx)
                        })
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
        }
        if r.ranked {
            let start_day = r.active_start();
            if r.past_loads.is_empty() || advanced {
                r.past_loads.clear();
                goat_core::journal::write_iter(
                    &mut r.past_loads,
                    loads.iter().filter(|v| v.epoch_day < start_day),
                );
                r.past_records = goat_core::journal::pack_records(
                    records.iter().filter(|v| v.season < r.season),
                );
            }
            r.records = records
                .iter()
                .filter(|v| v.season >= r.season)
                .cloned()
                .collect();
            r.loads = loads
                .iter()
                .filter(|v| v.epoch_day >= start_day)
                .copied()
                .collect();
        } else {
            r.records = records.to_vec();
            r.loads = loads.to_vec();
        }
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

// Stored session inputs are bound exactly to the canonical journals, not just a digest.
crate::checkpoint::fields!(CheckpointBinding {
    model,
    seed,
    year,
    season,
    day,
    realistic,
    scopes,
    cards,
    scores,
    loads,
    records,
    competitions
});
struct CheckpointBinding {
    model: u32,
    seed: u64,
    year: u32,
    season: u32,
    day: u32,
    realistic: bool,
    scopes: Vec<goat_core::deep::DeepScope>,
    cards: Vec<goat_core::discipline::NpcCardEvent>,
    scores: Vec<goat_core::deep::DeepFixtureResult>,
    loads: Vec<u8>,
    records: Vec<u8>,
    competitions: Vec<u8>,
}
impl CheckpointBinding {
    fn matches(&self, state: &goat_core::state::WorldState) -> bool {
        self.model == crate::checkpoint::MODEL
            && self.seed == state.world_seed
            && self.year == state.career_base_year
            && self.season == state.season_number
            && self.day == state.pc_epoch_day
            && self.realistic == state.realistic_npc
            && self.competitions
                == state
                    .competition_calendar
                    .as_ref()
                    .map_or_else(Vec::new, crate::competitions::encode_calendar)
            && self.scopes == state.deep_scopes
            && self.cards == state.npc_cards
            && self.scores == state.deep_results
            && goat_core::journal::matches_loads(&self.loads, state.npc_match_loads.iter())
            && goat_core::journal::matches_records(&self.records, state.orbit_records.iter())
    }
}
impl SimulationSession {
    /// Materialize one exact local resume cache. First synchronize canonical inputs;
    /// a fresh session may need a one-time replay before its first checkpoint exists.
    pub fn resume_checkpoint(&mut self, state: &goat_core::state::WorldState) -> Option<Vec<u8>> {
        use crate::checkpoint::Snapshot;
        if !state.dated_calendar || state.season_number == 0 {
            return None;
        }
        self.population_deep(state);
        let retained = self.retained.as_ref()?;
        let mut loads = Vec::new();
        goat_core::journal::write(&mut loads, &state.npc_match_loads);
        let binding = CheckpointBinding {
            competitions: state
                .competition_calendar
                .as_ref()
                .map_or_else(Vec::new, crate::competitions::encode_calendar),
            model: crate::checkpoint::MODEL,
            seed: state.world_seed,
            year: state.career_base_year,
            season: state.season_number,
            day: state.pc_epoch_day,
            realistic: state.realistic_npc,
            scopes: state.deep_scopes.clone(),
            cards: state.npc_cards.clone(),
            scores: state.deep_results.clone(),
            loads,
            records: goat_core::journal::pack_records(state.orbit_records.iter()),
        };
        let mut out = Vec::new();
        crate::checkpoint::FORMAT.write(&mut out);
        binding.write(&mut out);
        retained.replay.write(&mut out);
        retained
            .world
            .clubs
            .iter()
            .map(|c| (c.budget, c.academy_boost, c.tactical_identity.clone()))
            .collect::<Vec<_>>()
            .write(&mut out);
        retained
            .world
            .leagues
            .iter()
            .map(|l| l.clubs.clone())
            .collect::<Vec<_>>()
            .write(&mut out);
        retained.past_loads.write(&mut out);
        retained.past_records.write(&mut out);
        retained.records.write(&mut out);
        retained.loads.write(&mut out);
        retained.deep_results.write(&mut out);
        retained.originals.write(&mut out);
        if out.len() > crate::checkpoint::MAX_BYTES - 8 {
            return None;
        }
        let sum = crate::checkpoint::checksum(&out);
        sum.write(&mut out);
        #[cfg(feature = "capacity-bench")]
        if let Some(profile) = self.capacity {
            let wrapped = profile.wrap_checkpoint(out);
            return (wrapped.len() <= crate::checkpoint::MAX_BYTES).then_some(wrapped);
        }
        Some(out)
    }
    /// Restore the optional cache only after integrity, structure and exact input checks.
    /// Failure leaves this session untouched; callers can use normal seed replay.
    pub fn restore_checkpoint(
        &mut self,
        state: &goat_core::state::WorldState,
        bytes: &[u8],
    ) -> bool {
        self.decode_checkpoint(state, bytes)
            .is_some_and(|retained| {
                self.retained = Some(retained);
                self.deep_progress = None;
                true
            })
    }
    fn decode_checkpoint(
        &self,
        state: &goat_core::state::WorldState,
        bytes: &[u8],
    ) -> Option<RetainedWorld> {
        use crate::checkpoint::{Reader, Snapshot};
        #[cfg(feature = "capacity-bench")]
        let bytes = if let Some(profile) = self.capacity {
            profile.checkpoint_body(bytes)?
        } else {
            bytes
        };
        if bytes.len() < 12
            || bytes.len() > crate::checkpoint::MAX_BYTES
            || !state.dated_calendar
            || state.season_number == 0
        {
            return None;
        }
        let body = &bytes[..bytes.len() - 8];
        let expected = u64::from_le_bytes(bytes[bytes.len() - 8..].try_into().ok()?);
        if crate::checkpoint::checksum(body) != expected {
            return None;
        }
        let mut r = Reader::new(body);
        if u32::read(&mut r)? != crate::checkpoint::FORMAT {
            return None;
        }
        let binding = CheckpointBinding::read(&mut r)?;
        if !binding.matches(state) {
            return None;
        }
        let replay = ReplayCache::read(&mut r)?;
        let mut world = self.generated_world(state.world_seed);
        let clubs = Vec::<(i64, u8, goat_core::tactical_identity::TacticalIdentity)>::read(&mut r)?;
        if clubs.len() != world.clubs.len() {
            return None;
        }
        for (club, (budget, boost, tactics)) in world.clubs.iter_mut().zip(clubs) {
            if boost > crate::world::ACADEMY_BOOST_MAX {
                return None;
            }
            club.budget = budget;
            club.academy_boost = boost;
            club.tactical_identity = tactics;
        }
        let members = Vec::<Vec<usize>>::read(&mut r)?;
        if members.len() != world.leagues.len() {
            return None;
        }
        for (league, members) in world.leagues.iter_mut().zip(members) {
            if members.len() != league.max_clubs as usize
                || members.iter().any(|&id| id >= world.clubs.len())
            {
                return None;
            }
            league.clubs = members;
        }
        let calendar = state.chronology()?;
        if replay.pop().dated_rosters
            != state
                .competition_calendar
                .as_ref()
                .is_some_and(|cal| cal.market_enabled)
        {
            return None;
        }
        let retained = RetainedWorld {
            seed: state.world_seed,
            ranked: true,
            calendar: Some(calendar),
            season: state.season_number,
            world,
            replay,
            past_loads: Vec::<u8>::read(&mut r)?,
            past_records: Vec::<u8>::read(&mut r)?,
            records: Vec::read(&mut r)?,
            loads: Vec::read(&mut r)?,
            deep_results: Vec::read(&mut r)?,
            originals: BTreeMap::read(&mut r)?,
        };
        if r.remaining() != 0
            || !retained.replay.checkpoint_valid(
                &retained.world,
                state.world_seed,
                state.season_number,
                calendar,
            )
            || !retained.past_matches(&state.orbit_records, &state.npc_match_loads)
            || !retained.records.iter().eq(state
                .orbit_records
                .iter()
                .filter(|v| v.season >= retained.season))
            || !retained.loads.iter().eq(state
                .npc_match_loads
                .iter()
                .filter(|v| v.epoch_day >= retained.active_start()))
            || retained.deep_results != state.deep_results
            || retained
                .originals
                .keys()
                .any(|&i| i >= retained.replay.pop().len())
        {
            return None;
        }
        Some(retained)
    }
}

impl SimulationSession {
    fn generated_world(&self, seed: u64) -> WorldGenesis {
        let world = WorldGenesis::generate(seed);
        #[cfg(feature = "capacity-bench")]
        if let Some(profile) = self.capacity {
            return profile.world(world);
        }
        world
    }
    pub(crate) fn permanent_top_leagues(&self) -> usize {
        #[cfg(feature = "capacity-bench")]
        if let Some(profile) = self.capacity {
            return profile.deep_budget as usize - 1;
        }
        crate::deep::DEFAULT_TOP_LEAGUES
    }
    /// Construct a stress profile without enabling a different live-world/save model.
    /// `players=0` keeps genesis squad sizes; a nonzero target is distributed among
    /// the same 1,200 clubs. Profile checkpoints cannot be read by normal sessions.
    #[cfg(feature = "capacity-bench")]
    pub fn capacity_benchmark(players: u32, deep_budget: u32) -> Result<Self, &'static str> {
        let profile = crate::capacity::CapacityProfile::new(players, deep_budget)?;
        Ok(Self {
            capacity: Some(profile),
            ..Self::new()
        })
    }
}
