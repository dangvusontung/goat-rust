//! Domestic cup: single-elimination, tier-staggered entry, round-by-round random redraw.
//!
//! Bible §9 / TASK-DESIGN-round4-competitions-slice2-3 §2. Every draw (pairing, bye
//! assignment) is a pure function of `(world_seed, nation, season, round)`, on its own
//! salted stream — independent of `calendar`/`match`/`transfer`/`injury` and of the
//! continental draws in `continental.rs`. No protective seeding anywhere: two Tier-1
//! clubs entering the same round can draw each other from the moment they enter.

use crate::world::{seed_mix, ClubId, DivLevel, NationId, WorldGenesis};
use goat_rng::{GoatRng, RngSource};

/// Total rounds in the domestic cup at the confirmed 60-clubs/nation scale
/// (20 Tier-1 + 20 Tier-2 + 20 Tier-3): R1..R8, R8 is the final.
pub const CUP_ROUNDS: usize = 8;

/// Distinct domains so the draw stream and the match-result stream never collide with
/// each other or with any other subsystem's randomness, even though all of them ultimately
/// derive from the same `world_seed`.
const DRAW_SALT: u64 = 0x0000_0000_0000_0C01;
const MATCH_SALT: u64 = 0x0000_0000_0000_0C02;
const TIEBREAK_SALT: u64 = 0x0000_0000_0000_0C03;

/// Which tier's clubs enter the cup at the start of `round` (0-indexed), if any.
/// Tier 3 enters R1 (round 0), Tier 2 enters R2 (round 1), Tier 1 enters R3 (round 2) —
/// mirrors the real FA Cup's staggered entry (§2.1).
pub fn entry_tier(round: usize) -> Option<DivLevel> {
    match round {
        0 => Some(DivLevel::Third),
        1 => Some(DivLevel::Second),
        2 => Some(DivLevel::Top),
        _ => None,
    }
}

/// A single round's random draw: pairings plus the bye recipient (if the round's
/// participant count was odd).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundDraw {
    /// (home, away) pairs — home/away decided by an independent roll per pair, not by
    /// draw order (§2.2).
    pub pairs: Vec<(ClubId, ClubId)>,
    /// The club that sat out this round without playing, if the pool was odd-sized.
    pub bye: Option<ClubId>,
}

/// Draw one round's pairings, redrawn fresh every round from `(world_seed, nation,
/// season, round)` — never a fixed bracket set once at the top (§2.2). `clubs` is this
/// round's full participant pool (previous round's winners plus any new tier entrants).
///
/// Bye assignment (§2.1): when the pool is odd, one club is drawn uniformly at random
/// from the WHOLE pool to receive the bye — never preferentially a higher-tier club.
pub fn draw_round(
    world_seed: u64,
    season: u32,
    nation: NationId,
    round: usize,
    clubs: &[ClubId],
) -> RoundDraw {
    let seed = seed_mix(
        world_seed ^ (season as u64).rotate_left(11) ^ (round as u64).rotate_left(29),
        DRAW_SALT,
        nation as u64,
    );
    draw_bracket_round(seed, clubs)
}

/// Shared bracket-round drawing primitive: Fisher-Yates shuffle, bye-if-odd (uniformly
/// random over the whole pool), then pair sequentially with an independent home/away
/// roll per pair. Pulled out of `draw_round` so `continental.rs`'s Tier-2 knockout can
/// reuse the exact same odd-bracket/bye handling (§3.3: "a direct, deliberate reuse of
/// Slice 2's bracket-oddness handling rather than a second bye mechanism invented for
/// this tier") while still drawing from its own, independently-seeded stream — the
/// caller computes `seed` from its own salted domain, so no two callers ever share a
/// stream even though they share this shuffling code.
pub fn draw_bracket_round(seed: u64, clubs: &[ClubId]) -> RoundDraw {
    let mut rng = GoatRng::new(seed);

    let mut pool = clubs.to_vec();
    let bye = if pool.len() % 2 == 1 {
        let idx = rng.next_range_u32(0, pool.len() as u32 - 1) as usize;
        Some(pool.remove(idx))
    } else {
        None
    };

    // Fisher-Yates shuffle of the remaining (even-sized) pool.
    for i in (1..pool.len()).rev() {
        let j = rng.next_range_u32(0, i as u32) as usize;
        pool.swap(i, j);
    }

    let pairs = pool
        .chunks_exact(2)
        .map(|pair| {
            // Independent home/away roll per pair — pairing order itself carries no
            // home/away signal.
            if rng.next_range_u32(0, 1) == 1 {
                (pair[1], pair[0])
            } else {
                (pair[0], pair[1])
            }
        })
        .collect();

    RoundDraw { pairs, bye }
}

/// One resolved tie in a cup round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CupTie {
    pub home: ClubId,
    pub away: ClubId,
    pub home_goals: u32,
    pub away_goals: u32,
    pub winner: ClubId,
}

/// A fully-resolved cup round: every tie's result plus who (if anyone) had the bye.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CupRoundResult {
    pub round: usize,
    pub ties: Vec<CupTie>,
    pub bye: Option<ClubId>,
}

impl CupRoundResult {
    /// Clubs advancing to the next round: every tie's winner, plus the bye recipient.
    pub fn winners(&self) -> Vec<ClubId> {
        let mut w: Vec<ClubId> = self.ties.iter().map(|t| t.winner).collect();
        if let Some(b) = self.bye {
            w.push(b);
        }
        w
    }
}

/// Resolve one drawn round's ties using `sim_team_match`, on its own match-result
/// stream (independent of the draw stream above). Drawn ties are broken by an
/// unbiased coin flip on a third, dedicated stream — real shootouts are close to a
/// 50/50 proposition regardless of the two sides' run-of-play strength.
fn resolve_round(
    world: &WorldGenesis,
    world_seed: u64,
    season: u32,
    nation: NationId,
    round: usize,
    draw: &RoundDraw,
) -> CupRoundResult {
    let match_seed = seed_mix(
        world_seed ^ (season as u64).rotate_left(13) ^ (round as u64).rotate_left(31),
        MATCH_SALT,
        nation as u64,
    );
    let mut match_rng = GoatRng::new(match_seed);
    let tiebreak_seed = seed_mix(
        world_seed ^ (season as u64).rotate_left(17) ^ (round as u64).rotate_left(37),
        TIEBREAK_SALT,
        nation as u64,
    );
    let mut tiebreak_rng = GoatRng::new(tiebreak_seed);

    let ties = draw
        .pairs
        .iter()
        .map(|&(home, away)| {
            let (hg, ag) = crate::season::sim_team_match(
                world.clubs[home].strength,
                world.clubs[away].strength,
                &mut match_rng,
            );
            let winner = break_tie(home, away, hg, ag, &mut tiebreak_rng);
            CupTie {
                home,
                away,
                home_goals: hg,
                away_goals: ag,
                winner,
            }
        })
        .collect();

    CupRoundResult {
        round,
        ties,
        bye: draw.bye,
    }
}

/// Decide a knockout tie's winner from a (possibly aggregate) scoreline: higher goals
/// wins outright; a level scoreline is broken by an unbiased coin flip on its own
/// stream — real extra-time-then-penalties outcomes are close to a 50/50 proposition
/// regardless of the two sides' run-of-play strength. Shared by `continental.rs`'s
/// single-leg and two-legged-aggregate knockout ties.
pub fn break_tie(
    home: ClubId,
    away: ClubId,
    home_goals: u32,
    away_goals: u32,
    rng: &mut GoatRng,
) -> ClubId {
    if home_goals > away_goals {
        home
    } else if away_goals > home_goals {
        away
    } else if rng.next_range_u32(0, 1) == 0 {
        home
    } else {
        away
    }
}

/// Entrants for `round` in one nation's cup: the previous round's winners plus any
/// new-tier entrants due this round (§2.1). Round 0 has no "previous winners".
fn round_pool(
    world: &WorldGenesis,
    membership: &[Vec<ClubId>],
    nation: NationId,
    round: usize,
    previous_winners: &[ClubId],
) -> Vec<ClubId> {
    let mut pool = previous_winners.to_vec();
    if let Some(tier) = entry_tier(round) {
        pool.extend(tier_clubs(world, membership, nation, tier));
    }
    pool
}

/// Current-season membership of one nation's tier (post promotion/relegation — the
/// caller resolves membership, same idiom as `fixtures::generate_fixtures`).
pub fn tier_clubs(
    world: &WorldGenesis,
    membership: &[Vec<ClubId>],
    nation: NationId,
    tier: DivLevel,
) -> Vec<ClubId> {
    world
        .leagues
        .iter()
        .find(|l| l.nation == nation && l.tier as usize == tier as usize)
        .map(|l| membership[l.id].clone())
        .unwrap_or_default()
}

/// The full resolved domestic cup for one nation, one season: every round's draw and
/// results, plus the champion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CupSeasonResult {
    pub nation: NationId,
    pub rounds: Vec<CupRoundResult>,
    pub champion: ClubId,
}

/// Simulate one nation's entire domestic cup for `season`, from R1 through the final.
/// Deterministic in `(world_seed, season, nation, membership)` — "generated but
/// consistent", never persisted (§9).
pub fn simulate_domestic_cup(
    world: &WorldGenesis,
    membership: &[Vec<ClubId>],
    world_seed: u64,
    season: u32,
    nation: NationId,
) -> CupSeasonResult {
    let mut rounds = Vec::with_capacity(CUP_ROUNDS);
    let mut winners: Vec<ClubId> = Vec::new();

    for round in 0..CUP_ROUNDS {
        let pool = round_pool(world, membership, nation, round, &winners);
        let draw = draw_round(world_seed, season, nation, round, &pool);
        let result = resolve_round(world, world_seed, season, nation, round, &draw);
        winners = result.winners();
        rounds.push(result);
    }

    let champion = winners
        .first()
        .copied()
        .expect("the final round must leave exactly one club standing");
    debug_assert_eq!(winners.len(), 1, "the final round must leave one champion");

    CupSeasonResult {
        nation,
        rounds,
        champion,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{League, CLUBS_PER_DIV};

    fn genesis_membership(world: &WorldGenesis) -> Vec<Vec<ClubId>> {
        world.static_league_clubs()
    }

    #[test]
    fn entry_table_matches_confirmed_shape() {
        // §2.1's table: R1 Tier3 (20), R2 Tier2+10=30, R3 Tier1+15=35 (1 bye -> 18),
        // R4 18, R5 9 (1 bye -> 5), R6 5 (1 bye -> 3), R7 3 (1 bye -> 2), R8 2 -> 1.
        let world = WorldGenesis::generate(7);
        let membership = genesis_membership(&world);
        let nation = 0;
        let mut winners: Vec<ClubId> = Vec::new();
        let expected_participants = [20, 30, 35, 18, 9, 5, 3, 2];
        let expected_winners = [10, 15, 18, 9, 5, 3, 2, 1];
        for round in 0..CUP_ROUNDS {
            let pool = round_pool(&world, &membership, nation, round, &winners);
            assert_eq!(
                pool.len(),
                expected_participants[round],
                "round {round} participant count"
            );
            let draw = draw_round(7, 1, nation, round, &pool);
            let result = resolve_round(&world, 7, 1, nation, round, &draw);
            winners = result.winners();
            assert_eq!(
                winners.len(),
                expected_winners[round],
                "round {round} winners"
            );
        }
        assert_eq!(winners.len(), 1);
    }

    #[test]
    fn domestic_cup_draw_is_deterministic_per_seed() {
        let clubs: Vec<ClubId> = (0..20).collect();
        let a = draw_round(42, 1, 0, 2, &clubs);
        let b = draw_round(42, 1, 0, 2, &clubs);
        assert_eq!(a, b);
    }

    #[test]
    fn different_rounds_or_seasons_redraw_differently() {
        let clubs: Vec<ClubId> = (0..20).collect();
        let r2 = draw_round(42, 1, 0, 2, &clubs);
        let r3 = draw_round(42, 1, 0, 3, &clubs);
        assert_ne!(
            r2.pairs, r3.pairs,
            "redrawn every round, not a fixed bracket"
        );
        let s1 = draw_round(42, 1, 0, 2, &clubs);
        let s2 = draw_round(42, 2, 0, 2, &clubs);
        assert_ne!(s1.pairs, s2.pairs, "redrawn every season");
    }

    #[test]
    fn two_tier1_clubs_can_meet_in_their_entry_round() {
        // R3 (round idx 2): 20 real Tier-1 clubs (ids 0..20) join 15 previous winners
        // (synthetic ids 1000..1015). No protective separation: over many seeds, at
        // least one draw pairs two of the 20 Tier-1 entrants together.
        let tier1: Vec<ClubId> = (0..20).collect();
        let winners: Vec<ClubId> = (1000..1015).collect();
        let mut pool = tier1.clone();
        pool.extend(winners);

        let mut saw_two_tier1_together = false;
        for seed in 0u64..500 {
            let draw = draw_round(seed, 1, 0, 2, &pool);
            if draw
                .pairs
                .iter()
                .any(|&(a, b)| tier1.contains(&a) && tier1.contains(&b))
            {
                saw_two_tier1_together = true;
                break;
            }
        }
        assert!(
            saw_two_tier1_together,
            "two Tier-1 entrants must be able to draw each other in their entry round"
        );
    }

    #[test]
    fn bracket_converges_to_one_champion_per_nation_per_season() {
        let world = WorldGenesis::generate(11);
        let membership = genesis_membership(&world);
        for nation in 0..3 {
            let result = simulate_domestic_cup(&world, &membership, 11, 1, nation);
            assert_eq!(result.rounds.len(), CUP_ROUNDS);
            // Every round strictly shrinks its OWN field (ties + a possible bye net
            // fewer winners than entrants); the final round must leave exactly one.
            for round in &result.rounds {
                let entrants = round.ties.len() * 2 + usize::from(round.bye.is_some());
                let remaining = round.winners().len();
                assert!(
                    remaining < entrants,
                    "round {} did not shrink its own field",
                    round.round
                );
            }
            let champions = result.rounds.last().unwrap().winners();
            assert_eq!(champions.len(), 1);
            assert!(
                world
                    .clubs_for_nation(nation)
                    .any(|c| c.id == result.champion),
                "champion must be a club from this nation"
            );
        }
    }

    #[test]
    fn cup_is_deterministic_per_seed() {
        let world = WorldGenesis::generate(23);
        let membership = genesis_membership(&world);
        let a = simulate_domestic_cup(&world, &membership, 23, 4, 2);
        let b = simulate_domestic_cup(&world, &membership, 23, 4, 2);
        assert_eq!(a, b);
    }

    #[test]
    fn bye_recipient_is_uniformly_random_not_tier_biased() {
        // Synthetic R3-shaped pool: ids 0..20 stand in for "Tier-1", ids 20..35 for
        // "everyone else" — the same 35-club, odd-sized pool R3 produces. Tally which
        // bucket receives the bye across many seeds; it should track each bucket's
        // share of the pool (20/35 vs 15/35), not systematically favor one tier.
        let pool: Vec<ClubId> = (0..35).collect();
        let mut tier1_byes = 0u32;
        let mut other_byes = 0u32;
        const TRIALS: u64 = 4000;
        for seed in 0..TRIALS {
            let draw = draw_round(seed, 1, 0, 2, &pool);
            match draw.bye {
                Some(b) if b < 20 => tier1_byes += 1,
                Some(_) => other_byes += 1,
                None => panic!("odd pool must always produce a bye"),
            }
        }
        let total = tier1_byes + other_byes;
        assert_eq!(total, TRIALS as u32);
        let tier1_share = tier1_byes as f64 / total as f64;
        // Expected share is 20/35 ≈ 0.571; allow generous statistical slack.
        assert!(
            (0.45..0.70).contains(&tier1_share),
            "bye recipient share for the 20/35 bucket was {tier1_share}, expected ~0.571 \
             (uniformly random, not tier-biased)"
        );
    }

    #[test]
    fn even_pool_never_produces_a_bye() {
        let clubs: Vec<ClubId> = (0..20).collect();
        for seed in 0..50u64 {
            let draw = draw_round(seed, 1, 0, 0, &clubs);
            assert!(draw.bye.is_none());
            assert_eq!(draw.pairs.len(), 10);
        }
    }

    #[test]
    fn tier_clubs_reads_current_membership() {
        let world = WorldGenesis::generate(3);
        let mut membership = genesis_membership(&world);
        // Simulate a promotion: move the first Tier-2 club of nation 0 into Tier-1.
        let nation = 0;
        let tier2_league = world
            .leagues
            .iter()
            .find(|l: &&League| l.nation == nation && l.tier == DivLevel::Second)
            .unwrap();
        let moved = membership[tier2_league.id].remove(0);
        let tier1_league_id = world
            .leagues
            .iter()
            .find(|l: &&League| l.nation == nation && l.tier == DivLevel::Top)
            .unwrap()
            .id;
        membership[tier1_league_id].push(moved);
        membership[tier1_league_id].remove(0); // keep the tier at CLUBS_PER_DIV size

        let tier1 = tier_clubs(&world, &membership, nation, DivLevel::Top);
        assert_eq!(tier1.len(), CLUBS_PER_DIV);
        assert!(tier1.contains(&moved));
    }
}
