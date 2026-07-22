//! Club economy — budget accrual and market valuation (Design round 5, Doc A, Slice 1-2:
//! the foundation slice every later club-economy slice — scouting, auction, academy —
//! reads `Club.budget` and/or calls `market_valuation`).
//!
//! Not a fully simulated revenue model: every income/expense number is a formula over
//! already-cheap fields (`strength`, `DivLevel`, squad OVR/size), not per-player wage
//! contracts or attendance-tied ticket sales (bible §9.1's "background growth is
//! formula-driven" discipline). AI-vs-AI only — no PC-facing UI, no change to Phase 8's
//! PC contract/negotiation machinery in `goat-core`.

use crate::population::Population;
use crate::world::{Club, DivLevel};

/// One additive contributor to a club's per-window income. Today only this one exists;
/// future rounds add sibling functions (sponsorship, matchday/ticket sales, shirt sales,
/// prize money — bible §7.2's "rich identity... finances" list) to `total_income`'s sum.
/// Every caller of `total_income` is untouched when a new contributor is added — that's
/// the whole point of the abstraction.
///
/// `pub(crate)`, not private: `WorldGenesis::generate`'s genesis seeding (1.6) calls this
/// directly from `world.rs`, a sibling module in the same crate.
pub(crate) fn tier_baseline_income(strength: u8, tier: DivLevel) -> i64 {
    let tier_mult: i64 = match tier {
        DivLevel::Top => 12,   // TV money, prestige sponsorship — top-flight clubs earn
        DivLevel::Second => 4, // far more per point of strength than lower tiers
        DivLevel::Third => 1,
    };
    (strength as i64) * tier_mult * 20 // scaled to £k units — see task doc §1.3
}

/// The one number every spending formula in this round reads. Today: `tier_baseline_income`
/// alone. Composing a new source later is a one-line addition here, nothing else changes.
pub fn total_income(club: &Club, tier: DivLevel) -> i64 {
    tier_baseline_income(club.strength, tier)
    // future: + sponsorship_income(club)
    // future: + matchday_income(club, attendance_proxy)
    // future: + shirt_sales_income(club, stature)
    // future: + prize_money_income(club, this_season_results)
}

/// Abstracted wage cost for one window: proportional to squad quality (mean current OVR)
/// and squad size. Elite squads cost more to run, not just to buy into. No new `Population`
/// column — wages stay a per-window deduction against `total_income`, not per-player
/// contract data (Ground rules).
pub(crate) fn window_wage_deduction(pop: &Population, squad: &[usize], elapsed_weeks: u32) -> i64 {
    if squad.is_empty() {
        return 0;
    }
    let mean_ovr = pop.live_strength_from_squad(squad, elapsed_weeks) as i64; // 1-99
    let per_player_wage = mean_ovr * mean_ovr / 10; // superlinear — elite wages compound
    per_player_wage * squad.len() as i64
}

/// Top up a club's budget for one transfer window: income in, wages out. `squad` is the
/// list of population indices currently at this club — the caller builds it (e.g. filter
/// `pop.club[i] == club.id`), since every prior round's club-scoped code already does this
/// ad hoc and there's no single shared helper for it yet.
///
/// `budget` can legitimately go negative — an overspent club in financial distress is a
/// meaningful state a later slice's bid-ceiling formula reads and naturally excludes from
/// bidding, not a bug to clamp away.
pub fn open_transfer_window(
    club: &mut Club,
    pop: &Population,
    squad: &[usize],
    tier: DivLevel,
    elapsed_weeks: u32,
) {
    club.budget += total_income(club, tier) - window_wage_deduction(pop, squad, elapsed_weeks);
}

/// The market pays a *little* for scouted upside, not its full eventual value — see
/// `market_valuation`'s doc comment for why.
const POTENTIAL_HINT_WEIGHT: i64 = 3;

/// A player's transfer valuation (the selling club's floor/reserve price), £k. Deliberately
/// weights *current* ability far more than *potential* — this underpricing of unrealized
/// potential is not a bug, it's the exact market inefficiency a sibling slice's
/// gem-hunting lane exploits: round-3's outlier roll guarantees roughly 1-in-50 players at
/// *any* club (including bottom-tier minnows) a `potential_ovr` independent of club
/// strength, and because this formula prices mostly off *current* OVR (still low for a
/// young outlier prospect), that prospect is cheap despite a high ceiling — an actual
/// buyer for round-3's "unearthed at a nobody club" story, not a no-op.
pub fn market_valuation(current_ovr: u8, potential_ovr: u8, age: u32) -> i64 {
    let ovr = current_ovr as i64;
    let base = ovr * ovr; // superlinear — elite current ability costs disproportionately more
    let ceiling_hint = (potential_ovr as i64 - ovr).max(0) * POTENTIAL_HINT_WEIGHT;
    ((base + ceiling_hint) * age_value_pct(age)) / 100
}

fn age_value_pct(age: u32) -> i64 {
    match age {
        0..=20 => 60,   // unproven teenager discount
        21..=29 => 100, // peak transfer-value years
        30..=33 => 65,
        _ => 30,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use goat_core::roles::NUM_ROLES;
    use goat_core::tactical_identity::TacticalIdentity;
    use goat_fixed::Fixed;

    fn neutral_identity() -> TacticalIdentity {
        TacticalIdentity {
            role_weight: [Fixed::ONE; NUM_ROLES],
        }
    }

    // ── Slice 1 TDD anchors ──────────────────────────────────────────────────────

    #[test]
    fn total_income_is_monotonic_in_strength_and_tier() {
        let mk = |strength: u8| Club {
            id: 0,
            name: String::new(),
            nation: 0,
            strength,
            squad_size: 20,
            tactical_identity: neutral_identity(),
            budget: 0,
            academy_boost: 0,
        };

        for tier in DivLevel::ALL {
            let low = total_income(&mk(20), tier);
            let high = total_income(&mk(80), tier);
            assert!(
                high > low,
                "higher strength must yield higher total_income for tier {tier:?}"
            );
        }

        let strength = 50;
        let club = mk(strength);
        let top = total_income(&club, DivLevel::Top);
        let second = total_income(&club, DivLevel::Second);
        let third = total_income(&club, DivLevel::Third);
        assert!(
            top > second,
            "Top tier must out-earn Second at equal strength"
        );
        assert!(
            second > third,
            "Second tier must out-earn Third at equal strength"
        );
    }

    #[test]
    fn budget_can_go_negative_and_stays_negative_until_income_recovers() {
        // A weak Third-tier club (tiny income) fielding an elite, expensive squad — wages
        // outstrip income every window.
        let mut club = Club {
            id: 0,
            name: String::new(),
            nation: 0,
            strength: 5,
            squad_size: 20,
            tactical_identity: neutral_identity(),
            budget: 100,
            academy_boost: 0,
        };
        let mut pop = Population::default();
        let squad: Vec<usize> = (0..20)
            .map(|slot| {
                let idx = pop.len();
                pop.seed.push(idx as u64);
                pop.club.push(0);
                pop.nation.push(0);
                pop.position.push((slot % 3) as u8);
                pop.birth_age_weeks.push(28 * 52); // peak plateau -> current_ovr == potential
                pop.potential_ovr.push(90);
                pop.intake_week.push(0);
                pop.career_goals.push(0);
                pop.career_apps.push(0);
                pop.career_titles.push(0);
                idx
            })
            .collect();

        let mut expected = club.budget;
        for _ in 0..5 {
            let income = total_income(&club, DivLevel::Third);
            let wages = window_wage_deduction(&pop, &squad, 0);
            expected += income - wages;
            open_transfer_window(&mut club, &pop, &squad, DivLevel::Third, 0);
            assert_eq!(
                club.budget, expected,
                "budget must track the running income-minus-wages sum exactly, no floor at 0"
            );
        }
        assert!(
            club.budget < 0,
            "an elite squad at a minnow club must overspend into negative budget"
        );
    }

    fn push_test_player(pop: &mut Population, ovr: u8) -> usize {
        let idx = pop.len();
        pop.seed.push(idx as u64);
        pop.club.push(0);
        pop.nation.push(0);
        pop.position.push(0);
        pop.birth_age_weeks.push(28 * 52); // peak plateau -> current_ovr == potential
        pop.potential_ovr.push(ovr);
        pop.intake_week.push(0);
        pop.career_goals.push(0);
        pop.career_apps.push(0);
        pop.career_titles.push(0);
        idx
    }

    #[test]
    fn window_wage_deduction_scales_with_squad_quality_and_size() {
        let mut pop = Population::default();

        // Equal size, one squad uniformly higher-quality.
        let weak_squad: Vec<usize> = (0..10).map(|_| push_test_player(&mut pop, 50)).collect();
        let strong_squad: Vec<usize> = (0..10).map(|_| push_test_player(&mut pop, 85)).collect();
        // Equal quality, different sizes.
        let small_squad: Vec<usize> = (0..5).map(|_| push_test_player(&mut pop, 70)).collect();
        let large_squad: Vec<usize> = (0..15).map(|_| push_test_player(&mut pop, 70)).collect();

        assert!(
            window_wage_deduction(&pop, &strong_squad, 0)
                > window_wage_deduction(&pop, &weak_squad, 0),
            "higher-quality squad of equal size must deduct more"
        );
        assert!(
            window_wage_deduction(&pop, &large_squad, 0)
                > window_wage_deduction(&pop, &small_squad, 0),
            "larger squad of equal quality must deduct more"
        );
    }

    // ── Slice 2 TDD anchors ──────────────────────────────────────────────────────

    #[test]
    fn valuation_favors_current_over_potential() {
        let low_current = market_valuation(60, 90, 25);
        let high_current = market_valuation(80, 90, 25);
        assert!(
            high_current > low_current,
            "equal potential_ovr, higher current_ovr must value higher"
        );
    }

    #[test]
    fn valuation_underprices_young_high_ceiling_players_relative_to_ovr_sum() {
        // The task doc's own worked example (§2.2): a peaked 30yo veteran vs. a rising
        // 19yo prospect with a much higher OVR sum but a lower valuation.
        let veteran = market_valuation(85, 85, 30);
        let prospect = market_valuation(60, 90, 19);

        assert_eq!(
            veteran, 4_696,
            "veteran valuation should match the worked example"
        );
        assert_eq!(
            prospect, 2_214,
            "prospect valuation should match the worked example"
        );
        assert!(
            prospect < veteran,
            "young high-ceiling prospect must value below the peaked veteran despite a \
             higher current_ovr + potential_ovr sum ({} vs {})",
            60 + 90,
            85 + 85
        );
    }

    #[test]
    fn age_value_pct_peaks_in_mid_twenties() {
        let samples = [15, 20, 21, 25, 29, 30, 33, 34, 45];
        let pcts: Vec<i64> = samples.iter().map(|&a| age_value_pct(a)).collect();

        // Non-decreasing up to the 21..=29 peak plateau.
        let peak_start = samples.iter().position(|&a| a == 21).unwrap();
        for w in pcts[..=peak_start].windows(2) {
            assert!(
                w[0] <= w[1],
                "age_value_pct must not decrease before the peak: {pcts:?}"
            );
        }
        // Non-increasing from the peak plateau onward.
        let peak_end = samples.iter().position(|&a| a == 29).unwrap();
        for w in pcts[peak_end..].windows(2) {
            assert!(
                w[0] >= w[1],
                "age_value_pct must not increase after the peak: {pcts:?}"
            );
        }
    }
}
