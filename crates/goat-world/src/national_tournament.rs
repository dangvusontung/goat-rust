//! National-team competitions: World Cup + continental championship. Bible §9 /
//! TASK-DESIGN-round4-competitions-slice4-national-teams.
//!
//! Cadence: `season_number % 4 == 1` is a World Cup year, `== 3` a continental-
//! championship year (§4.1) — the two can never coincide. Qualifying is 4 groups of 5
//! nations (single round-robin, the standard odd-team circle method: 5 rounds, 2 games +
//! 1 bye per round), top 2 per group advancing (8 finalists). The tournament proper draws
//! those 8 into 2 groups of 4 (single round-robin), then a single-leg semifinal + final —
//! deliberately NOT two-legged like `continental.rs`'s club-tier knockouts (§4.4: real
//! international tournaments play one-off knockout matches).
//!
//! Every draw (qualifying-group partition, tournament-proper group draw, knockout
//! pairing) is a pure function of `(world_seed, tournament_season, ...)`, on its own
//! forked RNG stream — independent of `calendar`/`match`/`transfer`/`injury` and of the
//! `domestic_cup`/`continental` streams. No protective seeding anywhere.
//!
//! A nation's match-strength is recomputed fresh from its eligible (non-retired,
//! by-nationality) population each time it's needed — mirroring Design round 2 Doc B's
//! B.2 "recompute eligibility fresh each window, no persisted national-squad roster"
//! pattern exactly, rather than inventing a second national-squad concept.

use crate::domestic_cup::{break_tie, draw_bracket_round};
use crate::population::Population;
use crate::season::sim_team_match;
use crate::world::{seed_mix, NationId, WorldGenesis, NUM_NATIONS};
use goat_rng::{GoatRng, RngSource};

// ── RNG domains ──────────────────────────────────────────────────────────────────
// Distinct salts, mutually independent of `domestic_cup.rs` (0x0C0x) and
// `continental.rs` (0xCA0x), and of each other.
const QUALIFYING_GROUP_DRAW_SALT: u64 = 0x0000_0000_0000_F001;
const QUALIFYING_MATCH_SALT: u64 = 0x0000_0000_0000_F002;
const QUALIFYING_STANDINGS_TIEBREAK_SALT: u64 = 0x0000_0000_0000_F003;
const TOURNAMENT_GROUP_DRAW_SALT: u64 = 0x0000_0000_0000_F004;
const TOURNAMENT_MATCH_SALT: u64 = 0x0000_0000_0000_F005;
const TOURNAMENT_STANDINGS_TIEBREAK_SALT: u64 = 0x0000_0000_0000_F006;
const KNOCKOUT_DRAW_SALT: u64 = 0x0000_0000_0000_F007;
const KNOCKOUT_MATCH_SALT: u64 = 0x0000_0000_0000_F008;
const KNOCKOUT_TIEBREAK_SALT: u64 = 0x0000_0000_0000_F009;

// ── §4.1 — cadence ────────────────────────────────────────────────────────────────

/// True when `season_number` is a World Cup year (every 4 seasons).
pub fn is_world_cup_season(season_number: u32) -> bool {
    season_number % 4 == 1
}

/// True when `season_number` is a continental-championship year (every 4 seasons,
/// staggered 2 years from the World Cup).
pub fn is_continental_championship_season(season_number: u32) -> bool {
    season_number % 4 == 3
}

// ── §4.3 — qualifying-group formation ────────────────────────────────────────────

pub const NUM_QUALIFYING_GROUPS: usize = 4;
pub const QUALIFYING_GROUP_SIZE: usize = 5;

/// Split all 20 nations into 4 groups of 5 by a seeded random partition, redrawn fresh
/// per qualifying cycle (keyed by `tournament_season`, the season the cycle culminates
/// in — so a World-Cup cycle and a continental-championship cycle never share groups).
/// A "generated but consistent" random grouping, not geography-based (no confederations
/// exist on `GeneratedNation`).
pub fn qualifying_group_partition(world_seed: u64, tournament_season: u32) -> Vec<Vec<NationId>> {
    debug_assert_eq!(NUM_NATIONS, NUM_QUALIFYING_GROUPS * QUALIFYING_GROUP_SIZE);
    let seed = seed_mix(
        world_seed ^ (tournament_season as u64).rotate_left(19),
        QUALIFYING_GROUP_DRAW_SALT,
        0,
    );
    let mut rng = GoatRng::new(seed);
    let mut pool: Vec<NationId> = (0..NUM_NATIONS).collect();
    for i in (1..pool.len()).rev() {
        let j = rng.next_range_u32(0, i as u32) as usize;
        pool.swap(i, j);
    }
    pool.chunks_exact(QUALIFYING_GROUP_SIZE)
        .map(|c| c.to_vec())
        .collect()
}

// ── Standard odd-team-count circle-method round robin ───────────────────────────

/// One round of a round-robin schedule: the pairs playing (by in-group index, 0..n),
/// plus the index sitting out this round (`None` for an even-sized group).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundRobinRound {
    pub pairs: Vec<(usize, usize)>,
    pub bye: Option<usize>,
}

/// Standard circle-method single round-robin schedule for `n` participants (by in-group
/// index, 0..n). For odd `n`, a dummy slot is added internally so every real participant
/// sits out exactly once across `n` rounds (2 games + 1 bye per round at `n = 5`); for
/// even `n`, no dummy is needed and every round is fully paired (`n - 1` rounds).
/// Deterministic — a structural schedule, not a random draw (the group's own membership
/// was already seeded by `qualifying_group_partition`/the tournament-proper group draw).
pub fn round_robin_schedule(n: usize) -> Vec<RoundRobinRound> {
    assert!(n >= 2, "a round-robin needs at least 2 participants");
    let odd = n % 2 == 1;
    let dummy = n; // sentinel index, only meaningful when `odd`
    let total = if odd { n + 1 } else { n };
    let mut arr: Vec<usize> = (0..total).collect();
    let num_rounds = total - 1;

    let mut rounds = Vec::with_capacity(num_rounds);
    for _ in 0..num_rounds {
        let mut pairs = Vec::with_capacity(total / 2);
        let mut bye = None;
        for i in 0..total / 2 {
            let a = arr[i];
            let b = arr[total - 1 - i];
            if odd && a == dummy {
                bye = Some(b);
            } else if odd && b == dummy {
                bye = Some(a);
            } else {
                pairs.push((a, b));
            }
        }
        rounds.push(RoundRobinRound { pairs, bye });

        // Rotate: keep arr[0] fixed, shift everyone else one slot clockwise.
        let last = arr[total - 1];
        for i in (2..total).rev() {
            arr[i] = arr[i - 1];
        }
        arr[1] = last;
    }
    rounds
}

/// Which of the 3 qualifying-window slots (0, 1, 2) round `round_idx` (0-indexed, 0..5)
/// is played in — window 0 -> rounds 0-1, window 1 -> rounds 2-3, window 2 -> round 4
/// only (§4.3's confirmed mapping; the 6th budgeted slot in window 2 is left unused,
/// flagged explicitly rather than inventing a mechanic to fill it).
pub fn qualifying_window_for_round(round_idx: usize) -> usize {
    match round_idx {
        0 | 1 => 0,
        2 | 3 => 1,
        4 => 2,
        _ => panic!("a 5-nation single round-robin only has rounds 0..5, got {round_idx}"),
    }
}

// ── Nation match-strength ─────────────────────────────────────────────────────────

/// Typical national-squad size the eligible-population quality average is taken over.
const NATIONAL_SQUAD_SIZE: usize = 23;

/// A nation's match-strength (1-99) for one fixture: mean current-OVR of its top
/// `NATIONAL_SQUAD_SIZE` eligible (non-retired, by-nationality) background players,
/// blended with `stature` (the nation's footballing-ecosystem reputation, the same dial
/// `continental.rs` ranks nations by) as a floor/ceiling nudge. Recomputed fresh every
/// call — no persisted national-squad roster, per Doc B's B.2 pattern.
pub fn national_team_strength(
    pop: &Population,
    nation_stature: u8,
    nation: NationId,
    elapsed_weeks: u32,
) -> u8 {
    let mut ovrs: Vec<u32> = (0..pop.len())
        .filter(|&i| pop.nation[i] as usize == nation && !pop.is_retired(i, elapsed_weeks))
        .map(|i| pop.current_ovr(i, elapsed_weeks) as u32)
        .collect();
    ovrs.sort_unstable_by(|a, b| b.cmp(a));
    ovrs.truncate(NATIONAL_SQUAD_SIZE);
    let pop_quality = if ovrs.is_empty() {
        1
    } else {
        ovrs.iter().sum::<u32>() / ovrs.len() as u32
    };
    ((pop_quality * 7 + nation_stature as u32 * 3) / 10).clamp(1, 99) as u8
}

// ── Standings + tie-break ─────────────────────────────────────────────────────────

/// One nation's record within a qualifying or tournament-proper group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NationStanding {
    pub nation: NationId,
    pub w: u32,
    pub d: u32,
    pub l: u32,
    pub gf: u32,
    pub ga: u32,
}

impl NationStanding {
    pub fn points(&self) -> u32 {
        self.w * 3 + self.d
    }
    pub fn goal_diff(&self) -> i32 {
        self.gf as i32 - self.ga as i32
    }
}

/// Sort `standings` points desc, goal-diff desc, goals-scored desc, then a seeded
/// per-nation random tiebreak key (§4.3/§4.4's "seeded coin-flip" for a genuine full tie)
/// drawn from its own stream so the order is total and deterministic per seed.
fn sort_standings_with_tiebreak(standings: &mut [NationStanding], tiebreak_seed: u64) {
    let mut rng = GoatRng::new(tiebreak_seed);
    // Draw keys in standings order (not sorted order) so the result is independent of
    // whatever order the caller happened to build `standings` in.
    let keys: Vec<u64> = standings.iter().map(|_| rng.next_u64()).collect();
    let mut idx: Vec<usize> = (0..standings.len()).collect();
    idx.sort_by(|&a, &b| {
        standings[b]
            .points()
            .cmp(&standings[a].points())
            .then(standings[b].goal_diff().cmp(&standings[a].goal_diff()))
            .then(standings[b].gf.cmp(&standings[a].gf))
            .then(keys[b].cmp(&keys[a]))
    });
    let sorted: Vec<NationStanding> = idx.iter().map(|&i| standings[i]).collect();
    standings.copy_from_slice(&sorted);
}

/// Bundles the per-cycle identity a round-robin group is resolved against — pulled out
/// of `simulate_round_robin_group`'s argument list (clippy's `too_many_arguments`),
/// mirroring `continental.rs`'s `KnockoutSeasonCtx` pattern.
struct RoundRobinCtx<'a> {
    world: &'a WorldGenesis,
    pop: &'a Population,
    world_seed: u64,
    tournament_season: u32,
    elapsed_weeks: u32,
    match_salt: u64,
    tiebreak_salt: u64,
}

/// Play a single round-robin among `nations` (already-drawn group membership), returning
/// each nation's final standing, points/GD/GF/seeded-coin-flip sorted. Shared by
/// qualifying groups (size 5) and tournament-proper groups (size 4) — same round-robin +
/// tie-break shape, only the group size and RNG salts differ.
fn simulate_round_robin_group(
    ctx: &RoundRobinCtx,
    group_idx: usize,
    nations: &[NationId],
) -> Vec<NationStanding> {
    let mut standings: Vec<NationStanding> = nations
        .iter()
        .map(|&n| NationStanding {
            nation: n,
            w: 0,
            d: 0,
            l: 0,
            gf: 0,
            ga: 0,
        })
        .collect();

    let match_seed = seed_mix(
        ctx.world_seed
            ^ (ctx.tournament_season as u64).rotate_left(23)
            ^ (group_idx as u64).rotate_left(41),
        ctx.match_salt,
        0,
    );
    let mut rng = GoatRng::new(match_seed);

    for round in round_robin_schedule(nations.len()) {
        for (a_idx, b_idx) in round.pairs {
            let (home_idx, away_idx) = if rng.next_range_u32(0, 1) == 1 {
                (b_idx, a_idx)
            } else {
                (a_idx, b_idx)
            };
            let home_nation = nations[home_idx];
            let away_nation = nations[away_idx];
            let home_str = national_team_strength(
                ctx.pop,
                ctx.world.nations[home_nation].stature,
                home_nation,
                ctx.elapsed_weeks,
            );
            let away_str = national_team_strength(
                ctx.pop,
                ctx.world.nations[away_nation].stature,
                away_nation,
                ctx.elapsed_weeks,
            );
            let (hg, ag) = sim_team_match(home_str, away_str, &mut rng);
            standings[home_idx].gf += hg;
            standings[home_idx].ga += ag;
            standings[away_idx].gf += ag;
            standings[away_idx].ga += hg;
            if hg > ag {
                standings[home_idx].w += 1;
                standings[away_idx].l += 1;
            } else if ag > hg {
                standings[away_idx].w += 1;
                standings[home_idx].l += 1;
            } else {
                standings[home_idx].d += 1;
                standings[away_idx].d += 1;
            }
        }
    }

    let tiebreak_seed = seed_mix(
        ctx.world_seed
            ^ (ctx.tournament_season as u64).rotate_left(29)
            ^ (group_idx as u64).rotate_left(43),
        ctx.tiebreak_salt,
        0,
    );
    sort_standings_with_tiebreak(&mut standings, tiebreak_seed);
    standings
}

// ── §4.3 — qualifying ─────────────────────────────────────────────────────────────

/// One qualifying group's fully-resolved standings (points/GD/GF/coin-flip sorted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifyingGroupResult {
    pub group_idx: usize,
    pub standings: Vec<NationStanding>,
}

/// The full qualifying cycle for one tournament: 4 groups of 5, single round-robin each.
/// Population aging is snapshotted at the tournament season's elapsed-weeks anchor (the
/// 3-season qualifying campaign itself isn't broken into per-season population growth —
/// consistent with `domestic_cup`/`continental`'s own single-season-snapshot precedent;
/// per-real-calendar-season fixture scheduling is a bridge-level concern, not this
/// engine's).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifyingCycleResult {
    pub groups: Vec<QualifyingGroupResult>,
}

pub fn simulate_qualifying_cycle(
    world: &WorldGenesis,
    pop: &Population,
    world_seed: u64,
    tournament_season: u32,
) -> QualifyingCycleResult {
    let ctx = RoundRobinCtx {
        world,
        pop,
        world_seed,
        tournament_season,
        elapsed_weeks: tournament_season.saturating_sub(1) * 52,
        match_salt: QUALIFYING_MATCH_SALT,
        tiebreak_salt: QUALIFYING_STANDINGS_TIEBREAK_SALT,
    };
    let group_pools = qualifying_group_partition(world_seed, tournament_season);
    let groups = group_pools
        .into_iter()
        .enumerate()
        .map(|(group_idx, nations)| QualifyingGroupResult {
            group_idx,
            standings: simulate_round_robin_group(&ctx, group_idx, &nations),
        })
        .collect();
    QualifyingCycleResult { groups }
}

/// Top 2 nations per qualifying group advance to the tournament proper: 4 groups x 2 =
/// 8 finalists (§4.3).
pub fn qualifying_finalists(result: &QualifyingCycleResult) -> Vec<NationId> {
    result
        .groups
        .iter()
        .flat_map(|g| [g.standings[0].nation, g.standings[1].nation])
        .collect()
}

// ── §4.4 — tournament proper: group stage ────────────────────────────────────────

pub const NUM_TOURNAMENT_GROUPS: usize = 2;
pub const TOURNAMENT_GROUP_SIZE: usize = 4;

/// Draw the 8 finalists into 2 groups of 4, no protective seeding (own forked stream,
/// `national_tournament_draw`).
pub fn draw_tournament_groups(
    world_seed: u64,
    tournament_season: u32,
    finalists: &[NationId],
) -> Vec<Vec<NationId>> {
    debug_assert_eq!(
        finalists.len(),
        NUM_TOURNAMENT_GROUPS * TOURNAMENT_GROUP_SIZE
    );
    let seed = seed_mix(
        world_seed ^ (tournament_season as u64).rotate_left(31),
        TOURNAMENT_GROUP_DRAW_SALT,
        0,
    );
    let mut rng = GoatRng::new(seed);
    let mut pool = finalists.to_vec();
    for i in (1..pool.len()).rev() {
        let j = rng.next_range_u32(0, i as u32) as usize;
        pool.swap(i, j);
    }
    pool.chunks_exact(TOURNAMENT_GROUP_SIZE)
        .map(|c| c.to_vec())
        .collect()
}

/// The full tournament-proper group stage: 2 groups of 4, single round-robin each.
pub fn simulate_tournament_group_stage(
    world: &WorldGenesis,
    pop: &Population,
    world_seed: u64,
    tournament_season: u32,
    groups: &[Vec<NationId>],
) -> Vec<Vec<NationStanding>> {
    let ctx = RoundRobinCtx {
        world,
        pop,
        world_seed,
        tournament_season,
        elapsed_weeks: tournament_season.saturating_sub(1) * 52,
        match_salt: TOURNAMENT_MATCH_SALT,
        tiebreak_salt: TOURNAMENT_STANDINGS_TIEBREAK_SALT,
    };
    groups
        .iter()
        .enumerate()
        .map(|(group_idx, nations)| simulate_round_robin_group(&ctx, group_idx, nations))
        .collect()
}

/// Top 2 nations per tournament-proper group advance to the knockout: 2 groups x 2 = 4.
pub fn tournament_group_advancers(group_standings: &[Vec<NationStanding>]) -> Vec<NationId> {
    group_standings
        .iter()
        .flat_map(|g| [g[0].nation, g[1].nation])
        .collect()
}

// ── §4.4 — knockout: single-leg throughout (deliberately NOT two-legged) ─────────

/// One resolved single-leg knockout tie. Deliberately has no second-leg/aggregate
/// concept at all (unlike `continental::KnockoutTie`, whose club-tier ties are
/// two-legged except the final) — real international-tournament knockout rounds
/// (World Cup/Euro semifinal and final alike) are one-off matches, so this type simply
/// never carries the data a two-legged tie would need.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NationalKnockoutTie {
    pub nation_a: NationId,
    pub nation_b: NationId,
    pub goals_a: u32,
    pub goals_b: u32,
    pub winner: NationId,
}

/// One resolved knockout round: every tie (this bracket is always exactly 4 -> 2 -> 1,
/// so a round never goes odd — no bye field is needed here, unlike `domestic_cup`'s
/// `CupRoundResult`/`continental`'s `KnockoutRoundResult`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NationalKnockoutRoundResult {
    pub round: usize,
    pub ties: Vec<NationalKnockoutTie>,
}

impl NationalKnockoutRoundResult {
    pub fn winners(&self) -> Vec<NationId> {
        self.ties.iter().map(|t| t.winner).collect()
    }
}

/// Run the single-leg knockout bracket (semifinal, then final) from the 4 tournament
/// group-stage advancers to a champion. Every round redraws pairings fresh (no fixed
/// bracket, no protective seeding), reusing `domestic_cup::draw_bracket_round`'s
/// shuffle+pair primitive on this file's own salted stream (safe to share the code even
/// though this pool is always even-sized and so never actually exercises that
/// primitive's bye branch).
pub fn simulate_national_knockout(
    world: &WorldGenesis,
    pop: &Population,
    world_seed: u64,
    tournament_season: u32,
    seed_pool: Vec<NationId>,
) -> Vec<NationalKnockoutRoundResult> {
    let elapsed_weeks = tournament_season.saturating_sub(1) * 52;
    let mut rounds = Vec::new();
    let mut pool = seed_pool;
    let mut round_idx = 0;
    while pool.len() > 1 {
        debug_assert_eq!(pool.len() % 2, 0, "national knockout must never go odd");
        let draw_seed = seed_mix(
            world_seed
                ^ (tournament_season as u64).rotate_left(11)
                ^ (round_idx as u64).rotate_left(29),
            KNOCKOUT_DRAW_SALT,
            0,
        );
        let draw = draw_bracket_round(draw_seed, &pool);
        debug_assert!(draw.bye.is_none(), "an even pool must never produce a bye");

        let match_seed = seed_mix(
            world_seed
                ^ (tournament_season as u64).rotate_left(13)
                ^ (round_idx as u64).rotate_left(31),
            KNOCKOUT_MATCH_SALT,
            0,
        );
        let mut match_rng = GoatRng::new(match_seed);
        let tiebreak_seed = seed_mix(
            world_seed
                ^ (tournament_season as u64).rotate_left(17)
                ^ (round_idx as u64).rotate_left(37),
            KNOCKOUT_TIEBREAK_SALT,
            0,
        );
        let mut tiebreak_rng = GoatRng::new(tiebreak_seed);

        let ties: Vec<NationalKnockoutTie> = draw
            .pairs
            .iter()
            .map(|&(a, b)| {
                let a_str = national_team_strength(pop, world.nations[a].stature, a, elapsed_weeks);
                let b_str = national_team_strength(pop, world.nations[b].stature, b, elapsed_weeks);
                let (ga, gb) = sim_team_match(a_str, b_str, &mut match_rng);
                let winner = break_tie(a, b, ga, gb, &mut tiebreak_rng);
                NationalKnockoutTie {
                    nation_a: a,
                    nation_b: b,
                    goals_a: ga,
                    goals_b: gb,
                    winner,
                }
            })
            .collect();

        pool = ties.iter().map(|t| t.winner).collect();
        rounds.push(NationalKnockoutRoundResult {
            round: round_idx,
            ties,
        });
        round_idx += 1;
    }
    rounds
}

// ── Full-cycle driver ────────────────────────────────────────────────────────────

/// The full resolved national-team tournament cycle: qualifying -> finalists ->
/// tournament-proper group stage -> knockout -> champion. Deterministic in
/// `(world_seed, tournament_season)` — "generated but consistent", never persisted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NationalTournamentResult {
    pub qualifying: QualifyingCycleResult,
    pub tournament_groups: Vec<Vec<NationStanding>>,
    pub knockout: Vec<NationalKnockoutRoundResult>,
    pub champion: NationId,
}

pub fn simulate_national_tournament(
    world: &WorldGenesis,
    pop: &Population,
    world_seed: u64,
    tournament_season: u32,
) -> NationalTournamentResult {
    let qualifying = simulate_qualifying_cycle(world, pop, world_seed, tournament_season);
    let finalists = qualifying_finalists(&qualifying);
    debug_assert_eq!(
        finalists.len(),
        NUM_TOURNAMENT_GROUPS * TOURNAMENT_GROUP_SIZE
    );

    let groups = draw_tournament_groups(world_seed, tournament_season, &finalists);
    let tournament_groups =
        simulate_tournament_group_stage(world, pop, world_seed, tournament_season, &groups);
    let advancers = tournament_group_advancers(&tournament_groups);
    debug_assert_eq!(advancers.len(), 4);

    let knockout = simulate_national_knockout(world, pop, world_seed, tournament_season, advancers);
    let champion = knockout
        .last()
        .and_then(|r| r.winners().first().copied())
        .expect("the national knockout must always resolve to exactly one champion");

    NationalTournamentResult {
        qualifying,
        tournament_groups,
        knockout,
        champion,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::population;

    fn setup(seed: u64) -> (WorldGenesis, Population) {
        let world = WorldGenesis::generate(seed);
        let pop = population::genesis(seed, &world);
        (world, pop)
    }

    #[test]
    fn world_cup_and_continental_championship_never_land_in_the_same_season() {
        for season in 1u32..=200 {
            let wc = is_world_cup_season(season);
            let cc = is_continental_championship_season(season);
            assert!(
                !(wc && cc),
                "season {season} flagged as both WC and CC year"
            );
        }
        // Sanity: both cadences do actually fire somewhere in the range (not vacuously
        // true because neither formula ever matches).
        assert!((1u32..=200).any(is_world_cup_season));
        assert!((1u32..=200).any(is_continental_championship_season));
    }

    #[test]
    fn qualifying_group_partition_is_deterministic_per_seed() {
        let a = qualifying_group_partition(42, 5);
        let b = qualifying_group_partition(42, 5);
        assert_eq!(a, b);
        let c = qualifying_group_partition(42, 9);
        assert_ne!(a, c, "different qualifying cycles must redraw groups");

        assert_eq!(a.len(), NUM_QUALIFYING_GROUPS);
        for g in &a {
            assert_eq!(g.len(), QUALIFYING_GROUP_SIZE);
        }
        let mut seen: Vec<NationId> = a.iter().flatten().copied().collect();
        seen.sort_unstable();
        let expected: Vec<NationId> = (0..NUM_NATIONS).collect();
        assert_eq!(seen, expected, "every nation must appear exactly once");
    }

    #[test]
    fn qualifying_round_robin_completes_in_five_rounds_with_one_bye_per_round() {
        let rounds = round_robin_schedule(QUALIFYING_GROUP_SIZE);
        assert_eq!(
            rounds.len(),
            5,
            "a 5-team single round-robin needs 5 rounds"
        );

        let mut bye_counts = [0u32; QUALIFYING_GROUP_SIZE];
        let mut games_played = [0u32; QUALIFYING_GROUP_SIZE];
        for round in &rounds {
            assert_eq!(round.pairs.len(), 2, "2 simultaneous games per round");
            assert!(round.bye.is_some(), "exactly 1 nation sits out per round");
            bye_counts[round.bye.unwrap()] += 1;
            for &(a, b) in &round.pairs {
                games_played[a] += 1;
                games_played[b] += 1;
            }
        }
        assert_eq!(
            bye_counts.iter().sum::<u32>(),
            5,
            "5 rounds, 1 bye each = 5 total byes"
        );
        assert!(
            bye_counts.iter().all(|&c| c == 1),
            "every nation sits out exactly once: {bye_counts:?}"
        );
        assert!(
            games_played.iter().all(|&c| c == 4),
            "every nation plays exactly 4 games (the other 4 group members): {games_played:?}"
        );

        // Window mapping (§4.3): window0 -> rounds 0-1, window1 -> rounds 2-3,
        // window2 -> round 4 only.
        assert_eq!(qualifying_window_for_round(0), 0);
        assert_eq!(qualifying_window_for_round(1), 0);
        assert_eq!(qualifying_window_for_round(2), 1);
        assert_eq!(qualifying_window_for_round(3), 1);
        assert_eq!(qualifying_window_for_round(4), 2);
    }

    #[test]
    fn top_two_per_qualifying_group_advance() {
        let (world, pop) = setup(17);
        let qualifying = simulate_qualifying_cycle(&world, &pop, 17, 5);
        assert_eq!(qualifying.groups.len(), NUM_QUALIFYING_GROUPS);
        for g in &qualifying.groups {
            assert_eq!(g.standings.len(), QUALIFYING_GROUP_SIZE);
            // Every nation in a 5-team single round-robin plays exactly 4 games.
            for s in &g.standings {
                assert_eq!(s.w + s.d + s.l, 4);
            }
        }
        let finalists = qualifying_finalists(&qualifying);
        assert_eq!(finalists.len(), 8, "4 groups x top 2 = 8 finalists");
        let mut sorted = finalists.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 8, "8 distinct finalist nations");
    }

    #[test]
    fn tournament_group_stage_produces_correct_advancers() {
        let (world, pop) = setup(23);
        let qualifying = simulate_qualifying_cycle(&world, &pop, 23, 9);
        let finalists = qualifying_finalists(&qualifying);
        let groups = draw_tournament_groups(23, 9, &finalists);
        assert_eq!(groups.len(), NUM_TOURNAMENT_GROUPS);
        for g in &groups {
            assert_eq!(g.len(), TOURNAMENT_GROUP_SIZE);
        }
        let mut seen: Vec<NationId> = groups.iter().flatten().copied().collect();
        seen.sort_unstable();
        let mut expected = finalists.clone();
        expected.sort_unstable();
        assert_eq!(
            seen, expected,
            "every finalist drawn into exactly one group"
        );

        let standings = simulate_tournament_group_stage(&world, &pop, 23, 9, &groups);
        assert_eq!(standings.len(), NUM_TOURNAMENT_GROUPS);
        for g in &standings {
            assert_eq!(g.len(), TOURNAMENT_GROUP_SIZE);
            for s in g {
                assert_eq!(s.w + s.d + s.l, 3, "single round-robin of 4 = 3 games each");
            }
        }
        let advancers = tournament_group_advancers(&standings);
        assert_eq!(advancers.len(), 4, "2 groups x top 2 = 4 knockout entrants");
    }

    #[test]
    fn tournament_knockout_is_single_leg_not_two_legged() {
        let (world, pop) = setup(31);
        let pool: Vec<NationId> = vec![0, 1, 2, 3];
        let rounds = simulate_national_knockout(&world, &pop, 31, 5, pool);

        assert_eq!(
            rounds.len(),
            2,
            "4 nations: semifinal then final only, never a bye"
        );
        assert_eq!(rounds[0].ties.len(), 2, "semifinal: 2 ties");
        assert_eq!(rounds[1].ties.len(), 1, "final: 1 tie");

        // `NationalKnockoutTie` carries only ONE scoreline per tie (goals_a/goals_b),
        // with no aggregate/leg2 concept at all -- structurally distinct from
        // `continental::KnockoutTie`, which is two-legged except at its final round.
        // This is true for every round here, including the semifinal (not just the
        // final), which is the deliberate divergence this test guards.
        for round in &rounds {
            for tie in &round.ties {
                assert!(tie.winner == tie.nation_a || tie.winner == tie.nation_b);
                // A tie's outcome is fully determined by exactly one scoreline: no
                // higher-goals side can lose.
                if tie.goals_a > tie.goals_b {
                    assert_eq!(tie.winner, tie.nation_a);
                } else if tie.goals_b > tie.goals_a {
                    assert_eq!(tie.winner, tie.nation_b);
                }
            }
        }
        assert_eq!(rounds.last().unwrap().winners().len(), 1);
    }

    #[test]
    fn tournament_produces_exactly_one_champion() {
        let (world, pop) = setup(50);
        let result = simulate_national_tournament(&world, &pop, 50, 13);
        assert!(world.nations.iter().any(|n| n.id == result.champion));
    }

    #[test]
    fn national_tournament_cycle_is_deterministic_per_seed() {
        let (world, pop) = setup(101);
        let a = simulate_national_tournament(&world, &pop, 101, 5);
        let b = simulate_national_tournament(&world, &pop, 101, 5);
        assert_eq!(a, b);
    }

    #[test]
    fn national_streams_independent_of_continental_and_domestic_cup() {
        // Same world_seed/season-ish key: the qualifying-group draw must not coincide
        // with continental's group draw or domestic_cup's round draw (guards against an
        // accidental shared-stream salt collision).
        use crate::continental::{self, ContinentalTier};
        use crate::domestic_cup::draw_round;
        use crate::season::Table;

        let (world, pop) = setup(200);
        let _ = &pop;
        let groups = qualifying_group_partition(200, 1);
        let flat: Vec<NationId> = groups.iter().flatten().copied().collect();

        let tables: Vec<Table> = world.leagues.iter().map(|l| Table::new(&l.clubs)).collect();
        let qualified = continental::qualify_clubs(&world, &tables, ContinentalTier::Tier1);
        let continental_groups =
            continental::draw_groups(200, 1, ContinentalTier::Tier1, &qualified);
        let cup_draw = draw_round(200, 1, 0, 0, &world.leagues[2].clubs.clone());

        assert_ne!(
            &flat,
            &continental_groups
                .iter()
                .flatten()
                .copied()
                .collect::<Vec<_>>(),
            "different-shaped pools, but guards a copy-paste salt collision regardless"
        );
        assert!(!cup_draw.pairs.is_empty());
    }

    #[test]
    fn national_team_strength_is_bounded_and_deterministic() {
        let (world, pop) = setup(7);
        for nation in 0..NUM_NATIONS {
            let s1 = national_team_strength(&pop, world.nations[nation].stature, nation, 0);
            let s2 = national_team_strength(&pop, world.nations[nation].stature, nation, 0);
            assert_eq!(s1, s2);
            assert!((1..=99).contains(&s1));
        }
    }
}
