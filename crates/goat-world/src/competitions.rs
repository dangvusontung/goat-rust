//! A dated, persistent competition subsystem on the shared career chronology.
use crate::{population::Population, session::SimulationSession, world::WorldGenesis};
use goat_core::{
    chronology::{Chronology, CivilDate},
    competitions::*,
    deep::{DeepFixtureResult, DeepScope},
    discipline::NpcCardEvent,
    history::NpcMatchLoad,
    state::{reduce, Intent, NpcMatchCredit, OrbitMatchRecord, WorldState},
    tactical::TacticalProfile,
};
use goat_rng::{GoatRng, RngSource};
use std::collections::{BTreeMap, BTreeSet};
const FIXTURE_DOMAIN: u64 = 0xC400_0000_0000_0000;
const MATCH_DOMAIN: u64 = 0x4341_4C45_4E44_4152;
const SHOOTOUT_DOMAIN: u64 = 0x5348_4F4F_544F_5554;
const INTERNATIONAL_DATES: [(u8, u8); 5] = [(9, 5), (10, 10), (11, 14), (3, 24), (6, 5)];
const DOMESTIC_DATES: [(u8, u8); 6] = [(9, 17), (11, 12), (1, 14), (3, 11), (4, 15), (5, 20)];
const GROUP_DATES: [(u8, u8); 3] = [(9, 24), (10, 22), (11, 26)];
const KNOCKOUT_DATES: [(u8, u8); 5] = [(2, 5), (3, 5), (4, 2), (4, 30), (5, 28)];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalendarError {
    InvalidClock,
    MissingCalendar,
    NoLegalDate(u64),
    InvalidProgress,
}
fn date(c: Chronology, season: u32, month: u8, day: u8) -> u32 {
    c.epoch_day(CivilDate {
        year: c.base_year + season - 1 + u32::from(month <= 6),
        month,
        day,
    })
    .unwrap()
}
fn summer(c: Chronology, season: u32, day: u8) -> u32 {
    c.epoch_day(CivilDate {
        year: c.base_year + season,
        month: 7,
        day,
    })
    .unwrap()
}
#[allow(clippy::too_many_arguments)]
fn fixture(
    season: u32,
    competition: u32,
    region: u32,
    stage: u8,
    round: u32,
    slot: u32,
    leg: u8,
    day: u32,
    home: u32,
    away: u32,
    priority: u8,
) -> DatedFixture {
    let id = FIXTURE_DOMAIN
        | ((season as u64) << 40)
        | ((competition as u64) << 32)
        | ((region as u64) << 24)
        | ((stage as u64) << 20)
        | ((round as u64) << 12)
        | ((leg as u64) << 10)
        | slot as u64;
    DatedFixture {
        id,
        workload_id: id,
        season,
        competition,
        region,
        stage,
        round,
        slot,
        leg,
        original_day: day,
        day,
        home,
        away,
        priority,
    }
}
fn put(cal: &mut CompetitionCalendar, t: &mut Tournament, f: DatedFixture) {
    t.fixture_ids.push(f.id);
    cal.fixtures.push(f);
}
fn blocked(c: Chronology, f: &DatedFixture, day: u32) -> bool {
    !f.national()
        && f.competition != FRIENDLY
        && INTERNATIONAL_DATES
            .iter()
            .any(|&(m, d)| date(c, f.season, m, d).abs_diff(day) < MIN_REST_DAYS)
}
/// Preserve resolved dates, place higher importance first, and retain original dates.
/// Club/nation identities are separate; national windows protect club availability.
pub fn resolve_schedule(cal: &mut CompetitionCalendar, c: Chronology) -> Result<(), CalendarError> {
    let mut occupied: BTreeMap<(bool, u32), BTreeSet<u32>> = BTreeMap::new();
    for r in &cal.results {
        for team in [r.fixture.home, r.fixture.away] {
            occupied
                .entry((r.fixture.national(), team))
                .or_default()
                .insert(r.fixture.day);
        }
    }
    cal.fixtures
        .sort_by_key(|f| (std::cmp::Reverse(f.priority), f.original_day, f.leg, f.id));
    let mut legs = BTreeMap::new();
    for r in &cal.results {
        let f = &r.fixture;
        if f.leg == 1 {
            legs.insert(
                (f.season, f.competition, f.region, f.stage, f.round, f.slot),
                f.day,
            );
        }
    }
    for f in &mut cal.fixtures {
        let key = (f.season, f.competition, f.region, f.stage, f.round, f.slot);
        let lower = if f.leg == 2 {
            legs.get(&key)
                .copied()
                .unwrap_or(f.original_day)
                .saturating_add(MIN_REST_DAYS)
        } else {
            0
        };
        let mut day = f.original_day.max(lower).max(cal.resolved_day);
        let max = if f.national() {
            summer(c, f.season, 31)
        } else {
            c.frame(f.season).end_day
        };
        loop {
            let conflict = [f.home, f.away].iter().any(|&team| {
                occupied.get(&(f.national(), team)).is_some_and(|days| {
                    days.range(
                        day.saturating_sub(MIN_REST_DAYS - 1)
                            ..=day.saturating_add(MIN_REST_DAYS - 1),
                    )
                    .next()
                    .is_some()
                })
            });
            if !blocked(c, f, day) && !conflict {
                break;
            }
            day += 1;
            if day > max {
                return Err(CalendarError::NoLegalDate(f.id));
            }
        }
        if day > max {
            return Err(CalendarError::NoLegalDate(f.id));
        }
        f.day = day;
        if f.leg == 1 {
            legs.insert(key, day);
        }
        for team in [f.home, f.away] {
            occupied
                .entry((f.national(), team))
                .or_default()
                .insert(day);
        }
    }
    cal.fixtures
        .sort_by_key(|f| (f.day, std::cmp::Reverse(f.priority), f.id));
    Ok(())
}
fn knockout(cal: &mut CompetitionCalendar, t: &mut Tournament, c: Chronology, seed: u64) {
    t.fixture_ids.clear();
    let alive = t.alive.iter().map(|&x| x as usize).collect::<Vec<_>>();
    let draw = crate::domestic_cup::draw_bracket_round(
        seed ^ ((t.competition as u64) << 48)
            ^ ((t.season as u64) << 24)
            ^ ((t.region as u64) << 8)
            ^ t.round as u64,
        &alive,
    );
    // The bye is a path-dependent part of the bracket, stored alongside paired entrants.
    let final_round = t.alive.len() == 2;
    for (slot, (a, b)) in draw.pairs.into_iter().enumerate() {
        let first = match t.kind {
            0 => {
                let (m, d) = DOMESTIC_DATES[(t.round as usize).min(5)];
                date(c, t.season, m, d)
            }
            3 => summer(c, t.season, if final_round { 23 } else { 18 }),
            _ => {
                let (m, d) = if final_round {
                    (5, 28)
                } else {
                    KNOCKOUT_DATES[(t.round as usize).min(4)]
                };
                date(c, t.season, m, d)
            }
        };
        let priority = if final_round {
            5
        } else if t.kind == 1 {
            4
        } else {
            3
        };
        put(
            cal,
            t,
            fixture(
                t.season,
                t.competition,
                t.region,
                1,
                t.round,
                slot as u32,
                1,
                first,
                a as u32,
                b as u32,
                priority,
            ),
        );
        if t.kind == 1 && !final_round {
            put(
                cal,
                t,
                fixture(
                    t.season,
                    t.competition,
                    t.region,
                    1,
                    t.round,
                    slot as u32,
                    2,
                    first + 7,
                    b as u32,
                    a as u32,
                    priority,
                ),
            );
        }
    }
}
fn groups(cal: &mut CompetitionCalendar, t: &mut Tournament, c: Chronology) {
    t.fixture_ids.clear();
    let gs = t.groups.clone();
    for (group, teams) in gs.iter().enumerate() {
        for (round, pairs) in crate::national_tournament::round_robin_schedule(teams.len())
            .into_iter()
            .enumerate()
        {
            for (slot, (a, b)) in pairs.pairs.into_iter().enumerate() {
                let day = match t.kind {
                    2 => {
                        let (m, d) = INTERNATIONAL_DATES[round];
                        date(c, t.season, m, d)
                    }
                    3 => summer(c, t.season, [3, 8, 13][round]),
                    _ => {
                        let (m, d) = GROUP_DATES[round];
                        date(c, t.season, m, d)
                    }
                };
                put(
                    cal,
                    t,
                    fixture(
                        t.season,
                        t.competition,
                        t.region,
                        0,
                        round as u32,
                        (group * 4 + slot) as u32,
                        0,
                        day,
                        teams[a],
                        teams[b],
                        4,
                    ),
                );
            }
        }
    }
}
fn table_for(
    cal: &CompetitionCalendar,
    clubs: &[usize],
    competition: u32,
    season: u32,
) -> crate::season::Table {
    let mut table = crate::season::Table::new(clubs);
    for r in cal
        .results
        .iter()
        .filter(|r| r.fixture.competition == competition && r.fixture.season == season)
    {
        if clubs.contains(&(r.fixture.home as usize)) && clubs.contains(&(r.fixture.away as usize))
        {
            table.apply_result(
                r.fixture.home as usize,
                r.fixture.away as usize,
                r.goals[0],
                r.goals[1],
            );
        }
    }
    table
}
fn rank_group(
    cal: &CompetitionCalendar,
    teams: &[u32],
    competition: u32,
    season: u32,
    region: u32,
) -> Vec<u32> {
    let mut rows = teams
        .iter()
        .map(|&id| (id, (0u32, 0i32, 0u32)))
        .collect::<BTreeMap<_, _>>();
    for r in cal.results.iter().filter(|r| {
        r.fixture.season == season
            && r.fixture.competition == competition
            && r.fixture.region == region
            && r.fixture.stage == 0
    }) {
        if !rows.contains_key(&r.fixture.home) || !rows.contains_key(&r.fixture.away) {
            continue;
        }
        for (id, gf, ga) in [
            (r.fixture.home, r.goals[0], r.goals[1]),
            (r.fixture.away, r.goals[1], r.goals[0]),
        ] {
            let row = rows.get_mut(&id).unwrap();
            row.0 += if gf > ga {
                3
            } else if gf == ga {
                1
            } else {
                0
            };
            row.1 += gf as i32 - ga as i32;
            row.2 += gf;
        }
    }
    let mut ids = teams.to_vec();
    ids.sort_by_key(|id| {
        (
            std::cmp::Reverse(rows[id].0),
            std::cmp::Reverse(rows[id].1),
            std::cmp::Reverse(rows[id].2),
            *id,
        )
    });
    ids
}
fn bootstrap_tables(world: &WorldGenesis) -> Vec<crate::season::Table> {
    world
        .leagues
        .iter()
        .map(|l| {
            let mut t = crate::season::Table::new(&l.clubs);
            // Seed-derived pre-career standing; never qualify using unplayed current fixtures.
            for entry in &mut t.entries {
                entry.w = world.clubs[entry.club_id].strength as u32;
            }
            t
        })
        .collect()
}
fn prepare(cal: &mut CompetitionCalendar, world: &WorldGenesis, state: &WorldState, c: Chronology) {
    let season = state.season_number;
    for league in &world.leagues {
        for f in
            crate::fixtures::generate_fixtures(state.world_seed, season, league.id, &league.clubs)
        {
            let day = crate::calendar::dated_fixture_day(c, season, f.round);
            let mut dated = fixture(
                season,
                LEAGUE,
                league.id as u32,
                0,
                f.round as u32,
                world.leagues[league.id]
                    .clubs
                    .iter()
                    .position(|&club| club == f.home)
                    .unwrap() as u32,
                0,
                day,
                f.home as u32,
                f.away as u32,
                1,
            );
            let w = crate::round_to_week(f.round);
            let slot = f.round - crate::week_to_rounds(w).start;
            dated.workload_id = if cal.market_enabled {
                dated.id
            } else {
                crate::workload::league_fixture_id(state.world_seed, season, f.round, slot)
            };
            cal.fixtures.push(dated);
        }
    }
    for nation in &world.nations {
        let clubs = world
            .clubs
            .iter()
            .filter(|club| club.nation == nation.id)
            .map(|club| club.id as u32)
            .collect::<Vec<_>>();
        let mut t = Tournament {
            season,
            competition: DOMESTIC,
            region: nation.id as u32,
            kind: 0,
            phase: 1,
            round: 0,
            participants: clubs.clone(),
            groups: Vec::new(),
            alive: clubs.clone(),
            fixture_ids: Vec::new(),
            champion: None,
        };
        knockout(cal, &mut t, c, state.world_seed);
        cal.tournaments.push(t);
        for (round, day) in [date(c, season, 8, 1), date(c, season, 8, 8)]
            .into_iter()
            .enumerate()
        {
            let draw = crate::domestic_cup::draw_bracket_round(
                state.world_seed
                    ^ 0xF81E_0000
                    ^ ((season as u64) << 32)
                    ^ ((nation.id as u64) << 8)
                    ^ round as u64,
                &clubs.iter().map(|&x| x as usize).collect::<Vec<_>>(),
            );
            for (slot, (a, b)) in draw.pairs.into_iter().enumerate() {
                cal.fixtures.push(fixture(
                    season,
                    FRIENDLY,
                    nation.id as u32,
                    0,
                    round as u32,
                    slot as u32,
                    0,
                    day,
                    a as u32,
                    b as u32,
                    0,
                ));
            }
        }
    }
    let tables = if season == 1 {
        bootstrap_tables(world)
    } else {
        // Last season's membership comes from its resolved fixtures, not newly promoted lists.
        world
            .leagues
            .iter()
            .map(|l| {
                let clubs = cal
                    .results
                    .iter()
                    .filter(|r| {
                        r.fixture.season == season - 1
                            && r.fixture.competition == LEAGUE
                            && r.fixture.region == l.id as u32
                    })
                    .flat_map(|r| [r.fixture.home as usize, r.fixture.away as usize])
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                table_for(cal, &clubs, LEAGUE, season - 1)
            })
            .collect()
    };
    for (i, tier) in crate::continental::ContinentalTier::ALL
        .into_iter()
        .enumerate()
    {
        let entrants = crate::ranking::qualified(world, &tables, tier);
        let gs = crate::continental::draw_groups(state.world_seed, season, tier, &entrants)
            .into_iter()
            .map(|g| g.into_iter().map(|x| x as u32).collect())
            .collect();
        let mut t = Tournament {
            season,
            competition: 3 + i as u32,
            region: 0,
            kind: 1,
            phase: 0,
            round: 0,
            participants: entrants.iter().map(|&x| x as u32).collect(),
            groups: gs,
            alive: Vec::new(),
            fixture_ids: Vec::new(),
            champion: None,
        };
        groups(cal, &mut t, c);
        cal.tournaments.push(t);
    }
    if season % 2 == 1 {
        let competition = if season % 4 == 1 { 6 } else { 7 };
        let gs = crate::national_tournament::qualifying_group_partition(state.world_seed, season)
            .into_iter()
            .map(|g| g.into_iter().map(|x| x as u32).collect())
            .collect();
        let mut t = Tournament {
            season,
            competition,
            region: 0,
            kind: 2,
            phase: 0,
            round: 0,
            participants: world.nations.iter().map(|n| n.id as u32).collect(),
            groups: gs,
            alive: Vec::new(),
            fixture_ids: Vec::new(),
            champion: None,
        };
        groups(cal, &mut t, c);
        cal.tournaments.push(t);
    }
    if season.is_multiple_of(2) {
        for (round, (m, d)) in INTERNATIONAL_DATES.into_iter().enumerate() {
            let ids = (0..world.nations.len()).collect::<Vec<_>>();
            let draw = crate::domestic_cup::draw_bracket_round(
                state.world_seed ^ 0x1A71_F81E ^ ((season as u64) << 24) ^ round as u64,
                &ids,
            );
            for (slot, (a, b)) in draw.pairs.into_iter().enumerate() {
                cal.fixtures.push(fixture(
                    season,
                    9,
                    0,
                    0,
                    round as u32,
                    slot as u32,
                    0,
                    date(c, season, m, d),
                    a as u32,
                    b as u32,
                    4,
                ));
            }
        }
    }
    cal.prepared_through = season;
}
fn progress(cal: &mut CompetitionCalendar, c: Chronology, seed: u64) {
    let done = cal
        .results
        .iter()
        .map(|r| (r.fixture.id, r.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut tournaments = std::mem::take(&mut cal.tournaments);
    let mut added = Vec::new();
    for t in &mut tournaments {
        if t.phase == 2 || t.fixture_ids.iter().any(|id| !done.contains_key(id)) {
            continue;
        }
        if t.phase == 0 {
            t.alive = t
                .groups
                .iter()
                .flat_map(|g| {
                    rank_group(cal, g, t.competition, t.season, t.region)
                        .into_iter()
                        .take(2)
                        .collect::<Vec<_>>()
                })
                .collect();
            if t.kind == 2 {
                t.phase = 2;
                let mut pool = t.alive.clone();
                shuffle(&mut pool, seed ^ ((t.season as u64) << 24) ^ 0x0FA1_1001);
                let mut finals = Tournament {
                    kind: 3,
                    region: 1,
                    phase: 0,
                    round: 0,
                    participants: pool.clone(),
                    groups: pool.chunks(4).map(|g| g.to_vec()).collect(),
                    alive: Vec::new(),
                    fixture_ids: Vec::new(),
                    champion: None,
                    ..t.clone()
                };
                groups(cal, &mut finals, c);
                added.push(finals);
                continue;
            }
            t.phase = 1;
            t.round = 0;
            knockout(cal, t, c, seed);
            continue;
        }
        let mut paired = BTreeSet::new();
        let mut winners = Vec::new();
        for id in &t.fixture_ids {
            let r = &done[id];
            if r.fixture.leg == 1 {
                paired.insert(r.fixture.home);
                paired.insert(r.fixture.away);
                let other = t
                    .fixture_ids
                    .iter()
                    .filter_map(|id| done.get(id))
                    .find(|leg| leg.fixture.slot == r.fixture.slot && leg.fixture.leg == 2);
                let totals = other.map_or(r.goals, |leg| {
                    [r.goals[0] + leg.goals[1], r.goals[1] + leg.goals[0]]
                });
                let winner = if totals[0] == totals[1] {
                    let mut rng = GoatRng::new(seed ^ r.fixture.id ^ SHOOTOUT_DOMAIN);
                    if rng.next_range_u32(0, 1) == 0 {
                        r.fixture.home
                    } else {
                        r.fixture.away
                    }
                } else if totals[0] > totals[1] {
                    r.fixture.home
                } else {
                    r.fixture.away
                };
                if let Some(stored) = cal.results.iter_mut().find(|x| x.fixture.id == *id) {
                    stored.winner = Some(winner);
                }
                winners.push(winner);
            }
        }
        winners.extend(t.alive.iter().filter(|id| !paired.contains(id)).copied());
        t.alive = winners;
        if t.alive.len() == 1 {
            t.phase = 2;
            t.champion = t.alive.first().copied();
        } else {
            t.round += 1;
            knockout(cal, t, c, seed);
        }
    }
    tournaments.extend(added);
    cal.tournaments = tournaments;
}
fn shuffle(pool: &mut [u32], seed: u64) {
    let mut rng = GoatRng::new(seed);
    for i in (1..pool.len()).rev() {
        let j = rng.next_range_u32(0, i as u32) as usize;
        pool.swap(i, j);
    }
}
fn coefficient(cal: &mut CompetitionCalendar, world: &WorldGenesis, season: u32) {
    if cal.coefficients.iter().any(|y| y.season == season) {
        return;
    }
    let ts = cal
        .tournaments
        .iter()
        .filter(|t| t.season == season && t.kind == 1)
        .collect::<Vec<_>>();
    if ts.len() != 3 || ts.iter().any(|t| t.phase != 2) {
        return;
    }
    let mut points = vec![0u64; world.nations.len()];
    let mut entrants = vec![0u32; world.nations.len()];
    for t in ts {
        for &club in &t.participants {
            let nation = world.clubs[club as usize].nation;
            entrants[nation] += 1;
            points[nation] += crate::ranking::GROUP_BONUS[(t.competition - 3) as usize] as u64;
        }
        for r in cal
            .results
            .iter()
            .filter(|r| r.fixture.season == season && r.fixture.competition == t.competition)
        {
            let a = world.clubs[r.fixture.home as usize].nation;
            let b = world.clubs[r.fixture.away as usize].nation;
            if r.goals[0] == r.goals[1] {
                points[a] += 1;
                points[b] += 1;
            } else {
                points[if r.goals[0] > r.goals[1] { a } else { b }] += 2;
            }
        }
        // Every bracket advancement (including the final), never shootout-as-match-win.
        for round in 0..=t.round {
            let first = cal.results.iter().filter(|r| {
                r.fixture.season == season
                    && r.fixture.competition == t.competition
                    && r.fixture.stage == 1
                    && r.fixture.round == round
                    && r.fixture.leg == 1
            });
            for r in first {
                if let Some(w) = r.winner {
                    points[world.clubs[w as usize].nation] += 1;
                }
            }
        }
    }
    cal.coefficients.push(CoefficientYear {
        season,
        points: points
            .into_iter()
            .zip(&entrants)
            .map(|(p, &n)| (p * crate::ranking::POINT_SCALE as u64 / n.max(1) as u64) as u32)
            .collect(),
        entrants,
    });
}

/// Actual participation dates serve bans. The new rules keep the frozen v13 path.
fn banned(
    state: &WorldState,
    cal: &CompetitionCalendar,
    pop: &Population,
    idx: usize,
    f: &DatedFixture,
) -> bool {
    let events = state
        .npc_cards
        .iter()
        .filter(|e| e.pop_idx == idx as u32)
        .copied()
        .collect::<Vec<_>>();
    let team = if f.national() {
        pop.nation[idx] as u32
    } else {
        pop.club[idx] as u32
    };
    discipline_banned_for_player(cal, &events, team, f, Some((pop, idx)))
}
fn discipline_banned(
    cal: &CompetitionCalendar,
    events: &[NpcCardEvent],
    team: u32,
    f: &DatedFixture,
) -> bool {
    discipline_banned_for_player(cal, events, team, f, None)
}
fn discipline_banned_for_player(
    cal: &CompetitionCalendar,
    events: &[NpcCardEvent],
    team: u32,
    f: &DatedFixture,
    player: Option<(&Population, usize)>,
) -> bool {
    if !events
        .iter()
        .any(|e| e.competition_id == f.competition && e.epoch_day < f.day)
    {
        return false;
    }
    let clubs = if f.national() {
        vec![team]
    } else if let Some((pop, idx)) = player {
        std::iter::once(
            pop.original_clubs
                .get(idx)
                .copied()
                .unwrap_or(pop.club[idx]) as u32,
        )
        .chain(pop.club_history(idx).iter().map(|&(_, club)| club as u32))
        .collect::<Vec<_>>()
    } else if cal.market_enabled && !cal.pc_affiliations.is_empty() {
        cal.pc_affiliations
            .iter()
            .map(|&(_, club)| club as u32)
            .collect::<Vec<_>>()
    } else {
        vec![team]
    };
    let mut dates = cal
        .results
        .iter()
        .map(|r| &r.fixture)
        .chain(cal.fixtures.iter())
        .filter(|other| {
            if other.competition != f.competition
                || other.day >= f.day
                || !clubs
                    .iter()
                    .any(|&club| other.home == club || other.away == club)
            {
                return false;
            }
            let registered = if other.national() {
                Some(team)
            } else {
                player.map_or_else(
                    || {
                        if cal.market_enabled && !cal.pc_affiliations.is_empty() {
                            let n = cal
                                .pc_affiliations
                                .partition_point(|&(day, _)| day <= other.day);
                            n.checked_sub(1).map(|n| cal.pc_affiliations[n].1 as u32)
                        } else {
                            Some(team)
                        }
                    },
                    |(pop, idx)| pop.club_at(idx, other.day).map(u32::from),
                )
            };
            other.competition == f.competition
                && registered.is_some_and(|club| other.home == club || other.away == club)
        })
        .map(|other| other.day)
        .collect::<Vec<_>>();
    dates.sort_unstable();
    dates.dedup();
    let threshold = if f.competition == LEAGUE {
        5
    } else if matches!(f.competition, 3..=5) {
        3
    } else {
        2
    };
    let reset = cal
        .tournaments
        .iter()
        .find(|t| t.season == f.season && t.competition == f.competition && matches!(t.kind, 1 | 3))
        .and_then(|t| {
            let mut size = t.groups.len() * 2;
            let mut reset_round = 0;
            while size > 4 {
                size = size.div_ceil(2);
                reset_round += 1;
            }
            cal.fixtures
                .iter()
                .chain(cal.results.iter().map(|r| &r.fixture))
                .filter(|other| {
                    other.season == f.season
                        && other.competition == f.competition
                        && other.stage == 1
                        && other.round == reset_round
                })
                .map(|other| other.day)
                .min()
        });
    goat_core::discipline::status_with_rules(
        events,
        f.season,
        f.day,
        f.competition,
        &dates,
        goat_core::discipline::DisciplineRules {
            yellow_threshold: if matches!(f.competition, 8 | 9) {
                u32::MAX
            } else {
                threshold
            },
            direct_red_games: if matches!(f.competition, 8 | 9) { 1 } else { 3 },
        },
        reset,
    )
    .ban_games
        > 0
}
/// Lazy contact: detail only the two imminent teams, retaining permanent league scope.
pub fn match_roster(
    session: &mut SimulationSession,
    state: &WorldState,
    f: &DatedFixture,
) -> MatchRoster {
    session.population_deep(state);
    let world = session
        .deep_world()
        .unwrap_or_else(|| WorldGenesis::generate(state.world_seed));
    let pop = session.population_deep(state);
    roster_from_population(pop, state, &world, f)
}
fn roster_from_population(
    pop: &Population,
    state: &WorldState,
    world: &WorldGenesis,
    f: &DatedFixture,
) -> MatchRoster {
    let cal = state.competition_calendar.as_ref().unwrap();
    let week = f.day / 7;
    let teams = [f.home, f.away].map(|team| {
        let mut ids = (0..pop.len())
            .filter(|&i| {
                if f.national() {
                    pop.nation[i] as u32 == team
                } else {
                    pop.club_at(i, f.day).map(u32::from) == Some(team)
                }
            })
            .collect::<Vec<_>>();
        if f.national() {
            ids.sort_by_key(|&i| (std::cmp::Reverse(pop.current_ovr(i, week)), i));
            let keepers = ids
                .iter()
                .filter(|&&i| pop.goalkeeper[i])
                .take(3)
                .copied()
                .collect::<Vec<_>>();
            let mut selected = ids
                .iter()
                .filter(|&&i| !pop.goalkeeper[i])
                .take(20)
                .copied()
                .collect::<Vec<_>>();
            selected.extend(keepers);
            ids = selected;
        }
        ids.into_iter()
            .map(|idx| {
                let attrs = pop.shared_view(idx, week).current;
                RosterPlayer {
                    id: idx as u32,
                    name: crate::history::name_from_seed(pop.seed[idx]),
                    attrs,
                    position: pop.position[idx],
                    keeper: pop.goalkeeper[idx],
                    rating: pop.current_ovr(idx, week),
                    form: pop.form[idx] as i32,
                    keeping: pop.goalkeeper_skill(idx, week),
                    energy: pop.match_energy(idx, f.day),
                    aggression: attrs[goat_core::attrs::AttrId::Aggression as usize]
                        .to_int()
                        .clamp(1, 99) as u8,
                    returning: pop.medical_status(idx, week).is_some_and(|m| {
                        matches!(m.phase, goat_core::medical::RecoveryPhase::Returning { .. })
                    }),
                    banned: !pop.is_available(idx, week) || banned(state, cal, pop, idx, f),
                }
            })
            .collect()
    });
    let pc_position = state.pc_player_id.map_or(2, |id| {
        state.players.get_primary_position(id).family() as u8
    });
    let pc_energy = state.pc_player_id.map_or(0, |id| {
        state.players.get_energy(id).to_int().clamp(0, 100) as u8
    });
    let pc_eligible = state
        .pc_player_id
        .is_some_and(|id| state.players.get_injury_weeks(id) == 0)
        && state.pc_suspension_matches_remaining(f.competition) == 0
        && !discipline_banned(
            cal,
            &cal.pc_cards,
            if pc_side(state, world, f) == Some(0) {
                f.home
            } else {
                f.away
            },
            f,
        );
    MatchRoster {
        profiles: [f.home, f.away].map(|team| {
            TacticalProfile::derive(
                if f.national() {
                    world.nations[team as usize].stature
                } else {
                    world.clubs[team as usize].strength
                },
                team,
                state.world_seed,
            )
        }),
        pc_side: pc_side(state, world, f),
        pc_position,
        pc_energy,
        pc_returning: state.pc_medical_status().is_some_and(|m| {
            matches!(m.phase, goat_core::medical::RecoveryPhase::Returning { .. })
        }),
        pc_eligible,
        pc_rating: state.pc_player_id.map_or(1, |id| {
            goat_core::derive::ovr(
                &state.pc_display_view().current,
                state.players.get_primary_position(id),
            )
            .to_int()
            .clamp(1, 99) as u8
        }),
        fixture: f.clone(),
        teams,
        cards: Vec::new(),
    }
}
fn detailed(
    pop: &Population,
    state: &WorldState,
    world: &WorldGenesis,
    f: &DatedFixture,
) -> (
    FixtureResult,
    Vec<NpcMatchCredit>,
    Vec<NpcMatchLoad>,
    Vec<NpcCardEvent>,
) {
    let roster = roster_from_population(pop, state, world, f);
    let candidates = roster.teams.each_ref().map(|team| {
        team.iter()
            .map(|p| crate::npc_match::Candidate {
                player: p.id,
                position: p.position,
                keeper: p.keeper,
                rating: p.rating,
                keeping: p.keeping,
                energy: p.energy,
                stamina: p.attrs[goat_core::attrs::AttrId::Stamina as usize]
                    .to_int()
                    .clamp(1, 99) as u8,
                aggression: p.aggression,
                returning: p.returning,
                banned: p.banned,
            })
            .collect::<Vec<_>>()
    });
    let profiles = [f.home, f.away].map(|team| {
        TacticalProfile::derive(
            if f.national() {
                world.nations[team as usize].stature
            } else {
                world.clubs[team as usize].strength
            },
            team,
            state.world_seed,
        )
    });
    let mut rng = GoatRng::new(state.world_seed ^ f.id ^ MATCH_DOMAIN);
    let detail = crate::npc_match::simulate(
        [&candidates[0], &candidates[1]],
        profiles,
        Default::default(),
        &mut rng,
    );
    let outcome = detail.goals[0].cmp(&detail.goals[1]) as i8;
    let credits = detail
        .appearances
        .iter()
        .enumerate()
        .flat_map(|(side, apps)| {
            apps.iter().map(move |a| NpcMatchCredit {
                pop_idx: a.player,
                goals: a.goals,
                assists: a.assists,
                result: if side == 0 { outcome } else { -outcome },
            })
        })
        .collect();
    let mut loads = Vec::new();
    for (side, players) in roster.teams.iter().enumerate() {
        for p in players {
            loads.push(NpcMatchLoad {
                pop_idx: p.id,
                competition_id: f.competition,
                fixture_id: f.workload_id,
                epoch_day: f.day,
                minutes: detail.appearances[side]
                    .iter()
                    .find(|a| a.player == p.id)
                    .map_or(0, |a| a.minutes),
            });
        }
    }
    let cards = detail
        .cards
        .iter()
        .map(|card| NpcCardEvent {
            season: f.season,
            competition_id: f.competition,
            pop_idx: card.player,
            fixture_id: f.workload_id,
            epoch_day: f.day,
            minute: card.minute,
            kind: card.kind,
        })
        .collect();
    (
        FixtureResult {
            fixture: f.clone(),
            goals: detail.goals,
            winner: None,
            detailed: true,
        },
        credits,
        loads,
        cards,
    )
}
fn is_deep(state: &WorldState, world: &WorldGenesis, f: &DatedFixture) -> bool {
    f.national()
        || state
            .deep_scopes
            .iter()
            .rev()
            .find(|s| s.epoch_day <= f.day)
            .is_some_and(|scope| {
                scope.leagues.iter().any(|&l| {
                    world.leagues[l as usize].clubs.contains(&(f.home as usize))
                        || world.leagues[l as usize].clubs.contains(&(f.away as usize))
                })
            })
}
impl SimulationSession {
    /// Prepare the next season once; previous national finals remain pending in July.
    pub fn prepare_competitions(
        &mut self,
        mut state: WorldState,
        world: &WorldGenesis,
    ) -> Result<WorldState, CalendarError> {
        let c = state.chronology().ok_or(CalendarError::InvalidClock)?;
        let existing = state
            .competition_calendar
            .as_ref()
            .ok_or(CalendarError::MissingCalendar)?;
        if state.season_number == 0 || state.season_number > 1000 {
            return Err(CalendarError::InvalidClock);
        }
        if existing.prepared_through >= state.season_number {
            return Ok(state);
        }
        if existing.prepared_through + 1 != state.season_number {
            return Err(CalendarError::InvalidProgress);
        }
        let mut cal = existing.clone();
        self.population_deep(&state);
        let actual = self.deep_world().unwrap_or_else(|| world.clone());
        let scores = self.league_scores(&state);
        let chosen = crate::deep::select_leagues(
            &actual,
            state.pc_div_idx as usize,
            self.permanent_top_leagues(),
            &scores,
        );
        let scope = DeepScope {
            season: state.season_number,
            epoch_day: c.frame(state.season_number).preparation_start,
            pc_league: state.pc_div_idx as u32,
            pc_club: state.pc_club_idx as u32,
            leagues: chosen.iter().map(|&l| l as u32).collect(),
        };
        state = reduce(
            state,
            Intent::SelectDeepScope { scope },
            &mut GoatRng::new(0),
        );
        prepare(&mut cal, &actual, &state, c);
        resolve_schedule(&mut cal, c)?;
        Ok(reduce(
            state,
            Intent::SetCompetitionCalendar { calendar: cal },
            &mut GoatRng::new(0),
        ))
    }
    /// Resolve through a date in chronological order, without a renderer or future rolls.
    /// This NPC-only entry point is also used when the PC cannot participate.
    pub fn advance_competitions(
        &mut self,
        state: WorldState,
        world: &WorldGenesis,
        through: u32,
    ) -> Result<WorldState, CalendarError> {
        self.advance_competitions_internal(state, world, through, false)
    }
    /// Stop before the next PC club/national fixture; all earlier NPC games still resolve.
    pub fn advance_until_pc_fixture(
        &mut self,
        state: WorldState,
        world: &WorldGenesis,
    ) -> Result<WorldState, CalendarError> {
        let end = state
            .chronology()
            .ok_or(CalendarError::InvalidClock)?
            .frame(state.season_number)
            .end_day;
        self.advance_competitions_internal(state, world, end, true)
    }
    fn advance_competitions_internal(
        &mut self,
        mut state: WorldState,
        world: &WorldGenesis,
        through: u32,
        stop_for_pc: bool,
    ) -> Result<WorldState, CalendarError> {
        let c = state.chronology().ok_or(CalendarError::InvalidClock)?;
        if through < state.pc_epoch_day
            || through >= c.frame(state.season_number).next_preparation_start
        {
            return Err(CalendarError::InvalidClock);
        }
        state = self.prepare_competitions(state, world)?;
        loop {
            let next = state
                .competition_calendar
                .as_ref()
                .unwrap()
                .fixtures
                .first()
                .cloned();
            let Some(f) = next.filter(|f| f.day <= through) else {
                break;
            };
            if f.day < state.pc_epoch_day {
                return Err(CalendarError::InvalidClock);
            }
            state = reduce(
                state,
                Intent::AdvanceToDate {
                    epoch_day: f.day,
                    train: false,
                },
                &mut GoatRng::new(0),
            );
            state.pc_epoch_day = f.day;
            let today = state
                .competition_calendar
                .as_ref()
                .unwrap()
                .fixtures
                .iter()
                .take_while(|p| p.day == f.day)
                .cloned()
                .collect::<Vec<_>>();
            if stop_for_pc
                && today
                    .iter()
                    .any(|fixture| pc_side(&state, world, fixture).is_some())
            {
                return Ok(state);
            }

            self.population_deep(&state);
            let actual = self.deep_world().unwrap_or_else(|| world.clone());
            let pop = self.population_deep(&state);
            let mut pending = Vec::new();
            for f in today {
                let outcome = if is_deep(&state, &actual, &f) {
                    detailed(pop, &state, &actual, &f)
                } else {
                    light(&state, &actual, &f)
                };
                pending.push((f, outcome));
            }
            let mut doses = Vec::new();
            let mut all_cards = Vec::new();
            for (f, (result, credits, loads, cards)) in pending {
                doses.extend(loads);
                all_cards.extend(cards);
                state = commit(
                    state,
                    &actual,
                    &f,
                    (result, credits, Vec::new(), Vec::new()),
                )?;
            }
            if !doses.is_empty() {
                state = reduce(
                    state,
                    Intent::RecordNpcMatchLoads { loads: doses },
                    &mut GoatRng::new(0),
                );
            }
            if !all_cards.is_empty() {
                state = reduce(
                    state,
                    Intent::RecordNpcCards { cards: all_cards },
                    &mut GoatRng::new(0),
                );
            }
            state = settle_progress(state, &actual, c)?;
        }
        state = reduce(
            state,
            Intent::AdvanceToDate {
                epoch_day: through,
                train: false,
            },
            &mut GoatRng::new(0),
        );
        state.pc_epoch_day = through;
        Ok(state)
    }
    /// Commit exactly one resolved match. External PC engines supply their real residue.
    /// Duplicate fixture commits are no-ops, including after loading a save.
    #[allow(clippy::type_complexity)]
    pub fn resolve_competition_fixture(
        &mut self,
        mut state: WorldState,
        world: &WorldGenesis,
        id: u64,
        external: Option<(
            FixtureResult,
            Vec<NpcMatchCredit>,
            Vec<NpcMatchLoad>,
            Vec<NpcCardEvent>,
        )>,
    ) -> Result<WorldState, CalendarError> {
        let c = state.chronology().ok_or(CalendarError::InvalidClock)?;
        let cal = state
            .competition_calendar
            .as_ref()
            .ok_or(CalendarError::MissingCalendar)?;
        if cal.results.iter().any(|r| r.fixture.id == id) {
            return Ok(state);
        }
        let f = cal
            .fixtures
            .iter()
            .find(|f| f.id == id)
            .cloned()
            .ok_or(CalendarError::InvalidProgress)?;
        if f.day != state.pc_epoch_day {
            return Err(CalendarError::InvalidClock);
        }
        let actual = {
            self.population_deep(&state);
            self.deep_world().unwrap_or_else(|| world.clone())
        };
        let outcome = if let Some(x) = external {
            if x.0.fixture != f
                || x.2.iter().any(|l| {
                    l.epoch_day != f.day
                        || l.competition_id != f.competition
                        || l.fixture_id != f.workload_id
                })
            {
                return Err(CalendarError::InvalidProgress);
            }
            x
        } else if is_deep(&state, &actual, &f) {
            detailed(self.population_deep(&state), &state, &actual, &f)
        } else {
            let mut rng = GoatRng::new(state.world_seed ^ f.id ^ MATCH_DOMAIN);
            let goals = crate::season::sim_team_match_shared(
                actual.clubs[f.home as usize].strength,
                actual.clubs[f.away as usize].strength,
                &mut rng,
            );
            (
                FixtureResult {
                    fixture: f.clone(),
                    goals: [goals.0, goals.1],
                    winner: None,
                    detailed: false,
                },
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        };
        state = commit(state, &actual, &f, outcome)?;
        settle_progress(state, &actual, c)
    }
}

/// Pure schedule lookup; it never resolves a match or changes a bracket.
pub fn next_pc_fixture<'a>(
    state: &'a WorldState,
    world: &WorldGenesis,
) -> Option<&'a DatedFixture> {
    state
        .competition_calendar
        .as_ref()?
        .fixtures
        .iter()
        .filter(|f| pc_side(state, world, f).is_some())
        .min_by_key(|f| (f.day, std::cmp::Reverse(f.priority), f.id))
}
fn pc_side(state: &WorldState, world: &WorldGenesis, f: &DatedFixture) -> Option<u8> {
    let team = if f.national() {
        world
            .nations
            .iter()
            .find(|n| world.nation_name(n.id) == state.pc_nationality)?
            .id as u32
    } else {
        state.pc_club_idx as u32
    };
    if f.home == team {
        Some(0)
    } else if f.away == team {
        Some(1)
    } else {
        None
    }
}
impl SimulationSession {
    /// Commit one finished PC fixture exactly once, including personal health and stats.
    pub fn apply_dated_pc_match(
        &mut self,
        state: WorldState,
        world: &WorldGenesis,
        receipt: DatedMatchReceipt,
    ) -> Result<WorldState, CalendarError> {
        let id = receipt.result.fixture.id;
        if state
            .competition_calendar
            .as_ref()
            .is_some_and(|cal| cal.results.iter().any(|r| r.fixture.id == id))
        {
            return Ok(state);
        }
        let f = &receipt.result.fixture;
        if pc_side(&state, world, f).is_none()
            || receipt.pc_minutes > 90
            || receipt.pc_goals > receipt.result.goals.iter().sum::<u32>()
        {
            return Err(CalendarError::InvalidProgress);
        }
        let roster = match_roster(self, &state, f);
        let players = roster
            .teams
            .iter()
            .flatten()
            .map(|p| p.id)
            .collect::<BTreeSet<_>>();
        let loads = receipt
            .loads
            .iter()
            .map(|l| l.pop_idx)
            .collect::<BTreeSet<_>>();
        let credits = receipt
            .credits
            .iter()
            .map(|c| c.pop_idx)
            .collect::<BTreeSet<_>>();
        if loads != players
            || loads.len() != receipt.loads.len()
            || credits.len() != receipt.credits.len()
            || receipt.loads.iter().any(|l| l.minutes > 90)
            || receipt.credits.iter().any(|c| {
                !players.contains(&c.pop_idx)
                    || receipt
                        .loads
                        .iter()
                        .all(|l| l.pop_idx != c.pop_idx || l.minutes == 0)
            })
            || receipt.cards.iter().any(|c| {
                !players.contains(&c.pop_idx)
                    || c.fixture_id != f.workload_id
                    || c.epoch_day != f.day
                    || c.competition_id != f.competition
                    || c.kind > 2
                    || c.minute > 90
            })
            || receipt.pc_cards.iter().any(|c| {
                c.pop_idx != u32::MAX
                    || c.fixture_id != f.workload_id
                    || c.epoch_day != f.day
                    || c.competition_id != f.competition
                    || c.kind > 2
                    || c.minute > 90
            })
            || receipt.credits.iter().map(|c| c.goals as u32).sum::<u32>() + receipt.pc_goals
                != receipt.result.goals.iter().sum::<u32>()
        {
            return Err(CalendarError::InvalidProgress);
        }
        let state = self.resolve_competition_fixture(
            state,
            world,
            id,
            Some((
                receipt.result.clone(),
                receipt.credits.clone(),
                receipt.loads.clone(),
                receipt.cards.clone(),
            )),
        )?;
        Ok(reduce(
            state,
            Intent::RecordDatedPcMatch { receipt },
            &mut GoatRng::new(0),
        ))
    }
    /// Settle June once, progress annual core state, and keep unfinished July tournaments.
    pub fn start_next_competition_season(
        &mut self,
        mut state: WorldState,
        world: &WorldGenesis,
    ) -> Result<WorldState, CalendarError> {
        let c = state.chronology().ok_or(CalendarError::InvalidClock)?;
        let season = state.season_number;
        let frame = c.frame(season);
        if state.pc_epoch_day > frame.end_day {
            return Err(CalendarError::InvalidClock);
        }
        state = self.advance_competitions(state, world, frame.end_day)?;
        if state
            .competition_calendar
            .as_ref()
            .unwrap()
            .fixtures
            .iter()
            .any(|f| f.season == season && !f.national())
        {
            return Err(CalendarError::InvalidProgress);
        }
        let clubs = state
            .competition_calendar
            .as_ref()
            .unwrap()
            .results
            .iter()
            .filter(|r| {
                r.fixture.season == season
                    && r.fixture.competition == LEAGUE
                    && r.fixture.region == state.pc_div_idx as u32
            })
            .flat_map(|r| [r.fixture.home as usize, r.fixture.away as usize])
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let table = table_for(
            state.competition_calendar.as_ref().unwrap(),
            &clubs,
            LEAGUE,
            season,
        );
        let position = table.position_of(state.pc_club_idx as usize) as u32;
        let legacy = Intent::ApplySeasonEndLegacy {
            season_goals: state.pc_season_goals,
            season_assists: state.pc_season_assists,
            season_matches: state.pc_season_matches,
            season_output_sum: state.pc_season_output,
            won_title: position == 1,
            player_of_year: false,
            finish_position: position,
            decisive_moments: state.pc_season_decisive_moments,
            season_clutch_index: state.pc_season_clutch_index,
            new_sporting_rep: state.pc_sporting_rep,
            new_club_fan_rep: state.pc_club_fan_rep,
            season_standout_matches: state.pc_season_standout_matches,
            season_transfer_requests: state.pc_season_transfer_requests,
            season_caps: state.pc_season_caps,
            season_international_goals: state.pc_season_international_goals,
            season_world_cups_won: state.pc_season_world_cups_won,
            season_continental_championships_won: state.pc_season_continental_championships_won,
        };
        if state.pc_seasons_played < season {
            if state
                .competition_calendar
                .as_ref()
                .is_some_and(|cal| cal.market_enabled)
            {
                state = reduce(state, Intent::CollectWage, &mut GoatRng::new(0));
            }
            state = reduce(state, legacy, &mut GoatRng::new(0));
        }
        state = reduce(
            state,
            Intent::AdvanceToDate {
                epoch_day: frame.next_preparation_start,
                train: false,
            },
            &mut GoatRng::new(0),
        );
        state.pc_epoch_day = frame.next_preparation_start;
        let closed = self
            .finish_deep_season(&state)
            .ok_or(CalendarError::InvalidProgress)?;
        if let Some(league) = closed
            .membership
            .iter()
            .position(|clubs| clubs.contains(&(state.pc_club_idx as usize)))
        {
            state.pc_div_idx = league as u8;
        }
        state = reduce(
            state,
            Intent::StartSeason {
                fixtures: Vec::new(),
            },
            &mut GoatRng::new(0),
        );
        if let Some(end) = state
            .competition_calendar
            .as_ref()
            .filter(|cal| cal.market_enabled)
            .and_then(|cal| cal.pc_contract_end)
        {
            state.pc_contract_seasons_left = c
                .planning_season(end)
                .saturating_sub(c.planning_season(state.pc_epoch_day));
        }
        self.prepare_competitions(state, world)
    }
}

type ResolvedMatch = (
    FixtureResult,
    Vec<NpcMatchCredit>,
    Vec<NpcMatchLoad>,
    Vec<NpcCardEvent>,
);
fn light(state: &WorldState, world: &WorldGenesis, f: &DatedFixture) -> ResolvedMatch {
    let mut rng = GoatRng::new(state.world_seed ^ f.id ^ MATCH_DOMAIN);
    let goals = crate::season::sim_team_match_shared(
        world.clubs[f.home as usize].strength,
        world.clubs[f.away as usize].strength,
        &mut rng,
    );
    (
        FixtureResult {
            fixture: f.clone(),
            goals: [goals.0, goals.1],
            winner: None,
            detailed: false,
        },
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}
fn commit(
    mut state: WorldState,
    actual: &WorldGenesis,
    f: &DatedFixture,
    outcome: ResolvedMatch,
) -> Result<WorldState, CalendarError> {
    let (mut result, credits, loads, cards) = outcome;
    let single_leg = f.competition == DOMESTIC
        || f.national()
        || state
            .competition_calendar
            .as_ref()
            .unwrap()
            .tournaments
            .iter()
            .any(|t| {
                t.season == f.season
                    && t.competition == f.competition
                    && t.region == f.region
                    && t.alive.len() == 2
            });
    if f.stage == 1 && single_leg {
        result.winner = Some(if result.goals[0] == result.goals[1] {
            let mut rng = GoatRng::new(state.world_seed ^ f.id ^ SHOOTOUT_DOMAIN);
            if rng.next_range_u32(0, 1) == 0 {
                f.home
            } else {
                f.away
            }
        } else if result.goals[0] > result.goals[1] {
            f.home
        } else {
            f.away
        });
    }
    if !credits.is_empty() {
        let div = if f.national() {
            state.pc_div_idx
        } else {
            actual
                .leagues
                .iter()
                .find(|l| l.clubs.contains(&(f.home as usize)))
                .map_or(state.pc_div_idx, |l| l.id as u8)
        };
        let season = state.season_number;
        state = reduce(
            state,
            Intent::RecordOrbitMatch {
                record: OrbitMatchRecord {
                    season,
                    round: f.record_round(),
                    div,
                    credits,
                },
            },
            &mut GoatRng::new(0),
        );
    }
    if f.competition == LEAGUE {
        let scored = DeepFixtureResult {
            season: f.season,
            league: f.region,
            round: f.round,
            epoch_day: f.day,
            home: f.home,
            away: f.away,
            home_goals: result.goals[0],
            away_goals: result.goals[1],
        };
        state = reduce(
            state,
            Intent::RecordDeepFixture { result: scored },
            &mut GoatRng::new(0),
        );
    }
    if !loads.is_empty() {
        state = reduce(
            state,
            Intent::RecordNpcMatchLoads { loads },
            &mut GoatRng::new(0),
        );
    }
    if !cards.is_empty() {
        state = reduce(
            state,
            Intent::RecordNpcCards { cards },
            &mut GoatRng::new(0),
        );
    }
    let mut cal = state.competition_calendar.take().unwrap();
    cal.fixtures.retain(|pending| pending.id != f.id);
    cal.results.push(result);
    cal.resolved_day = cal.resolved_day.max(f.day);
    Ok(reduce(
        state,
        Intent::SetCompetitionCalendar { calendar: cal },
        &mut GoatRng::new(0),
    ))
}
fn settle_progress(
    mut state: WorldState,
    actual: &WorldGenesis,
    c: Chronology,
) -> Result<WorldState, CalendarError> {
    let mut cal = state.competition_calendar.take().unwrap();
    let before = cal.fixtures.len();
    progress(&mut cal, c, state.world_seed);
    for season in state.season_number.saturating_sub(1).max(1)..=state.season_number {
        coefficient(&mut cal, actual, season);
    }
    if cal.fixtures.len() != before {
        resolve_schedule(&mut cal, c)?;
    }
    Ok(reduce(
        state,
        Intent::SetCompetitionCalendar { calendar: cal },
        &mut GoatRng::new(0),
    ))
}

/// Bounded storage codec shared by the save container and exact checkpoint binding.
pub fn encode_calendar(cal: &CompetitionCalendar) -> Vec<u8> {
    use crate::checkpoint::Snapshot;
    let mut out = Vec::new();
    cal.write(&mut out);
    out
}
pub fn decode_calendar(bytes: &[u8]) -> Option<CompetitionCalendar> {
    use crate::checkpoint::{Reader, Snapshot};
    if bytes.len() > crate::checkpoint::MAX_BYTES {
        return None;
    }
    let mut reader = Reader::new(bytes);
    let cal = CompetitionCalendar::read(&mut reader)?;
    if reader.remaining() != 0 || !valid_calendar(&cal) {
        return None;
    }
    Some(cal)
}
pub fn valid_calendar(cal: &CompetitionCalendar) -> bool {
    if cal.pc_affiliations.windows(2).any(|v| v[0].0 >= v[1].0)
        || cal
            .pc_affiliations
            .iter()
            .any(|&(_, club)| club as usize >= crate::world::NUM_CLUBS)
        || (!cal.market_enabled
            && (!cal.pc_affiliations.is_empty() || cal.pc_contract_end.is_some()))
    {
        return false;
    }
    let all = cal
        .fixtures
        .iter()
        .chain(cal.results.iter().map(|r| &r.fixture))
        .collect::<Vec<_>>();
    let ids = all.iter().map(|f| f.id).collect::<BTreeSet<_>>();
    cal.fixtures.windows(2).all(|pair| {
        (pair[0].day, std::cmp::Reverse(pair[0].priority), pair[0].id)
            <= (pair[1].day, std::cmp::Reverse(pair[1].priority), pair[1].id)
    }) && cal
        .pc_played_fixture_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .len()
        == cal.pc_played_fixture_ids.len()
        && cal
            .pc_played_fixture_ids
            .iter()
            .all(|id| cal.results.iter().any(|r| r.fixture.id == *id))
        && cal.pc_cards.iter().all(|e| {
            e.pop_idx == u32::MAX
                && e.kind <= 2
                && e.minute <= 90
                && cal.results.iter().any(|r| {
                    r.fixture.workload_id == e.fixture_id
                        && r.fixture.season == e.season
                        && r.fixture.competition == e.competition_id
                        && r.fixture.day == e.epoch_day
                })
        })
        && ids.len() == all.len()
        && cal.prepared_through <= 1000
        && all.iter().all(|f| {
            f.season > 0
                && f.season <= cal.prepared_through
                && (1..=9).contains(&f.competition)
                && f.home != f.away
                && f.home
                    < if f.national() {
                        crate::world::NUM_NATIONS as u32
                    } else {
                        crate::world::NUM_CLUBS as u32
                    }
                && f.away
                    < if f.national() {
                        crate::world::NUM_NATIONS as u32
                    } else {
                        crate::world::NUM_CLUBS as u32
                    }
                && f.stage <= 1
                && f.leg <= 2
                && f.round < 256
                && f.region < 256
                && f.original_day <= f.day
                && f.id
                    == fixture(
                        f.season,
                        f.competition,
                        f.region,
                        f.stage,
                        f.round,
                        f.slot,
                        f.leg,
                        f.original_day,
                        f.home,
                        f.away,
                        f.priority,
                    )
                    .id
                && f.priority <= 5
                && f.slot < 256
                && f.day < (1 << 22)
        })
        && cal.tournaments.iter().all(|t| {
            let limit = if matches!(t.kind, 2 | 3) {
                crate::world::NUM_NATIONS as u32
            } else {
                crate::world::NUM_CLUBS as u32
            };
            let entrants = t.participants.iter().copied().collect::<BTreeSet<_>>();
            t.phase <= 2
                && t.kind <= 3
                && t.season > 0
                && t.season <= cal.prepared_through
                && matches!(
                    (t.kind, t.competition),
                    (0, 2) | (1, 3..=5) | (2..=3, 6..=7)
                )
                && entrants.len() == t.participants.len()
                && entrants.len() >= 2
                && entrants.iter().all(|&id| id < limit)
                && t.alive.iter().all(|id| entrants.contains(id))
                && t.alive.iter().copied().collect::<BTreeSet<_>>().len() == t.alive.len()
                && t.groups.iter().all(|g| {
                    g.len() >= 2
                        && g.iter().all(|id| entrants.contains(id))
                        && g.iter().copied().collect::<BTreeSet<_>>().len() == g.len()
                })
                && t.champion
                    .is_none_or(|id| entrants.contains(&id) && t.phase == 2)
                && t.fixture_ids.iter().copied().collect::<BTreeSet<_>>().len()
                    == t.fixture_ids.len()
                && t.fixture_ids.iter().all(|id| {
                    all.iter().any(|f| {
                        f.id == *id
                            && f.season == t.season
                            && f.competition == t.competition
                            && f.region == t.region
                    })
                })
        })
        && cal.coefficients.iter().all(|y| {
            y.season > 0
                && y.season <= cal.prepared_through
                && y.points.len() == crate::world::NUM_NATIONS
                && y.entrants.len() == crate::world::NUM_NATIONS
        })
        && cal
            .coefficients
            .iter()
            .map(|y| y.season)
            .collect::<BTreeSet<_>>()
            .len()
            == cal.coefficients.len()
        && cal.results.iter().all(|r| {
            r.goals.iter().all(|&g| g <= 100)
                && r.winner
                    .is_none_or(|w| w == r.fixture.home || w == r.fixture.away)
        })
}
crate::checkpoint::fields!(DatedFixture {
    id,
    workload_id,
    season,
    competition,
    region,
    round,
    slot,
    stage,
    leg,
    original_day,
    day,
    home,
    away,
    priority
});
crate::checkpoint::fields!(FixtureResult {
    fixture,
    goals,
    winner,
    detailed
});
crate::checkpoint::fields!(Tournament {
    season,
    competition,
    region,
    kind,
    phase,
    round,
    participants,
    groups,
    alive,
    fixture_ids,
    champion
});
crate::checkpoint::fields!(CoefficientYear {
    season,
    points,
    entrants
});
crate::checkpoint::fields!(CompetitionCalendar {
    prepared_through,
    resolved_day,
    fixtures,
    results,
    tournaments,
    coefficients,
    pc_cards,
    pc_played_fixture_ids,
    market_enabled,
    pc_affiliations,
    pc_contract_end
});

/// Layout29 has no market flag; preserve its frozen annual roster behavior.
pub fn decode_calendar_v14(bytes: &[u8]) -> Option<CompetitionCalendar> {
    if bytes.len() >= crate::checkpoint::MAX_BYTES {
        return None;
    }
    let mut migrated = bytes.to_vec();
    migrated.push(0); // market_enabled
    migrated.extend_from_slice(&0u32.to_le_bytes()); // PC affiliations
    migrated.push(0); // contract_end = None
    decode_calendar(&migrated)
}

impl SimulationSession {
    /// Validate an accepted PC transfer against dated registration and live membership.
    /// Offer negotiation remains an existing meta policy; renderers supply the acceptance.
    pub fn transfer_pc_on_date(
        &mut self,
        mut state: WorldState,
        world: &WorldGenesis,
        intent: Intent,
    ) -> Result<WorldState, CalendarError> {
        let c = state.chronology().ok_or(CalendarError::InvalidClock)?;
        if !crate::market::registration_open(c, state.pc_epoch_day) {
            return Err(CalendarError::InvalidClock);
        }
        let Intent::ExecuteTransfer {
            to_club_idx,
            new_wage,
            new_length,
            fee_bonus,
            ..
        } = intent
        else {
            return Err(CalendarError::InvalidProgress);
        };
        if new_length == 0 || to_club_idx as usize >= world.clubs.len() {
            return Err(CalendarError::InvalidProgress);
        }
        self.population_deep(&state);
        let actual = self.deep_world().ok_or(CalendarError::InvalidProgress)?;
        let club = &actual.clubs[to_club_idx as usize];
        let league = actual
            .leagues
            .iter()
            .find(|l| l.clubs.contains(&(to_club_idx as usize)))
            .ok_or(CalendarError::InvalidProgress)?
            .id;
        state = reduce(
            state,
            Intent::ExecuteTransfer {
                to_club_idx,
                to_div_idx: league as u8,
                new_wage,
                new_length,
                new_club_name: club.name.clone(),
                facilities_mult: club.facilities_mult(),
                staff_mods: crate::staff::club_staff_mods(club.strength),
                fee_bonus,
            },
            &mut GoatRng::new(0),
        );
        let chosen = crate::deep::select_leagues(
            &actual,
            league,
            self.permanent_top_leagues(),
            &self.league_scores(&state),
        );
        let scope = DeepScope {
            season: state.season_number,
            epoch_day: state.pc_epoch_day,
            pc_league: league as u32,
            pc_club: to_club_idx as u32,
            leagues: chosen.iter().map(|&l| l as u32).collect(),
        };
        Ok(reduce(
            state,
            Intent::SelectDeepScope { scope },
            &mut GoatRng::new(0),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rest_conflicts_keep_identity_and_resolved_leg_date() {
        let c = Chronology::new(2023);
        let day = date(c, 1, 2, 5);
        let first = fixture(1, 3, 0, 1, 0, 0, 1, day, 0, 1, 4);
        let mut cal = CompetitionCalendar {
            prepared_through: 1,
            resolved_day: day,
            results: vec![FixtureResult {
                fixture: first.clone(),
                goals: [1, 0],
                winner: None,
                detailed: true,
            }],
            fixtures: vec![
                fixture(1, 3, 0, 1, 0, 0, 2, day + 1, 1, 0, 4),
                fixture(1, 1, 0, 0, 1, 0, 0, day + 1, 0, 2, 1),
            ],
            ..Default::default()
        };
        let ids = cal.fixtures.iter().map(|f| f.id).collect::<BTreeSet<_>>();
        resolve_schedule(&mut cal, c).unwrap();
        assert_eq!(cal.results[0].fixture, first);
        assert_eq!(
            cal.fixtures.iter().map(|f| f.id).collect::<BTreeSet<_>>(),
            ids
        );
        let second = cal.fixtures.iter().find(|f| f.leg == 2).unwrap();
        let league = cal.fixtures.iter().find(|f| f.competition == 1).unwrap();
        assert_eq!(second.day, day + 3);
        assert!(league.day.abs_diff(second.day) >= 3);
        let once = cal.clone();
        resolve_schedule(&mut cal, c).unwrap();
        assert_eq!(cal, once);
        assert_eq!(decode_calendar(&encode_calendar(&cal)), Some(cal.clone()));
        cal.fixtures[0].id ^= 1;
        assert!(decode_calendar(&encode_calendar(&cal)).is_none());
    }
    #[test]
    fn unresolved_past_fixture_cannot_move_the_career_clock_backwards() {
        let world = WorldGenesis::generate(42);
        let mut state = WorldState::new();
        state.world_seed = 42;
        state.dated_calendar = true;
        state.realistic_npc = true;
        state.career_base_year = 2023;
        state.season_number = 1;
        state.pc_epoch_day = 40;
        state.competition_calendar = Some(CompetitionCalendar::default());
        let mut session = SimulationSession::new();
        assert_eq!(
            session.advance_competitions(state, &world, 50).unwrap_err(),
            CalendarError::InvalidClock
        );
    }
    #[test]
    fn qualifiers_do_not_pollute_finals_group_ranking() {
        let mut f = fixture(1, 6, 0, 0, 0, 0, 0, 100, 0, 1, 4);
        let mut cal = CompetitionCalendar::default();
        cal.results.push(FixtureResult {
            fixture: f.clone(),
            goals: [10, 0],
            winner: None,
            detailed: true,
        });
        f.region = 1;
        cal.results.push(FixtureResult {
            fixture: f,
            goals: [0, 1],
            winner: None,
            detailed: true,
        });
        assert_eq!(rank_group(&cal, &[0, 1], 6, 1, 0), vec![0, 1]);
        assert_eq!(rank_group(&cal, &[0, 1], 6, 1, 1), vec![1, 0]);
    }
    #[test]
    fn calendar_rejects_unknown_tournament_members() {
        let f = fixture(1, 2, 0, 1, 0, 0, 1, 100, 0, 1, 3);
        let mut cal = CompetitionCalendar {
            prepared_through: 1,
            fixtures: vec![f.clone()],
            ..Default::default()
        };
        cal.tournaments.push(Tournament {
            season: 1,
            competition: 2,
            region: 0,
            kind: 0,
            phase: 1,
            round: 0,
            participants: vec![0, 1],
            groups: vec![],
            alive: vec![0, 1],
            fixture_ids: vec![f.id],
            champion: None,
        });
        assert!(valid_calendar(&cal));
        cal.tournaments[0].alive.push(u32::MAX);
        assert!(!valid_calendar(&cal));
    }
}
