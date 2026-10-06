//! Headless league selection and fixture-level NPC simulation on the career clock.
use crate::{
    fixtures::generate_fixtures,
    population::Population,
    session::SimulationSession,
    world::{LeagueId, WorldGenesis},
};
use goat_core::{
    deep::{DeepFixtureResult, DeepScope},
    history::NpcMatchLoad,
    match_model::{available_lines, simulate_match, TeamLines},
    state::{reduce, Intent, NpcMatchCredit, OrbitMatchRecord, WorldState},
};
use goat_rng::{GoatRng, RngSource};
use std::collections::BTreeSet;

pub const DEFAULT_TOP_LEAGUES: usize = 5;
const STATURE_SCORE_SCALE: u32 = 1_000;
const TIER_SCORE_PENALTY: u32 = 20_000;
const HOME_DOMAIN: u64 = 0x9E37_79B9_7F4A_7C15;
const AWAY_DOMAIN: u64 = 0xBF58_476D_1CE4_E5B9;
const MATCH_DOMAIN: u64 = 0x4445_4550_4C45_4147;
const OUTFIELD_STARTERS: usize = 10; // Existing engine has an abstract goalkeeper.
const SUB_MINUTES: [u16; 3] = [60, 70, 80];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeagueScore {
    pub league: LeagueId,
    pub score: u32,
}

/// Bootstrap scores, explicitly not a historical continental coefficient.
pub fn initial_scores(world: &WorldGenesis) -> Vec<LeagueScore> {
    world
        .leagues
        .iter()
        .map(|l| LeagueScore {
            league: l.id,
            score: (world.nations[l.nation].stature as u32 * STATURE_SCORE_SCALE)
                .saturating_sub(l.tier as u32 * TIER_SCORE_PENALTY),
        })
        .collect()
}

/// The PC league is mandatory, even outside the top N. Equal scores use stable IDs.
/// Supplied scores allow a future coefficient system without changing this policy.
pub fn select_leagues(
    world: &WorldGenesis,
    pc_league: LeagueId,
    top_n: usize,
    scores: &[LeagueScore],
) -> Vec<LeagueId> {
    let mut ranked = scores
        .iter()
        .copied()
        .filter(|r| world.leagues.iter().any(|l| l.id == r.league))
        .collect::<Vec<_>>();
    ranked.sort_by_key(|r| (std::cmp::Reverse(r.score), r.league));
    let mut seen = BTreeSet::new();
    let mut chosen = Vec::new();
    for r in ranked {
        if seen.insert(r.league) {
            chosen.push(r.league);
        }
        if chosen.len() == top_n {
            break;
        }
    }
    if top_n == 0 {
        chosen.clear();
    }
    if world.leagues.iter().any(|l| l.id == pc_league) {
        chosen.push(pc_league);
    }
    chosen.sort_unstable();
    chosen.dedup();
    chosen
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DeepProgress {
    seed: u64,
    year: u32,
    season: u32,
    day: u32,
    pc_league: u8,
    pc_club: u16,
    absent_round: Option<usize>,
}

/// Shared annual close output for headless and live adapters.
pub struct DeepSeasonClose {
    pub membership: Vec<Vec<crate::world::ClubId>>,
    pub events: Vec<crate::promotion::PromoRelegationEvent>,
}

struct Side {
    squad: Vec<usize>,
    minutes: Vec<(usize, u16)>,
    starters: Vec<usize>,
    lines: TeamLines,
}
fn side(pop: &Population, squad: Vec<usize>, week: u32) -> Side {
    let mut available = squad
        .iter()
        .copied()
        .filter(|&i| pop.is_available(i, week))
        .map(|i| {
            let energy = pop.energy_at(i, week).to_int();
            let rating = pop.current_ovr(i, week) as i32;
            (i, rating * 70 + pop.form[i] as i32 * 20 + energy * 10)
        })
        .collect::<Vec<_>>();
    available.sort_by_key(|&(i, score)| (std::cmp::Reverse(score), i));
    // Balanced 4-3-3 outfield, with available backups filling shortages.
    let mut starters = Vec::new();
    for (position, quota) in [(0, 3), (1, 3), (2, 4)] {
        starters.extend(
            available
                .iter()
                .filter(|&&(i, _)| pop.position[i] == position)
                .take(quota)
                .map(|&(i, _)| i),
        );
    }
    for &(i, _) in &available {
        if starters.len() == OUTFIELD_STARTERS {
            break;
        }
        if !starters.contains(&i) {
            starters.push(i);
        }
    }
    let mut minutes = starters.iter().map(|&i| (i, 90)).collect::<Vec<_>>();
    let mut used = starters.clone();
    let mut tired = starters.clone();
    tired.sort_by_key(|&i| (pop.energy_at(i, week).to_int(), i));
    for (&off, minute) in tired.iter().zip(SUB_MINUTES) {
        if let Some(&(on, _)) = available
            .iter()
            .find(|&&(i, _)| !used.contains(&i) && pop.position[i] == pop.position[off])
        {
            minutes.iter_mut().find(|m| m.0 == off).unwrap().1 = minute;
            minutes.push((on, 90 - minute));
            used.push(on);
        }
    }
    let strength = |position| {
        let players = starters
            .iter()
            .copied()
            .filter(|&i| pop.position[i] == position)
            .collect::<Vec<_>>();
        if players.is_empty() {
            return 1;
        }
        let sum = players
            .iter()
            .map(|&i| {
                let energy = pop.energy_at(i, week).to_int().clamp(0, 100);
                pop.current_ovr(i, week) as u32 * (700 + energy as u32 * 3) / 1000
            })
            .sum::<u32>();
        (sum / players.len() as u32).clamp(1, 99) as u8
    };
    let lines = available_lines(
        TeamLines {
            attack: strength(0),
            midfield: strength(1),
            defense: strength(2),
        },
        (starters.len() + 1).min(11) as u8,
    );
    Side {
        squad,
        minutes,
        starters,
        lines,
    }
}
fn credits(
    pop: &Population,
    side: &Side,
    goals: u32,
    result: i8,
    week: u32,
    rng: &mut impl RngSource,
) -> Vec<NpcMatchCredit> {
    let mut out = side
        .minutes
        .iter()
        .map(|&(i, _)| NpcMatchCredit {
            pop_idx: i as u32,
            goals: 0,
            assists: 0,
            result,
        })
        .collect::<Vec<_>>();
    if out.is_empty() {
        return out;
    }
    let weights = side
        .minutes
        .iter()
        .map(|&(i, minutes)| {
            let position = match pop.position[i] {
                0 => 6,
                1 => 3,
                _ => 1,
            };
            position * pop.current_ovr(i, week) as u64 * minutes as u64
        })
        .collect::<Vec<_>>();
    let total = weights.iter().sum::<u64>();
    for _ in 0..goals {
        let mut roll = rng.next_range_u64(0, total.saturating_sub(1));
        let scorer = weights
            .iter()
            .position(|&w| {
                if roll < w {
                    true
                } else {
                    roll -= w;
                    false
                }
            })
            .unwrap_or(0);
        out[scorer].goals += 1;
        let (player, duration) = side.minutes[scorer];
        let start = if side.starters.contains(&player) {
            0
        } else {
            90 - duration
        };
        let minute = rng.next_range_u32(start as u32, (start + duration - 1) as u32);
        let assist_candidates = side
            .minutes
            .iter()
            .enumerate()
            .filter(|&(index, &(player, duration))| {
                let start = if side.starters.contains(&player) {
                    0
                } else {
                    90 - duration
                };
                index != scorer && minute >= start as u32 && minute < (start + duration) as u32
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if !assist_candidates.is_empty() && rng.next_range_u32(0, 99) < 75 {
            let assist = assist_candidates
                [rng.next_range_u64(0, assist_candidates.len() as u64 - 1) as usize];
            out[assist].assists += 1;
        }
    }
    out
}
fn loads(side: &Side, fixture_id: u64, day: u32) -> Vec<NpcMatchLoad> {
    side.squad
        .iter()
        .map(|&i| NpcMatchLoad {
            competition_id: goat_core::calendar_loop::LEAGUE_COMPETITION_ID,
            pop_idx: i as u32,
            fixture_id,
            epoch_day: day,
            minutes: side.minutes.iter().find(|m| m.0 == i).map_or(0, |m| m.1),
        })
        .collect()
}

impl SimulationSession {
    /// Advance selected NPC fixtures only through the elapsed date, never future rounds.
    /// PC fixtures are supplied by the match engine; other leagues need no renderer.
    /// Changing PC league creates a dated scope event; no old light result is rerolled.
    pub fn advance_deep(&mut self, state: WorldState, live_world: &WorldGenesis) -> WorldState {
        self.advance_deep_model(state, live_world, None)
    }

    /// The suspended/academy PC cannot participate; the club still fields NPCs.
    pub fn advance_deep_without_pc(
        &mut self,
        state: WorldState,
        live_world: &WorldGenesis,
        round: usize,
    ) -> WorldState {
        self.advance_deep_model(state, live_world, Some(round))
    }

    fn advance_deep_model(
        &mut self,
        mut state: WorldState,
        live_world: &WorldGenesis,
        absent_round: Option<usize>,
    ) -> WorldState {
        let Some(c) = state.chronology() else {
            return state;
        };
        if state.season_number == 0 || state.pc_seasons_played >= state.season_number {
            return state;
        }
        let progress = DeepProgress {
            seed: state.world_seed,
            year: state.career_base_year,
            season: state.season_number,
            day: state.pc_epoch_day,
            pc_league: state.pc_div_idx,
            pc_club: state.pc_club_idx,
            absent_round,
        };
        if self.deep_progress == Some(progress) {
            return state;
        }
        let pc_league = state.pc_div_idx as usize;
        let leagues = select_leagues(
            live_world,
            pc_league,
            DEFAULT_TOP_LEAGUES,
            &initial_scores(live_world),
        );
        let scope = DeepScope {
            season: state.season_number,
            epoch_day: if state
                .deep_scopes
                .last()
                .is_none_or(|s| s.season != state.season_number)
            {
                c.frame(state.season_number).preparation_start
            } else {
                state.pc_epoch_day
            },
            pc_league: pc_league as u32,
            pc_club: state.pc_club_idx as u32,
            leagues: leagues.iter().map(|&id| id as u32).collect(),
        };
        state = reduce(
            state,
            Intent::SelectDeepScope { scope },
            &mut GoatRng::new(0),
        );
        let scopes = state
            .deep_scopes
            .iter()
            .filter(|s| s.season == state.season_number)
            .collect::<Vec<_>>();
        let leagues = scopes
            .iter()
            .flat_map(|s| s.leagues.iter().map(|&id| id as usize))
            .collect::<BTreeSet<_>>();
        let done = state
            .deep_results
            .iter()
            .map(|r| r.key())
            .collect::<BTreeSet<_>>();
        // Replay membership for remote leagues, then overlay the PC's live nation.
        self.population_deep(&state);
        let mut world = self.deep_world().unwrap();
        let pc_nation = live_world.leagues[pc_league].nation;
        for league in &live_world.leagues {
            if league.nation == pc_nation {
                world.leagues[league.id].clubs = league.clubs.clone();
            }
        }
        let mut fixtures = Vec::new();
        for league in leagues {
            for f in generate_fixtures(
                state.world_seed,
                state.season_number,
                league,
                &world.leagues[league].clubs,
            ) {
                let day = crate::calendar::dated_fixture_day(c, state.season_number, f.round);
                let key = (
                    state.season_number,
                    league as u32,
                    f.round as u32,
                    f.home as u32,
                    f.away as u32,
                );
                let active = scopes.iter().rev().find(|scope| scope.epoch_day <= day);
                let included = active.is_some_and(|scope| scope.leagues.contains(&(league as u32)));
                let pc_fixture = active.is_some_and(|scope| {
                    scope.pc_league == league as u32
                        && (scope.pc_club == f.home as u32 || scope.pc_club == f.away as u32)
                });
                let absent = absent_round == Some(f.round)
                    && league == pc_league
                    && (f.home == state.pc_club_idx as usize
                        || f.away == state.pc_club_idx as usize);
                if included
                    && day <= state.pc_epoch_day
                    && !done.contains(&key)
                    && (!pc_fixture || absent)
                {
                    fixtures.push((day, league, f));
                }
            }
        }
        fixtures.sort_by_key(|&(day, league, f)| (day, league, f.round, f.home, f.away));
        let mut cursor = 0;
        while cursor < fixtures.len() {
            let day = fixtures[cursor].0;
            let end = cursor + fixtures[cursor..].iter().take_while(|f| f.0 == day).count();
            let pop = self.population_deep(&state);
            let mut squads = vec![Vec::new(); world.clubs.len()];
            for i in 0..pop.len() {
                squads[pop.club[i] as usize].push(i);
            }
            let mut pending = Vec::new();
            for &(day, league, f) in &fixtures[cursor..end] {
                let week = day / 7;
                let home = side(pop, squads[f.home].clone(), week);
                let away = side(pop, squads[f.away].clone(), week);
                let seed = state.world_seed
                    ^ ((state.season_number as u64) << 32)
                    ^ ((league as u64) << 48)
                    ^ ((f.round as u64) << 16)
                    ^ MATCH_DOMAIN
                    ^ (f.home as u64).wrapping_mul(HOME_DOMAIN)
                    ^ (f.away as u64).wrapping_mul(AWAY_DOMAIN);
                let mut rng = GoatRng::new(seed);
                let (gf, ga) = simulate_match(home.lines, away.lines, &mut rng);
                let result = gf.cmp(&ga) as i8; // Ordering discriminants: Less=-1, Equal=0, Greater=1.
                let mut rows = credits(pop, &home, gf, result, week, &mut rng);
                rows.extend(credits(pop, &away, ga, -result, week, &mut rng));
                let local_week = crate::round_to_week(f.round);
                let slot = f.round - crate::week_to_rounds(local_week).start;
                let id = crate::workload::league_fixture_id(
                    state.world_seed,
                    state.season_number,
                    f.round,
                    slot,
                );
                let mut doses = loads(&home, id, day);
                doses.extend(loads(&away, id, day));
                pending.push((
                    DeepFixtureResult {
                        season: state.season_number,
                        round: f.round as u32,
                        league: league as u32,
                        epoch_day: day,
                        home: f.home as u32,
                        away: f.away as u32,
                        home_goals: gf,
                        away_goals: ga,
                    },
                    rows,
                    doses,
                ));
            }
            let mut all_doses = Vec::new();
            for (result, rows, doses) in pending {
                state = reduce(
                    state,
                    Intent::RecordOrbitMatch {
                        record: OrbitMatchRecord {
                            season: result.season,
                            round: result.round,
                            div: result.league as u8,
                            credits: rows,
                        },
                    },
                    &mut GoatRng::new(0),
                );
                all_doses.extend(doses);
                state = reduce(
                    state,
                    Intent::RecordDeepFixture { result },
                    &mut GoatRng::new(0),
                );
            }
            state = reduce(
                state,
                Intent::RecordNpcMatchLoads { loads: all_doses },
                &mut GoatRng::new(0),
            );
            cursor = end;
        }
        self.deep_progress = Some(progress);
        state
    }
}

/// Capture the PC engine's score without synthesizing or duplicating its NPC credits.
pub fn record_pc_score(
    state: WorldState,
    round: usize,
    home: usize,
    away: usize,
    home_goals: u32,
    away_goals: u32,
) -> WorldState {
    let result = DeepFixtureResult {
        season: state.season_number,
        round: round as u32,
        league: state.pc_div_idx as u32,
        epoch_day: state.pc_epoch_day,
        home: home as u32,
        away: away as u32,
        home_goals,
        away_goals,
    };
    reduce(
        state,
        Intent::RecordDeepFixture { result },
        &mut GoatRng::new(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> WorldState {
        let mut s = WorldState::new();
        s.world_seed = 42;
        s.dated_calendar = true;
        s.career_base_year = 2023;
        s.season_number = 1;
        s.pc_div_idx = 59;
        s.pc_club_idx = 1180;
        s
    }
    #[test]
    fn ranking_ties_dedup_and_mandatory_pc_league() {
        let w = WorldGenesis::generate(42);
        let mut scores = vec![LeagueScore {
            league: 999,
            score: 999,
        }];
        scores.extend([12, 9, 6, 3, 0, 0].map(|league| LeagueScore { league, score: 100 }));
        assert_eq!(select_leagues(&w, 59, 5, &scores), vec![0, 3, 6, 9, 12, 59]);
        assert_eq!(select_leagues(&w, 0, 5, &scores), vec![0, 3, 6, 9, 12]);
        assert_eq!(select_leagues(&w, 59, 0, &scores), vec![59]);
        scores.reverse();
        assert_eq!(select_leagues(&w, 59, 5, &scores), vec![0, 3, 6, 9, 12, 59]);
    }
    #[test]
    fn fixtures_conserve_minutes_credits_and_replay_without_duplicates() {
        let w = WorldGenesis::generate(42);
        let c = goat_core::chronology::Chronology::new(2023);
        let day = crate::calendar::dated_fixture_day(c, 1, 0);
        let mut session = SimulationSession::new();
        let mut s = session.advance_deep(state(), &w);
        assert!(s.deep_results.is_empty());
        s.pc_epoch_day = day;
        s = session.advance_deep(s, &w);
        assert_eq!(
            s.deep_results.len(),
            s.deep_scopes[0].leagues.len() * 10 - 1
        );
        assert!(s
            .deep_results
            .iter()
            .all(|r| r.round == 0 && r.epoch_day == day));
        assert_eq!(s.deep_scopes[0].leagues, vec![0, 3, 15, 18, 51, 59]);
        assert_eq!(
            s.deep_results[0],
            DeepFixtureResult {
                season: 1,
                round: 0,
                league: 0,
                epoch_day: 49,
                home: 0,
                away: 19,
                home_goals: 0,
                away_goals: 1
            }
        );
        let pop = session.population_deep(&s);
        for r in &s.deep_results {
            for club in [r.home, r.away] {
                let doses = s
                    .npc_match_loads
                    .iter()
                    .filter(|l| pop.club[l.pop_idx as usize] as u32 == club)
                    .collect::<Vec<_>>();
                assert!(!doses.is_empty());
                let minutes = doses.iter().map(|l| l.minutes as u32).sum::<u32>();
                assert!(minutes <= 900);
                assert_eq!(minutes % 90, 0, "substitutions conserve every field slot");
                let record = s
                    .orbit_records
                    .iter()
                    .find(|rec| rec.div as u32 == r.league && rec.round == 0)
                    .unwrap();
                let rows = record
                    .credits
                    .iter()
                    .filter(|credit| pop.club[credit.pop_idx as usize] as u32 == club)
                    .collect::<Vec<_>>();
                assert_eq!(rows.len(), doses.iter().filter(|l| l.minutes > 0).count());
                assert_eq!(
                    rows.iter().map(|credit| credit.goals as u32).sum::<u32>(),
                    if club == r.home {
                        r.home_goals
                    } else {
                        r.away_goals
                    }
                );
                assert!(rows
                    .iter()
                    .all(|credit| pop.is_available(credit.pop_idx as usize, day / 7)));
            }
        }
        let before = s.deep_results.clone();
        let apps = pop.career_apps.clone();
        let doses = s.npc_match_loads.clone();
        s = session.advance_deep(s, &w);
        assert_eq!(s.deep_results, before);
        assert_eq!(s.npc_match_loads, doses);
        assert_eq!(session.population_deep(&s).career_apps, apps);
        let mut fresh = SimulationSession::new();
        assert_eq!(
            session.population_deep(&s).career_fingerprint(),
            fresh.population_deep(&s).career_fingerprint()
        );
        assert_eq!(session.rebuild_count(), 1);
        // Transfer to a previously light sibling league between match dates.
        s.pc_epoch_day += 2;
        s.pc_div_idx = 58;
        s.pc_club_idx = 1160;
        s = session.advance_deep(s, &w);
        assert_eq!(
            s.deep_results, before,
            "joining a league must not reroll its past"
        );
        assert_eq!(s.deep_scopes.len(), 2);
        s.pc_epoch_day = crate::calendar::dated_fixture_day(c, 1, 1);
        s = session.advance_deep(s, &w);
        assert!(s
            .deep_results
            .iter()
            .any(|r| r.league == 58 && r.round == 1));
        assert!(!s
            .deep_results
            .iter()
            .any(|r| r.league == 58 && r.round == 0));
        assert_eq!(
            session.rebuild_count(),
            1,
            "sorted appends/round credit merges reuse the cache"
        );
        assert_eq!(
            session.population_deep(&s).career_fingerprint(),
            fresh.population_deep(&s).career_fingerprint()
        );
        let canonical = crate::orbit::rebuild_population_deep(
            42,
            2023,
            1,
            &s.orbit_records,
            &s.npc_match_loads,
            &s.deep_results,
        );
        let week = s.pc_epoch_day / 7 + 1;
        for idx in [0, 100, 1160 * 20] {
            if idx < canonical.len() {
                assert_eq!(
                    session.population_deep(&s).current_ovr(idx, week),
                    canonical.current_ovr(idx, week)
                );
                assert_eq!(
                    session.population_deep(&s).energy_at(idx, week),
                    canonical.energy_at(idx, week)
                );
            }
        }
    }
    #[test]
    fn transfer_between_advances_keeps_continuously_deep_leagues_caught_up() {
        let world = WorldGenesis::generate(42);
        let c = goat_core::chronology::Chronology::new(2023);
        let first = crate::calendar::dated_fixture_day(c, 1, 0);
        let second = crate::calendar::dated_fixture_day(c, 1, 1);
        let mut one_step = SimulationSession::new();
        let mut split = SimulationSession::new();
        let mut a = one_step.advance_deep(state(), &world);
        let mut b = split.advance_deep(state(), &world);
        b.pc_epoch_day = first + 1;
        b = split.advance_deep(b, &world);
        for state in [&mut a, &mut b] {
            state.pc_epoch_day = second;
            state.pc_div_idx = 58;
            state.pc_club_idx = 1160;
        }
        a = one_step.advance_deep(a, &world);
        b = split.advance_deep(b, &world);
        assert_eq!(a.deep_scopes, b.deep_scopes);
        assert_eq!(a.deep_results, b.deep_results);
        assert_eq!(a.npc_match_loads, b.npc_match_loads);
        assert_eq!(a.orbit_records, b.orbit_records);
        assert_eq!(
            a.deep_results
                .iter()
                .filter(|r| r.league == 0 && r.round == 0)
                .count(),
            10
        );
        assert_eq!(
            a.deep_results
                .iter()
                .filter(|r| r.league == 59 && r.round == 0)
                .count(),
            9
        );
        assert!(!a
            .deep_results
            .iter()
            .any(|r| r.league == 58 && r.round == 0));
    }
}

#[cfg(test)]
mod replay_tests {
    use super::*;
    #[test]
    fn actual_scores_drive_season_table_and_promotion_inputs() {
        let world = WorldGenesis::generate(42);
        let league = 59;
        let club = world.leagues[league].clubs[0];
        let scores = generate_fixtures(42, 1, league, &world.leagues[league].clubs)
            .into_iter()
            .filter(|f| f.home == club || f.away == club)
            .map(|f| DeepFixtureResult {
                season: 1,
                round: f.round as u32,
                league: league as u32,
                epoch_day: 100,
                home: f.home as u32,
                away: f.away as u32,
                home_goals: if f.home == club { 3 } else { 0 },
                away_goals: if f.away == club { 3 } else { 0 },
            })
            .collect::<Vec<_>>();
        let mut pop = crate::population::genesis(42, &world);
        let (results, tables, _) = crate::batch_tick::batch_tick_season_deep_with_match_points(
            &mut pop,
            &world,
            &world.static_league_clubs(),
            42,
            1,
            52,
            None,
            &scores,
        );
        assert_eq!(results[league].champion_club, club);
        assert_eq!(tables[league].sorted()[0].points(), 114);
        let live = crate::promotion::sim_league_season_with_scores(
            &world,
            league,
            &world.leagues[league].clubs,
            42,
            1,
            &scores,
        );
        assert_eq!(live.sorted()[0].club_id, club);
        assert_eq!(live.sorted()[0].points(), 114);
    }
    #[test]
    fn season_close_uses_canonical_promoted_membership_for_the_next_deep_year() {
        let world = WorldGenesis::generate(42);
        let c = goat_core::chronology::Chronology::new(2023);
        let mut s = WorldState::new();
        s.world_seed = 42;
        s.dated_calendar = true;
        s.career_base_year = 2023;
        s.season_number = 1;
        s.pc_div_idx = 59;
        s.pc_club_idx = 1180;
        let mut warm = SimulationSession::new();
        assert!(
            warm.finish_deep_season(&s).is_none(),
            "do not close future fixtures"
        );
        s.deep_results = generate_fixtures(42, 1, 59, &world.leagues[59].clubs)
            .into_iter()
            .filter(|f| f.home == 1180 || f.away == 1180)
            .map(|f| DeepFixtureResult {
                season: 1,
                round: f.round as u32,
                league: 59,
                epoch_day: crate::calendar::dated_fixture_day(c, 1, f.round),
                home: f.home as u32,
                away: f.away as u32,
                home_goals: if f.home == 1180 { 3 } else { 0 },
                away_goals: if f.away == 1180 { 3 } else { 0 },
            })
            .collect();
        s.pc_epoch_day = c.frame(2).preparation_start;
        let closed = warm.finish_deep_season(&s).unwrap();
        assert!(closed.membership[58].contains(&1180));
        assert!(!closed.membership[59].contains(&1180));
        let mut cold = SimulationSession::new();
        let replayed = cold.finish_deep_season(&s).unwrap();
        assert_eq!(closed.membership, replayed.membership);
        assert_eq!(closed.events, replayed.events);
        let next = warm.deep_world().unwrap();
        assert_eq!(next.leagues[58].clubs, closed.membership[58]);
        assert_ne!(next.leagues[58].clubs, world.leagues[58].clubs);
        s.season_number = 2;
        s.pc_div_idx = 58;
        s.pc_epoch_day = crate::calendar::dated_fixture_day(c, 2, 0);
        s = warm.advance_deep(s, &next);
        assert!(s
            .deep_results
            .iter()
            .filter(|r| r.season == 2 && r.league == 58)
            .all(|r| closed.membership[58].contains(&(r.home as usize))
                && closed.membership[58].contains(&(r.away as usize))));
        assert_eq!(warm.rebuild_count(), 1);
    }
    #[test]
    fn rescheduled_past_fixture_invalidates_health_but_future_load_keeps_prefix() {
        let world = WorldGenesis::generate(42);
        let mut warm = crate::population::genesis_dated(42, &world, 2023);
        let mut fresh = crate::population::genesis_dated(42, &world, 2023);
        let c = goat_core::chronology::Chronology::new(2023);
        warm.current_ovr(0, 20);
        let dose = NpcMatchLoad {
            pop_idx: 0,
            competition_id: 1,
            fixture_id: crate::workload::league_fixture_id(42, 1, 0, 0),
            epoch_day: c.frame(1).end_day,
            minutes: 90,
        };
        warm.record_match_load(dose);
        fresh.record_match_load(dose);
        assert_eq!(warm.current_ovr(0, 20), fresh.current_ovr(0, 20));
        assert_eq!(warm.energy_at(0, 20), fresh.energy_at(0, 20));
        let dose = NpcMatchLoad {
            fixture_id: crate::workload::league_fixture_id(42, 1, 20, 0),
            epoch_day: crate::calendar::dated_fixture_day(c, 1, 20),
            minutes: 0,
            ..dose
        };
        warm.record_match_load(dose);
        fresh.record_match_load(dose);
        assert_eq!(warm.current_ovr(0, 30), fresh.current_ovr(0, 30));
        assert_eq!(warm.energy_at(0, 30), fresh.energy_at(0, 30));
    }
}
