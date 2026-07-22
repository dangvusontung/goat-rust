//! Deterministic bidding-round auction & transfer execution (Design round 5, Doc A,
//! Slice 5) — the riskiest, most novel piece of the club-economy design, isolated in its
//! own module on purpose (a real design choice: ascending-round auction, not sealed-bid or
//! instant-highest-bidder).
//!
//! Consumes Slice 1-2's `market_valuation`/`Club.budget` (`economy.rs`) and Slice 3-4's
//! target search (`scouting.rs`). Every club in a pass computes its target against the
//! pass's *opening* snapshot — no club's search result depends on another club's search
//! happening "before" or "after" it in iteration order. Transfers are then resolved
//! contested-player-by-contested-player and applied as one batch at the pass's end, so
//! iterating clubs in any order produces the same result from the same seed (bible §9
//! determinism).

use std::collections::HashMap;

use goat_rng::{GoatRng, RngSource};

use crate::economy::market_valuation;
use crate::population::Population;
use crate::scouting::{
    candidates_by_position, gem_hunt_target, gem_targets_by_position, weakest_position_target,
};
use crate::world::{ClubId, WorldGenesis};

// ── 5.1 — Lane caps: three shares of one real budget, spent in priority order ───────────

/// Share of a club's *current* budget the weakest-position lane may draw on this pass.
const LANE_WEAKEST_POSITION_PCT: i64 = 50;
/// Share of a club's *current* budget the gem-hunt lane may draw on this pass.
const LANE_GEM_HUNT_PCT: i64 = 30;
/// Reserved share for the academy slice's youth-investment lane — not spent here. Kept
/// alongside its two siblings purely as documentation that 50 + 30 + 20 accounts for the
/// whole budget across all three lanes; the academy slice reads its own copy of this number.
#[allow(dead_code)]
const LANE_YOUTH_INVESTMENT_PCT: i64 = 20;

/// One lane's spending cap this pass: a share of the club's *current* budget, computed
/// fresh each time a lane runs — not a separate reserved sub-ledger. Passes execute
/// strictly in priority order (weakest-position, then gem-hunt, then the academy slice's
/// youth-investment), so a club that spends in an earlier pass automatically has a smaller
/// `budget` — and therefore smaller caps — for the next one, with no double-booking risk.
fn lane_cap(club_budget: i64, pct: i64) -> i64 {
    (club_budget.max(0) * pct) / 100
}

// ── 5.2 — Bid ceiling: budget + need + quality, one club's max willingness to pay ───────

/// A confirmed weakest-position gap is worth overpaying 30% for.
const NEED_MULT_WEAKEST_POSITION_PCT: i64 = 130;
/// Gem-hunting is opportunistic — a smaller overpay premium than a confirmed gap.
const NEED_MULT_GEM_HUNT_PCT: i64 = 110;

/// How much one club is willing to bid for one target this lane. Never exceeds the club's
/// actual (post-earlier-lane) budget, regardless of how much the need multiplier wants to
/// add — so a distressed club (negative `budget`) naturally drops out of bidding without
/// any special-case branch.
fn bid_ceiling(lane_budget: i64, need_mult_pct: i64, club_budget: i64) -> i64 {
    (lane_budget * need_mult_pct / 100).min(club_budget.max(0))
}

// ── 5.3 — One pass, one snapshot: order-independence ────────────────────────────────────

/// Which of the two transfer-search lanes a pass runs. Youth-investment (the third lane
/// from 5.1) is spent by the academy slice, not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferLane {
    WeakestPosition,
    GemHunt,
}

impl TransferLane {
    fn pct(&self) -> i64 {
        match self {
            TransferLane::WeakestPosition => LANE_WEAKEST_POSITION_PCT,
            TransferLane::GemHunt => LANE_GEM_HUNT_PCT,
        }
    }

    fn need_mult_pct(&self) -> i64 {
        match self {
            TransferLane::WeakestPosition => NEED_MULT_WEAKEST_POSITION_PCT,
            TransferLane::GemHunt => NEED_MULT_GEM_HUNT_PCT,
        }
    }
}

/// Build `club_id -> Vec<population index>` for the whole population (one pass). A private
/// local copy of the same tiny helper `batch_tick.rs` already has for its own use — no
/// shared "squads by club" util exists in this crate yet (the same one-copy-per-module
/// idiom this file's `auction_seed` below follows for the seed-mixing pattern).
fn squads_by_club(pop: &Population, num_clubs: usize) -> Vec<Vec<usize>> {
    let mut squads: Vec<Vec<usize>> = vec![Vec::new(); num_clubs];
    for idx in 0..pop.len() {
        squads[pop.club[idx] as usize].push(idx);
    }
    squads
}

/// One full pass: every club's target (computed off the shared opening snapshot), grouped
/// by contested player, each contested player's auction resolved independently, all
/// resulting transfers applied together at the end — see module doc for why this ordering
/// matters.
pub fn run_transfer_pass(
    pop: &mut Population,
    world: &mut WorldGenesis,
    world_seed: u64,
    season: u32,
    window: u8, // 0 = winter, 1 = summer — folded into the auction seed (5.4)
    lane: TransferLane,
) {
    let elapsed_weeks = season * 52;
    let candidates = candidates_by_position(pop, elapsed_weeks);
    let gem_lists = gem_targets_by_position(pop, &candidates, elapsed_weeks);
    let squads = squads_by_club(pop, world.clubs.len());

    let mut targets: HashMap<usize, Vec<ClubId>> = HashMap::new();
    for club in &world.clubs {
        let cap = lane_cap(club.budget, lane.pct());
        let ceiling = bid_ceiling(cap, lane.need_mult_pct(), club.budget);
        let target = match lane {
            TransferLane::WeakestPosition => weakest_position_target(
                club.id,
                pop,
                &squads[club.id],
                &candidates,
                ceiling,
                elapsed_weeks,
            ),
            TransferLane::GemHunt => {
                gem_hunt_target(club.id, pop, &gem_lists, ceiling, elapsed_weeks)
            }
        };
        if let Some(idx) = target {
            targets.entry(idx).or_default().push(club.id);
        }
    }

    // Resolve each contested player independently; apply all transfers as one batch.
    let mut transfers = Vec::new();
    for (player_idx, bidder_clubs) in targets {
        let valuation = market_valuation(
            pop.current_ovr(player_idx, elapsed_weeks),
            pop.potential_ovr[player_idx],
            pop.age_years_at(player_idx, elapsed_weeks),
        );
        let bidders: Vec<(ClubId, i64)> = bidder_clubs
            .iter()
            .map(|&c| {
                (
                    c,
                    bid_ceiling(
                        lane_cap(world.clubs[c].budget, lane.pct()),
                        lane.need_mult_pct(),
                        world.clubs[c].budget,
                    ),
                )
            })
            .filter(|&(_, ceiling)| ceiling >= valuation)
            .collect();
        let mut rng = GoatRng::new(auction_seed(
            world_seed,
            season,
            window,
            pop.seed[player_idx],
        ));
        if let Some((winner, fee)) = resolve_auction(&bidders, valuation, &mut rng) {
            transfers.push((player_idx, winner, fee));
        }
    }

    for (player_idx, winner, fee) in transfers {
        let seller = pop.club[player_idx] as usize;
        pop.club[player_idx] = winner as u16;
        // Conservation: money moves within the closed club economy, never created/destroyed
        // by a transfer fee (only `total_income`, foundation slice 1.2, creates new money) —
        // the fee sums to zero across (buyer, seller), the invariant TDD anchor 5.5 checks.
        world.clubs[winner].budget -= fee;
        world.clubs[seller].budget += fee;
    }
}

// ── 5.4 — Auction resolution: deterministic ascending rounds ────────────────────────────

/// This file's own local copy of the seed-mixing idiom (`world.rs::seed_mix`,
/// `population.rs::player_seed`/`intake_player_seed`) — a forked stream so the auction's
/// tie-break RNG never shares state with match/transfer-search/injury/calendar RNG.
fn auction_seed(world_seed: u64, season: u32, window: u8, player_seed: u64) -> u64 {
    world_seed
        ^ (season as u64)
            .rotate_left(17)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (window as u64)
            .rotate_left(29)
            .wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ player_seed
            .rotate_left(41)
            .wrapping_mul(0x1656_667B_19E3_779F)
}

/// Price climbs 8% per contested round.
const AUCTION_RAISE_PCT: i64 = 8;

/// `interested`: (club, bid_ceiling) pairs already filtered to `ceiling >= valuation`.
/// Returns `(winning_club, final_fee)`, or `None` if nobody clears the valuation.
///
/// Ascending-round auction, not sealed-bid or instant-highest-bidder: an uncontested target
/// costs exactly its valuation (no reason to bid against yourself); a target two or more
/// clubs want costs strictly more, in visible, replayable increments, with a seeded (not
/// order-dependent) tie-break only when ceilings cluster exactly below the final raise.
fn resolve_auction(
    interested: &[(ClubId, i64)],
    valuation: i64,
    rng: &mut GoatRng,
) -> Option<(ClubId, i64)> {
    match interested.len() {
        0 => None,
        1 => Some((interested[0].0, valuation)),
        _ => {
            let mut price = valuation;
            let mut remaining: Vec<(ClubId, i64)> = interested.to_vec();
            loop {
                remaining.retain(|&(_, ceiling)| ceiling >= price);
                if remaining.len() <= 1 {
                    break;
                }
                price += (price * AUCTION_RAISE_PCT) / 100 + 1; // +1 guards a zero-valuation stall
            }
            if remaining.is_empty() {
                // Everyone dropped in the same round (ceilings clustered exactly below the
                // final raise) — deterministic seeded tie-break among the original bidders,
                // at the price band they all cleared one round earlier.
                let idx = rng.next_range_u32(0, interested.len() as u32 - 1) as usize;
                Some((interested[idx].0, price))
            } else {
                Some((remaining[0].0, price))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Club;
    use goat_core::roles::NUM_ROLES;
    use goat_core::tactical_identity::TacticalIdentity;
    use goat_fixed::Fixed;

    fn neutral_identity() -> TacticalIdentity {
        TacticalIdentity {
            role_weight: [Fixed::ONE; NUM_ROLES],
        }
    }

    fn mk_club(id: ClubId, budget: i64) -> Club {
        Club {
            id,
            name: String::new(),
            nation: 0,
            strength: 50,
            squad_size: 5,
            tactical_identity: neutral_identity(),
            budget,
        }
    }

    /// `age_years == 28` sits in the 26..=31 development peak plateau, so
    /// `current_ovr == potential_ovr` exactly — the same trick `economy.rs`/`scouting.rs`'s
    /// own tests use to dial in an exact `current_ovr` via `potential_ovr` alone.
    fn push_peaked(pop: &mut Population, club: u16, position: u8, ovr: u8) -> usize {
        let idx = pop.len();
        pop.seed.push(idx as u64);
        pop.club.push(club);
        pop.nation.push(0);
        pop.position.push(position);
        pop.birth_age_weeks.push(28 * 52);
        pop.potential_ovr.push(ovr);
        pop.intake_week.push(0);
        pop.career_goals.push(0);
        pop.career_apps.push(0);
        pop.career_titles.push(0);
        idx
    }

    const DEFENDER: u8 = 0;
    const MIDFIELDER: u8 = 1;
    const FORWARD: u8 = 2;

    /// Three clubs: 0 and 1 both have a weak forward (own_best 30) and a big budget, 2 owns
    /// the one available forward upgrade (70) on a tiny budget — a clean contested auction
    /// with club 0's larger budget guaranteeing a distinct (non-tied) winner.
    fn build_contested_scenario() -> (Population, WorldGenesis, usize) {
        let mut pop = Population::default();
        for club in [0u16, 1u16] {
            push_peaked(&mut pop, club, DEFENDER, 80);
            push_peaked(&mut pop, club, DEFENDER, 80);
            push_peaked(&mut pop, club, MIDFIELDER, 80);
            push_peaked(&mut pop, club, MIDFIELDER, 80);
            push_peaked(&mut pop, club, FORWARD, 30);
        }
        push_peaked(&mut pop, 2, DEFENDER, 80);
        push_peaked(&mut pop, 2, DEFENDER, 80);
        push_peaked(&mut pop, 2, MIDFIELDER, 80);
        push_peaked(&mut pop, 2, MIDFIELDER, 80);
        let target = push_peaked(&mut pop, 2, FORWARD, 70);

        let world = WorldGenesis {
            nations: Vec::new(),
            leagues: Vec::new(),
            clubs: vec![
                mk_club(0, 12_000_000),
                mk_club(1, 8_000_000),
                mk_club(2, 100),
            ],
        };
        (pop, world, target)
    }

    // ── Slice 5 TDD anchors ──────────────────────────────────────────────────────

    #[test]
    fn uncontested_target_pays_exactly_valuation() {
        let mut rng = GoatRng::new(42);
        let valuation = 4_000;
        let result = resolve_auction(&[(0, 10_000)], valuation, &mut rng);
        assert_eq!(result, Some((0, valuation)));
    }

    #[test]
    fn contested_target_pays_strictly_more_than_valuation() {
        let valuation = 1_000;

        let mut rng_a = GoatRng::new(7);
        let two = resolve_auction(&[(0, 5_000), (1, 3_000)], valuation, &mut rng_a).unwrap();
        assert!(
            two.1 > valuation,
            "a contested target must cost strictly more than its valuation"
        );

        let mut rng_b = GoatRng::new(7);
        let three =
            resolve_auction(&[(0, 5_000), (1, 3_000), (2, 4_000)], valuation, &mut rng_b).unwrap();
        assert!(three.1 > valuation);
        assert!(
            three.1 > two.1,
            "adding a stronger second-place bidder must raise the fee further: {two:?} vs {three:?}"
        );
    }

    #[test]
    fn auction_result_is_order_independent() {
        let valuation = 1_000;
        let seed = 99;

        let mut rng_a = GoatRng::new(seed);
        let a = resolve_auction(&[(0, 5_000), (1, 3_000), (2, 4_000)], valuation, &mut rng_a);

        let mut rng_b = GoatRng::new(seed);
        let b = resolve_auction(&[(2, 4_000), (0, 5_000), (1, 3_000)], valuation, &mut rng_b);

        assert_eq!(
            a, b,
            "shuffling the interested slice's input order must not change the result"
        );
    }

    #[test]
    fn auction_is_deterministic_per_seed() {
        let s1 = auction_seed(1, 2, 0, 42);
        let s2 = auction_seed(1, 2, 0, 42);
        assert_eq!(s1, s2, "identical inputs must produce identical seeds");

        let s3 = auction_seed(1, 2, 0, 43);
        assert_ne!(
            s1, s3,
            "a different player_seed must derive an independent seed"
        );
    }

    #[test]
    fn budget_conservation_across_a_transfer() {
        let (mut pop, mut world, target) = build_contested_scenario();
        let buyer_before = world.clubs[0].budget;
        let seller_before = world.clubs[2].budget;

        run_transfer_pass(&mut pop, &mut world, 1, 0, 0, TransferLane::WeakestPosition);

        assert_eq!(
            pop.club[target], 0,
            "the higher-budget bidder must win the contested target"
        );
        let fee = world.clubs[2].budget - seller_before;
        assert!(fee > 0, "a winning transfer must move a positive fee");
        assert_eq!(
            world.clubs[0].budget,
            buyer_before - fee,
            "buyer's budget must drop by exactly the fee"
        );
        assert_eq!(
            buyer_before + seller_before,
            world.clubs[0].budget + world.clubs[2].budget,
            "money must move within the closed economy, never be created or destroyed"
        );
    }

    #[test]
    fn losing_bidders_budget_unaffected() {
        let (mut pop, mut world, _target) = build_contested_scenario();
        let loser_before = world.clubs[1].budget;

        run_transfer_pass(&mut pop, &mut world, 1, 0, 0, TransferLane::WeakestPosition);

        assert_eq!(
            world.clubs[1].budget, loser_before,
            "a club that bids but doesn't win must have an unchanged budget"
        );
    }

    #[test]
    fn negative_budget_club_never_bids() {
        let ceiling = bid_ceiling(
            lane_cap(-500, LANE_WEAKEST_POSITION_PCT),
            NEED_MULT_WEAKEST_POSITION_PCT,
            -500,
        );
        assert_eq!(
            ceiling, 0,
            "a distressed club's bid ceiling must clamp to zero, not go negative"
        );
    }
}
