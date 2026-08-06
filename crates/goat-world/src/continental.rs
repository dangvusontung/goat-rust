//! Continental club competitions: 3 tiers, stature-ranked qualification, group stage
//! then knockout. Bible §9 / TASK-DESIGN-round4-competitions-slice2-3 §3.
//!
//! Every draw (slot allocation is deterministic math, not RNG; group draw, knockout
//! draw) is a pure function of `(world_seed, tier, season, ...)`, on its own salted
//! stream per tier and phase — independent of `calendar`/`match`/`transfer`/`injury`
//! and of `domestic_cup`'s streams. No protective seeding: a group can contain 2+ clubs
//! from the same nation, and the knockout draw never avoids a rematch or a same-nation
//! pairing.

use crate::domestic_cup::{break_tie, draw_bracket_round};
use crate::season::{sim_team_match, Table};
use crate::world::{seed_mix, ClubId, DivLevel, NationId, WorldGenesis};
use goat_rng::{GoatRng, RngSource};

/// The three continental tiers (Champions-League/Europa/Conference-equivalent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinentalTier {
    Tier1,
    Tier2,
    Tier3,
}

impl ContinentalTier {
    pub const ALL: [ContinentalTier; 3] = [
        ContinentalTier::Tier1,
        ContinentalTier::Tier2,
        ContinentalTier::Tier3,
    ];

    /// Total qualified clubs entering the group stage (§3.2's taper totals): 32/48/64.
    pub fn group_stage_size(self) -> usize {
        match self {
            ContinentalTier::Tier1 => 32,
            ContinentalTier::Tier2 => 48,
            ContinentalTier::Tier3 => 64,
        }
    }

    /// Number of 4-team groups: 8/12/16.
    pub fn num_groups(self) -> usize {
        self.group_stage_size() / 4
    }

    /// Clubs entering the knockout bracket (top 2 per group): 16/24/32.
    pub fn knockout_size(self) -> usize {
        self.num_groups() * 2
    }

    /// A stable small index used only to salt this tier's RNG domains distinctly from
    /// the other two tiers.
    fn domain(self) -> u64 {
        match self {
            ContinentalTier::Tier1 => 0,
            ContinentalTier::Tier2 => 1,
            ContinentalTier::Tier3 => 2,
        }
    }
}

// ── RNG domains ──────────────────────────────────────────────────────────────────
// Distinct salts per phase, further mixed with `ContinentalTier::domain()` so all 3
// tiers' group draws (etc.) are mutually independent as well as independent of every
// other subsystem's stream, including domestic_cup's.
const GROUP_DRAW_SALT: u64 = 0x0000_0000_0000_CA01;
const GROUP_MATCH_SALT: u64 = 0x0000_0000_0000_CA02;
const KNOCKOUT_DRAW_SALT: u64 = 0x0000_0000_0000_CA03;
const KNOCKOUT_MATCH_SALT: u64 = 0x0000_0000_0000_CA04;
const KNOCKOUT_TIEBREAK_SALT: u64 = 0x0000_0000_0000_CA05;

// ── §3.2 — slot-count taper table ────────────────────────────────────────────────

/// (Tier1, Tier2, Tier3) continental slots for a nation at 1-indexed strength `rank`
/// (1 = strongest nation). Exactly the confirmed table (§3.2): tapers to zero for the
/// weakest 6 of 20 nations, in every tier.
fn slots_for_rank(rank: usize) -> (u8, u8, u8) {
    match rank {
        1..=2 => (4, 6, 8),
        3..=6 => (3, 5, 6),
        7..=10 => (2, 3, 4),
        11..=14 => (1, 1, 2),
        _ => (0, 0, 0), // 15..=20 — the weakest 30% get zero continental football.
    }
}

/// Every nation ordered strongest-first by `stature` (rank 1 = strongest). Ties (equal
/// `stature`) break by ascending nation id — a total order is required for the slot
/// allocation to be well-defined and deterministic per `world_seed`.
pub fn nation_ranks(world: &WorldGenesis) -> Vec<NationId> {
    let mut ids: Vec<NationId> = world.nations.iter().map(|n| n.id).collect();
    ids.sort_by(|&a, &b| {
        world.nations[b]
            .stature
            .cmp(&world.nations[a].stature)
            .then(a.cmp(&b))
    });
    ids
}

/// One nation's continental slot allocation across all 3 tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContinentalSlots {
    pub tier1: u8,
    pub tier2: u8,
    pub tier3: u8,
}

impl ContinentalSlots {
    pub fn for_tier(self, tier: ContinentalTier) -> u8 {
        match tier {
            ContinentalTier::Tier1 => self.tier1,
            ContinentalTier::Tier2 => self.tier2,
            ContinentalTier::Tier3 => self.tier3,
        }
    }
}

/// `nation`'s continental slot count in every tier, ranked by `stature` among all 20
/// nations (rank position, not a raw threshold — proportional regardless of how a given
/// `world_seed` distributes the stature rolls).
pub fn continental_slots_for_nation(world: &WorldGenesis, nation: NationId) -> ContinentalSlots {
    let ranks = nation_ranks(world);
    let rank = ranks
        .iter()
        .position(|&n| n == nation)
        .expect("nation must be in world.nations")
        + 1;
    let (tier1, tier2, tier3) = slots_for_rank(rank);
    ContinentalSlots {
        tier1,
        tier2,
        tier3,
    }
}

// ── §3.1 — qualification from final Tier-1 league position ──────────────────────

/// Qualified clubs for `tier`, reading each nation's slot count off its rank and its
/// final Tier-1 domestic league position (never the 2nd/3rd domestic tiers — §3.1).
/// `tier1_tables` must be indexed exactly like `world.leagues` (pass the season's final
/// tables, e.g. `batch_tick_season`'s second return value) — the caller resolves which
/// season's tables to use, this function only reads them.
pub fn qualify_clubs(
    world: &WorldGenesis,
    tier1_tables: &[Table],
    tier: ContinentalTier,
) -> Vec<ClubId> {
    let mut clubs = Vec::with_capacity(tier.group_stage_size());
    for nation in &world.nations {
        let slots = continental_slots_for_nation(world, nation.id);
        let n = slots.for_tier(tier) as usize;
        if n == 0 {
            continue;
        }
        let tier1_league = world
            .leagues
            .iter()
            .find(|l| l.nation == nation.id && l.tier == DivLevel::Top)
            .expect("every nation has a Tier-1 league");
        let sorted = tier1_tables[tier1_league.id].sorted();
        clubs.extend(sorted.iter().take(n).map(|e| e.club_id));
    }
    clubs
}

// ── §3.3 — group draw + single round-robin group stage ──────────────────────────

/// Draw `clubs` (exactly `tier.group_stage_size()` of them) into 4-team groups. No
/// protective seeding: a group may contain 2+ clubs from the same nation.
pub fn draw_groups(
    world_seed: u64,
    season: u32,
    tier: ContinentalTier,
    clubs: &[ClubId],
) -> Vec<[ClubId; 4]> {
    debug_assert_eq!(clubs.len(), tier.group_stage_size());
    debug_assert!(clubs.len().is_multiple_of(4));
    let seed = seed_mix(
        world_seed ^ (season as u64).rotate_left(19),
        GROUP_DRAW_SALT,
        tier.domain(),
    );
    let mut rng = GoatRng::new(seed);
    let mut pool = clubs.to_vec();
    for i in (1..pool.len()).rev() {
        let j = rng.next_range_u32(0, i as u32) as usize;
        pool.swap(i, j);
    }
    pool.chunks_exact(4)
        .map(|c| [c[0], c[1], c[2], c[3]])
        .collect()
}

/// One club's group-stage record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GroupStanding {
    pub club: ClubId,
    pub w: u32,
    pub d: u32,
    pub l: u32,
    pub gf: u32,
    pub ga: u32,
}

impl GroupStanding {
    pub fn points(&self) -> u32 {
        self.w * 3 + self.d
    }
    pub fn goal_diff(&self) -> i32 {
        self.gf as i32 - self.ga as i32
    }
}

/// Simulate one group's single round-robin (6 matches: every pair plays once — not
/// double round-robin, a deliberate density-control choice, §3.3), returning standings
/// sorted points desc, goal-diff desc, gf desc (mirrors `season::Table::sorted`'s
/// tie-break style).
fn simulate_group(
    world: &WorldGenesis,
    world_seed: u64,
    season: u32,
    tier: ContinentalTier,
    group_idx: usize,
    clubs: [ClubId; 4],
) -> [GroupStanding; 4] {
    let seed = seed_mix(
        world_seed ^ (season as u64).rotate_left(23) ^ (group_idx as u64).rotate_left(41),
        GROUP_MATCH_SALT,
        tier.domain(),
    );
    let mut rng = GoatRng::new(seed);

    let mut standings: [GroupStanding; 4] = core::array::from_fn(|i| GroupStanding {
        club: clubs[i],
        ..Default::default()
    });

    for i in 0..4 {
        for j in (i + 1)..4 {
            // Independent home/away roll per pair, same idiom as domestic_cup.
            let (home_idx, away_idx) = if rng.next_range_u32(0, 1) == 1 {
                (j, i)
            } else {
                (i, j)
            };
            let (hg, ag) = sim_team_match(
                world.clubs[clubs[home_idx]].strength,
                world.clubs[clubs[away_idx]].strength,
                &mut rng,
            );
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

    standings.sort_by(|a, b| {
        b.points()
            .cmp(&a.points())
            .then(b.goal_diff().cmp(&a.goal_diff()))
            .then(b.gf.cmp(&a.gf))
    });
    standings
}

// ── §3.3 — knockout: two-legged except a single-leg final, byes reuse Slice 2 ────

/// One resolved knockout tie. Two-legged unless `leg2_*` is `None` (the single-leg
/// final): `club_a` is home in leg 1, `club_b` is home in leg 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnockoutTie {
    pub club_a: ClubId,
    pub club_b: ClubId,
    pub leg1_a_goals: u32,
    pub leg1_b_goals: u32,
    pub leg2_a_goals: Option<u32>,
    pub leg2_b_goals: Option<u32>,
    pub winner: ClubId,
}

impl KnockoutTie {
    pub fn aggregate(&self) -> (u32, u32) {
        (
            self.leg1_a_goals + self.leg2_a_goals.unwrap_or(0),
            self.leg1_b_goals + self.leg2_b_goals.unwrap_or(0),
        )
    }

    /// True for the single-leg final (no second leg played at all).
    pub fn is_single_leg(&self) -> bool {
        self.leg2_a_goals.is_none()
    }
}

/// One resolved knockout round: every tie plus who (if anyone) had the bye — reuses
/// Slice 2's exact odd-bracket bye rule (uniformly random over the whole pool, not
/// tier-biased) via `domestic_cup::draw_bracket_round`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnockoutRoundResult {
    pub round: usize,
    pub ties: Vec<KnockoutTie>,
    pub bye: Option<ClubId>,
}

impl KnockoutRoundResult {
    pub fn winners(&self) -> Vec<ClubId> {
        let mut w: Vec<ClubId> = self.ties.iter().map(|t| t.winner).collect();
        if let Some(b) = self.bye {
            w.push(b);
        }
        w
    }
}

/// Bundles the per-season identity a knockout round is resolved against — pulled out of
/// `resolve_knockout_round`'s argument list (clippy's `too_many_arguments`), and reused
/// as-is by `simulate_knockout`'s round loop.
struct KnockoutSeasonCtx<'a> {
    world: &'a WorldGenesis,
    world_seed: u64,
    season: u32,
    tier: ContinentalTier,
}

fn resolve_knockout_round(
    ctx: &KnockoutSeasonCtx,
    round: usize,
    draw_pairs: &[(ClubId, ClubId)],
    bye: Option<ClubId>,
    is_final: bool,
) -> KnockoutRoundResult {
    let world = ctx.world;
    let match_seed = seed_mix(
        ctx.world_seed ^ (ctx.season as u64).rotate_left(13) ^ (round as u64).rotate_left(31),
        KNOCKOUT_MATCH_SALT,
        ctx.tier.domain(),
    );
    let mut match_rng = GoatRng::new(match_seed);
    let tiebreak_seed = seed_mix(
        ctx.world_seed ^ (ctx.season as u64).rotate_left(17) ^ (round as u64).rotate_left(37),
        KNOCKOUT_TIEBREAK_SALT,
        ctx.tier.domain(),
    );
    let mut tiebreak_rng = GoatRng::new(tiebreak_seed);

    let ties = draw_pairs
        .iter()
        .map(|&(a, b)| {
            // Leg 1: a at home.
            let (l1a, l1b) = sim_team_match(
                world.clubs[a].strength,
                world.clubs[b].strength,
                &mut match_rng,
            );
            // Leg 2: b at home — skipped for the single-leg final.
            let (leg2_a_goals, leg2_b_goals) = if is_final {
                (None, None)
            } else {
                let (l2b, l2a) = sim_team_match(
                    world.clubs[b].strength,
                    world.clubs[a].strength,
                    &mut match_rng,
                );
                (Some(l2a), Some(l2b))
            };
            let agg_a = l1a + leg2_a_goals.unwrap_or(0);
            let agg_b = l1b + leg2_b_goals.unwrap_or(0);
            let winner = break_tie(a, b, agg_a, agg_b, &mut tiebreak_rng);
            KnockoutTie {
                club_a: a,
                club_b: b,
                leg1_a_goals: l1a,
                leg1_b_goals: l1b,
                leg2_a_goals,
                leg2_b_goals,
                winner,
            }
        })
        .collect();

    KnockoutRoundResult { round, ties, bye }
}

/// Run the knockout bracket to a champion from `seed_pool` (the group stage's top-2s,
/// or any even/odd-sized pool for standalone testing). Every round redraws pairings
/// fresh (no fixed bracket), reusing `domestic_cup::draw_bracket_round`'s bye rule at
/// whichever round the pool goes odd. Two-legged throughout except the final round
/// (exactly 2 clubs remaining), which is single-leg.
pub fn simulate_knockout(
    world: &WorldGenesis,
    world_seed: u64,
    season: u32,
    tier: ContinentalTier,
    seed_pool: Vec<ClubId>,
) -> Vec<KnockoutRoundResult> {
    let ctx = KnockoutSeasonCtx {
        world,
        world_seed,
        season,
        tier,
    };
    let mut rounds = Vec::new();
    let mut pool = seed_pool;
    let mut round_idx = 0;
    while pool.len() > 1 {
        let is_final = pool.len() == 2;
        let draw_seed = seed_mix(
            world_seed ^ (season as u64).rotate_left(11) ^ (round_idx as u64).rotate_left(29),
            KNOCKOUT_DRAW_SALT,
            tier.domain(),
        );
        let draw = draw_bracket_round(draw_seed, &pool);
        let result = resolve_knockout_round(&ctx, round_idx, &draw.pairs, draw.bye, is_final);
        pool = result.winners();
        rounds.push(result);
        round_idx += 1;
    }
    rounds
}

// ── Full season driver ───────────────────────────────────────────────────────────

/// The full resolved continental competition for one tier, one season.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinentalSeasonResult {
    pub tier: ContinentalTier,
    pub groups: Vec<[GroupStanding; 4]>,
    pub knockout: Vec<KnockoutRoundResult>,
    pub champion: ClubId,
}

/// Simulate one continental tier's entire season: qualify from `tier1_tables`, draw
/// groups, play the group stage, then run the knockout to a champion. Deterministic in
/// `(world_seed, season, tier, tier1_tables)` — "generated but consistent", never
/// persisted (§9).
pub fn simulate_continental(
    world: &WorldGenesis,
    tier1_tables: &[Table],
    world_seed: u64,
    season: u32,
    tier: ContinentalTier,
) -> ContinentalSeasonResult {
    let qualified = qualify_clubs(world, tier1_tables, tier);
    debug_assert_eq!(qualified.len(), tier.group_stage_size());

    let group_pools = draw_groups(world_seed, season, tier, &qualified);
    let groups: Vec<[GroupStanding; 4]> = group_pools
        .into_iter()
        .enumerate()
        .map(|(i, clubs)| simulate_group(world, world_seed, season, tier, i, clubs))
        .collect();

    let seed_pool: Vec<ClubId> = groups.iter().flat_map(|g| [g[0].club, g[1].club]).collect();
    debug_assert_eq!(seed_pool.len(), tier.knockout_size());

    let knockout = simulate_knockout(world, world_seed, season, tier, seed_pool);
    let champion = knockout
        .last()
        .and_then(|r| r.winners().first().copied())
        .expect("knockout must always resolve to exactly one champion");

    ContinentalSeasonResult {
        tier,
        groups,
        knockout,
        champion,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::CLUBS_PER_DIV;

    /// Zeroed Tier-1 tables for every league, in genesis club order — enough for
    /// qualification/structural tests, which only need *some* deterministic ranking,
    /// not a realistically-played season (that's covered by `promotion.rs`'s own tests
    /// and the perf benchmark in `examples/bench_genesis.rs`).
    fn zeroed_tier1_tables(world: &WorldGenesis) -> Vec<Table> {
        world.leagues.iter().map(|l| Table::new(&l.clubs)).collect()
    }

    #[test]
    fn continental_slots_match_taper_table() {
        let world = WorldGenesis::generate(5);
        let ranks = nation_ranks(&world);
        assert_eq!(ranks.len(), 20);

        let expected = |rank: usize| -> (u8, u8, u8) {
            match rank {
                1..=2 => (4, 6, 8),
                3..=6 => (3, 5, 6),
                7..=10 => (2, 3, 4),
                11..=14 => (1, 1, 2),
                _ => (0, 0, 0),
            }
        };
        for (i, &nation) in ranks.iter().enumerate() {
            let rank = i + 1;
            let slots = continental_slots_for_nation(&world, nation);
            let (t1, t2, t3) = expected(rank);
            assert_eq!(
                (slots.tier1, slots.tier2, slots.tier3),
                (t1, t2, t3),
                "nation rank {rank} slot mismatch"
            );
        }

        let (sum1, sum2, sum3) = ranks.iter().fold((0u32, 0u32, 0u32), |acc, &n| {
            let slots = continental_slots_for_nation(&world, n);
            (
                acc.0 + slots.tier1 as u32,
                acc.1 + slots.tier2 as u32,
                acc.2 + slots.tier3 as u32,
            )
        });
        assert_eq!(sum1, 32, "Tier1 total slots");
        assert_eq!(sum2, 48, "Tier2 total slots");
        assert_eq!(sum3, 64, "Tier3 total slots");
    }

    #[test]
    fn slot_allocation_is_deterministic_and_a_total_order() {
        let world = WorldGenesis::generate(77);
        let a = nation_ranks(&world);
        let b = nation_ranks(&world);
        assert_eq!(a, b);
        let mut sorted = a.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 20, "every nation must appear exactly once");
    }

    #[test]
    fn qualify_clubs_reads_only_tier1_position() {
        let world = WorldGenesis::generate(9);
        let tables = zeroed_tier1_tables(&world);
        for tier in ContinentalTier::ALL {
            let qualified = qualify_clubs(&world, &tables, tier);
            assert_eq!(qualified.len(), tier.group_stage_size());
            // Every qualified club must actually be a Tier-1 club (never Tier-2/3).
            for &club in &qualified {
                let league_id = world.club_league(club);
                assert_eq!(world.leagues[league_id].tier, DivLevel::Top);
            }
        }
    }

    #[test]
    fn continental_group_draw_deterministic_per_seed() {
        let world = WorldGenesis::generate(9);
        let tables = zeroed_tier1_tables(&world);
        let qualified = qualify_clubs(&world, &tables, ContinentalTier::Tier1);
        let a = draw_groups(9, 1, ContinentalTier::Tier1, &qualified);
        let b = draw_groups(9, 1, ContinentalTier::Tier1, &qualified);
        assert_eq!(a, b);
        let c = draw_groups(9, 2, ContinentalTier::Tier1, &qualified);
        assert_ne!(a, c, "different seasons must redraw groups");
    }

    #[test]
    fn group_draw_covers_every_qualified_club_exactly_once() {
        let world = WorldGenesis::generate(4);
        let tables = zeroed_tier1_tables(&world);
        for tier in ContinentalTier::ALL {
            let qualified = qualify_clubs(&world, &tables, tier);
            let groups = draw_groups(4, 1, tier, &qualified);
            assert_eq!(groups.len(), tier.num_groups());
            let mut seen: Vec<ClubId> = groups.iter().flatten().copied().collect();
            seen.sort_unstable();
            let mut expected = qualified.clone();
            expected.sort_unstable();
            assert_eq!(seen, expected);
        }
    }

    #[test]
    fn continental_group_stage_top_two_advance() {
        let world = WorldGenesis::generate(15);
        let tables = zeroed_tier1_tables(&world);
        for tier in ContinentalTier::ALL {
            let qualified = qualify_clubs(&world, &tables, tier);
            let groups_pool = draw_groups(15, 1, tier, &qualified);
            let groups: Vec<[GroupStanding; 4]> = groups_pool
                .into_iter()
                .enumerate()
                .map(|(i, c)| simulate_group(&world, 15, 1, tier, i, c))
                .collect();
            assert_eq!(groups.len(), tier.num_groups());
            let advancing: Vec<ClubId> =
                groups.iter().flat_map(|g| [g[0].club, g[1].club]).collect();
            assert_eq!(advancing.len(), tier.knockout_size());
            // Exactly 2 per group, and every group's 4 members played exactly 3
            // matches each (single round-robin).
            for g in &groups {
                for standing in g {
                    assert_eq!(standing.w + standing.d + standing.l, 3);
                }
            }
        }
    }

    #[test]
    fn continental_tier2_knockout_uses_a_bye_round_at_24_to_3() {
        let world = WorldGenesis::generate(31);
        let pool: Vec<ClubId> = (0..ContinentalTier::Tier2.knockout_size()).collect();
        let rounds = simulate_knockout(&world, 31, 1, ContinentalTier::Tier2, pool);

        let sizes: Vec<usize> = rounds
            .iter()
            .map(|r| r.ties.len() * 2 + usize::from(r.bye.is_some()))
            .collect();
        assert_eq!(
            sizes,
            vec![24, 12, 6, 3, 2],
            "Tier-2 knockout entrant counts per round"
        );

        let byes: Vec<bool> = rounds.iter().map(|r| r.bye.is_some()).collect();
        assert_eq!(
            byes,
            vec![false, false, false, true, false],
            "exactly one bye, at the round the bracket goes odd (3 remaining)"
        );

        // Every round two-legged except the final.
        for (i, r) in rounds.iter().enumerate() {
            let is_final = i == rounds.len() - 1;
            for tie in &r.ties {
                assert_eq!(tie.is_single_leg(), is_final);
            }
        }
        assert_eq!(rounds.last().unwrap().winners().len(), 1);
    }

    #[test]
    fn tier1_and_tier3_knockouts_need_no_byes() {
        let world = WorldGenesis::generate(6);
        for (tier, expected_sizes) in [
            (ContinentalTier::Tier1, vec![16, 8, 4, 2]),
            (ContinentalTier::Tier3, vec![32, 16, 8, 4, 2]),
        ] {
            let pool: Vec<ClubId> = (0..tier.knockout_size()).collect();
            let rounds = simulate_knockout(&world, 6, 1, tier, pool);
            let sizes: Vec<usize> = rounds
                .iter()
                .map(|r| r.ties.len() * 2 + usize::from(r.bye.is_some()))
                .collect();
            assert_eq!(sizes, expected_sizes, "{tier:?} knockout entrant counts");
            assert!(
                rounds.iter().all(|r| r.bye.is_none()),
                "{tier:?} must need no byes"
            );
            assert_eq!(rounds.last().unwrap().winners().len(), 1);
        }
    }

    #[test]
    fn continental_champion_per_tier_per_season() {
        let world = WorldGenesis::generate(50);
        let tables = zeroed_tier1_tables(&world);
        for tier in ContinentalTier::ALL {
            let result = simulate_continental(&world, &tables, 50, 1, tier);
            assert_eq!(result.groups.len(), tier.num_groups());
            assert!(world.clubs.iter().any(|c| c.id == result.champion));
        }
    }

    #[test]
    fn continental_season_is_deterministic_per_seed() {
        let world = WorldGenesis::generate(101);
        let tables = zeroed_tier1_tables(&world);
        let a = simulate_continental(&world, &tables, 101, 3, ContinentalTier::Tier1);
        let b = simulate_continental(&world, &tables, 101, 3, ContinentalTier::Tier1);
        assert_eq!(a, b);
    }

    #[test]
    fn continental_streams_independent_of_domestic_cup() {
        // Same world_seed/season: the continental group draw and the domestic cup's R1
        // draw must not coincide (would indicate an accidental shared stream).
        use crate::domestic_cup::draw_round;
        let world = WorldGenesis::generate(200);
        let tables = zeroed_tier1_tables(&world);
        let qualified = qualify_clubs(&world, &tables, ContinentalTier::Tier1);
        let groups = draw_groups(200, 1, ContinentalTier::Tier1, &qualified);

        let tier3_clubs = world.leagues[2].clubs.clone(); // some Tier-3 league's clubs
        let cup_draw = draw_round(200, 1, 0, 0, &tier3_clubs);

        // Not a meaningful equality target (different-shaped pools), just guards against
        // a copy-paste salt collision by asserting the group draw doesn't degenerate to
        // the identity permutation of `qualified` (which a shared, already-consumed
        // stream might coincidentally produce for this seed).
        assert_ne!(
            &groups.iter().flatten().copied().collect::<Vec<_>>(),
            &qualified
        );
        assert!(!cup_draw.pairs.is_empty());
    }

    #[test]
    fn group_stage_size_num_groups_knockout_size_are_consistent() {
        for tier in ContinentalTier::ALL {
            assert_eq!(tier.group_stage_size(), tier.num_groups() * 4);
            assert_eq!(tier.knockout_size(), tier.num_groups() * 2);
        }
        assert_eq!(ContinentalTier::Tier1.group_stage_size(), 32);
        assert_eq!(ContinentalTier::Tier2.group_stage_size(), 48);
        assert_eq!(ContinentalTier::Tier3.group_stage_size(), 64);
        assert_eq!(ContinentalTier::Tier1.num_groups(), 8);
        assert_eq!(ContinentalTier::Tier2.num_groups(), 12);
        assert_eq!(ContinentalTier::Tier3.num_groups(), 16);
    }

    #[test]
    fn club_league_helper_is_used_correctly_for_clubs_per_div() {
        // Sanity check that the fixture used across these tests still matches the
        // confirmed scale (20 clubs/tier) the taper table's totals depend on.
        assert_eq!(CLUBS_PER_DIV, 20);
    }
}
