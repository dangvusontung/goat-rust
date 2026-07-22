//! Youth-academy investment lane (Design round 5, Doc A §Slice 6) — a distinct lever from
//! the transfers slice: instead of buying from another club, a club spends its remaining
//! window budget on its own academy, raising the anchor round-3's youth intake rolls new
//! players' `potential_ovr` against (§6.4).
//!
//! Consumes Slice 1-2's `Club.budget` (`economy.rs`) and Slice 5's `lane_cap`/
//! `LANE_YOUTH_INVESTMENT_PCT` (`transfers.rs`) — the third of the three lane shares that
//! slice defines, spent here rather than there.

use crate::transfers::{lane_cap, LANE_YOUTH_INVESTMENT_PCT};
use crate::world::{Club, WorldGenesis, ACADEMY_BOOST_MAX};

/// Per-season decay applied to `Club::academy_boost` when investment lapses — not per
/// window, so a club gets up to two investment opportunities per season before any decay
/// bites (the integration slice calls this once per season, after both windows have landed).
const ACADEMY_BOOST_DECAY_PCT: i64 = 15;

/// Spend `spend` on one club's own academy: diminishing returns (pricier per point the
/// higher the existing boost), real spend (not a loan — deducted from `club.budget`
/// unconditionally, mirroring the transfers lane's fee accounting).
pub fn apply_academy_investment(club: &mut Club, spend: i64) {
    if spend <= 0 {
        return;
    }
    let cost_per_point = 1_000 + (club.academy_boost as i64) * 300;
    let gained = (spend / cost_per_point.max(1)) as u8;
    club.academy_boost = club
        .academy_boost
        .saturating_add(gained)
        .min(ACADEMY_BOOST_MAX);
    club.budget -= spend;
}

/// Season-end decay: sustained investment required, or the boost trends back toward 0.
pub fn decay_academy_boost(club: &mut Club) {
    club.academy_boost =
        ((club.academy_boost as i64 * (100 - ACADEMY_BOOST_DECAY_PCT)) / 100) as u8;
}

/// One full window's youth-investment lane, every club: the always-invest-the-full-lane-cap
/// policy (§6.4 — Design's simplification over a per-club "buy vs. develop" preference
/// system) means no target search is needed, unlike the transfers slice's two lanes — the
/// whole lane amount goes straight into `apply_academy_investment` for every club.
pub fn run_academy_investment_pass(world: &mut WorldGenesis) {
    for club in &mut world.clubs {
        let spend = lane_cap(club.budget, LANE_YOUTH_INVESTMENT_PCT);
        apply_academy_investment(club, spend);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::population::{apply_youth_intake, genesis};
    use crate::world::{GeneratedNation, League};
    use goat_core::roles::NUM_ROLES;
    use goat_core::tactical_identity::TacticalIdentity;
    use goat_fixed::Fixed;

    fn neutral_identity() -> TacticalIdentity {
        TacticalIdentity {
            role_weight: [Fixed::ONE; NUM_ROLES],
        }
    }

    fn mk_club(id: usize, strength: u8, budget: i64, academy_boost: u8) -> Club {
        Club {
            id,
            name: format!("Club{id}"),
            nation: 0,
            strength,
            squad_size: 20,
            tactical_identity: neutral_identity(),
            budget,
            academy_boost,
        }
    }

    // ── Slice 6 TDD anchors ──────────────────────────────────────────────────────

    #[test]
    fn academy_investment_has_diminishing_returns() {
        let mut low_boost = mk_club(0, 50, 100_000, 0);
        let mut high_boost = mk_club(1, 50, 100_000, 10);

        apply_academy_investment(&mut low_boost, 10_000);
        apply_academy_investment(&mut high_boost, 10_000);

        let gained_at_low = low_boost.academy_boost; // started at 0
        let gained_at_high = high_boost.academy_boost - 10; // started at 10

        assert!(
            gained_at_low > gained_at_high,
            "equal spend must gain fewer points at a higher starting academy_boost: {} vs {}",
            gained_at_low,
            gained_at_high
        );
    }

    #[test]
    fn academy_boost_never_exceeds_cap() {
        let mut club = mk_club(0, 50, 0, 0);
        for _ in 0..50 {
            club.budget = 1_000_000;
            apply_academy_investment(&mut club, 1_000_000);
            assert!(club.academy_boost <= ACADEMY_BOOST_MAX);
        }
        assert_eq!(
            club.academy_boost, ACADEMY_BOOST_MAX,
            "repeated large investments must saturate at the cap"
        );
    }

    #[test]
    fn academy_boost_decays_without_reinvestment() {
        let mut club = mk_club(0, 50, 0, ACADEMY_BOOST_MAX);
        let mut prev = club.academy_boost;
        let mut reached_zero = false;
        for _ in 0..30 {
            decay_academy_boost(&mut club);
            assert!(
                club.academy_boost <= prev,
                "decay must never increase academy_boost"
            );
            prev = club.academy_boost;
            if club.academy_boost == 0 {
                reached_zero = true;
                break;
            }
        }
        assert!(
            reached_zero,
            "zero spend for enough seasons must decay academy_boost to 0, got {}",
            club.academy_boost
        );
    }

    #[test]
    fn apply_academy_investment_ignores_nonpositive_spend() {
        let mut club = mk_club(0, 50, 5_000, 3);
        let before = (club.academy_boost, club.budget);
        apply_academy_investment(&mut club, 0);
        apply_academy_investment(&mut club, -1_000);
        assert_eq!(
            (club.academy_boost, club.budget),
            before,
            "zero/negative spend must be a no-op"
        );
    }

    #[test]
    fn run_academy_investment_pass_spends_exactly_the_lane_cap() {
        let mut world = WorldGenesis {
            nations: vec![GeneratedNation {
                id: 0,
                name: "Testland".into(),
                stature: 60,
                tactical_identity: neutral_identity(),
            }],
            leagues: vec![League {
                id: 0,
                nation: 0,
                tier: crate::world::DivLevel::Top,
                name: "Test League".into(),
                clubs: vec![0],
                max_clubs: 1,
            }],
            clubs: vec![mk_club(0, 50, 100_000, 0)],
        };

        let cap = lane_cap(world.clubs[0].budget, LANE_YOUTH_INVESTMENT_PCT);
        run_academy_investment_pass(&mut world);

        assert_eq!(
            world.clubs[0].budget,
            100_000 - cap,
            "the always-invest-full-lane-cap policy must spend exactly lane_cap this pass"
        );
        assert!(
            world.clubs[0].academy_boost > 0,
            "a positive lane cap must raise academy_boost off zero"
        );
    }

    // ── End-to-end regression tying this slice to round-3's existing mechanic ──────

    #[test]
    fn academy_boost_raises_effective_intake_anchor() {
        let neutral = neutral_identity();
        let make_world = |academy_boost: u8| WorldGenesis {
            nations: vec![GeneratedNation {
                id: 0,
                name: "Testland".into(),
                stature: 50,
                tactical_identity: neutral.clone(),
            }],
            leagues: vec![League {
                id: 0,
                nation: 0,
                tier: crate::world::DivLevel::Top,
                name: "Test League".into(),
                clubs: vec![0],
                max_clubs: 1,
            }],
            clubs: vec![mk_club(0, 50, 0, academy_boost)],
        };

        let plain_world = make_world(0);
        let boosted_world = make_world(15);

        let mut pop_plain = genesis(9, &plain_world);
        let mut pop_boosted = genesis(9, &boosted_world);

        for season in 1..=60u32 {
            apply_youth_intake(&mut pop_plain, &plain_world, 9, season);
            apply_youth_intake(&mut pop_boosted, &boosted_world, 9, season);
        }

        let (sum_plain, count_plain) = intake_potential_sum(&pop_plain);
        let (sum_boosted, count_boosted) = intake_potential_sum(&pop_boosted);

        assert!(
            count_plain > 0 && count_boosted > 0,
            "expected intake players in both runs"
        );
        // Compare means without floats/division: sum/count cross-multiplied.
        assert!(
            sum_boosted * count_plain as i64 > sum_plain * count_boosted as i64,
            "a nonzero academy_boost must raise the mean intake potential_ovr: {}/{} vs {}/{}",
            sum_boosted,
            count_boosted,
            sum_plain,
            count_plain
        );
    }

    fn intake_potential_sum(pop: &crate::population::Population) -> (i64, usize) {
        let mut sum = 0i64;
        let mut count = 0usize;
        for i in 0..pop.len() {
            if pop.intake_week[i] > 0 {
                sum += pop.potential_ovr[i] as i64;
                count += 1;
            }
        }
        (sum, count)
    }
}
