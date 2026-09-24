//! Transfer target search — weakest-position detection and gem-hunting (Design round 5,
//! Doc A, Slice 3-4). Both lanes are population-scale search functions a later slice
//! (`slice5-transfers`, not yet built) will call once per club per transfer window to
//! decide who to bid for; this module only finds targets, it never spends money or
//! mutates anything.
//!
//! **Coarse `position`, not 14-role `RoleId`, for whole-market search.** `derive::role_rating`
//! needs a background player's full per-attribute `current` vector, which only exists after
//! `Population::promote` lazy-realizes them — the exact per-window cost bible §9's lazy
//! promotion exists to avoid at population scale (~29,000 players). This module's two
//! search lanes instead read only `Population`'s cheap SoA columns (`position`, `current_ovr`,
//! `potential_ovr`, `age_years_at`, `is_retired`) — the same cost profile `batch_tick.rs`'s
//! own whole-population scan already uses.

use crate::economy::market_valuation;
use crate::population::Population;
use crate::world::ClubId;

/// Design's own bound on how deep a target search scans a position's sorted candidate
/// list — a perf/quality tradeoff, not measured against a real `--release` benchmark this
/// round (flagged for Tùng's sign-off in the task doc's "Decisions" list, item 4).
const TARGET_SEARCH_PREFIX: usize = 50;

/// Only unproven-ceiling players are "gems" for Slice 4's scoring — a peaked veteran has no
/// unrealized potential left to buy cheap. Design's own cutoff (task doc "Decisions" item 5).
const GEM_HUNT_MAX_AGE: u32 = 21;

/// Which of the 3 coarse positions has this club's weakest *best* player, by current OVR.
/// Cheap: one pass over the (small, `~18-30`-player) squad, no realization.
fn weakest_position(pop: &Population, squad: &[usize], elapsed_weeks: u32) -> Option<u8> {
    (0u8..3).min_by_key(|&pos| {
        squad
            .iter()
            .filter(|&&i| pop.position[i] == pos && !pop.is_retired(i, elapsed_weeks))
            .map(|&i| pop.current_ovr(i, elapsed_weeks))
            .max()
            .unwrap_or(0) // an empty position group reads as maximally weak — correct: a
                          // club with *no* forward at all should prioritize buying one
    })
}

/// Built once per window (not once per club — 1,200 clubs re-scanning the whole population
/// each would be the same cost mistake `batch_tick.rs`'s own doc-comment on
/// `live_strength`/`live_strength_from_squad` already warns against, round-3 §3.2). Each
/// position's `Vec` is sorted descending by `current_ovr` once; every club's search below is
/// then a bounded prefix scan, not a rescan.
pub fn candidates_by_position(pop: &Population, elapsed_weeks: u32) -> [Vec<usize>; 3] {
    let mut out: [Vec<usize>; 3] = Default::default();
    for i in 0..pop.len() {
        if !pop.is_retired(i, elapsed_weeks) {
            out[pop.position[i] as usize].push(i);
        }
    }
    for list in &mut out {
        list.sort_by_key(|&i| std::cmp::Reverse(pop.current_ovr(i, elapsed_weeks)));
    }
    out
}

/// A club's single weakest-position target for this pass, or `None` if nothing affordable
/// beats what it already has. `lane_cap` is this club's weakest-position spending ceiling
/// this window (a sibling slice's `Club.budget`-derived value).
pub fn weakest_position_target(
    club_id: ClubId,
    pop: &Population,
    squad: &[usize],
    candidates: &[Vec<usize>; 3],
    lane_cap: i64,
    elapsed_weeks: u32,
) -> Option<usize> {
    let pos = weakest_position(pop, squad, elapsed_weeks)?;
    let own_best = squad
        .iter()
        .filter(|&&i| pop.position[i] == pos)
        .map(|&i| pop.current_ovr(i, elapsed_weeks))
        .max()
        .unwrap_or(0);
    // Bounded prefix scan (e.g. top 50 by current_ovr in this position) — a real upgrade
    // (current_ovr > own_best), not already at this club, and its market_valuation fits
    // within this lane's cap.
    candidates[pos as usize]
        .iter()
        .take(TARGET_SEARCH_PREFIX)
        .copied()
        .find(|&i| {
            pop.club[i] as usize != club_id
                && pop.current_ovr(i, elapsed_weeks) > own_best
                && market_valuation(
                    pop.current_ovr(i, elapsed_weeks),
                    pop.potential_ovr[i],
                    pop.age_years_at(i, elapsed_weeks),
                ) <= lane_cap
        })
}

/// The market pays a *little* for scouted upside, not its full eventual value — see
/// `market_valuation`'s doc comment for why this is exploitable. This score deliberately
/// ranks by `potential_ovr - current_ovr`, the exact gap `market_valuation` underprices.
fn gem_hunt_score(current_ovr: u8, potential_ovr: u8, age: u32) -> i64 {
    if age > GEM_HUNT_MAX_AGE {
        return 0;
    }
    (potential_ovr as i64 - current_ovr as i64).max(0)
}

/// Reuses the same `candidates_by_position` lists (3.2) purely as a cheap, already-
/// built "who's out there" index — re-sorted here by `gem_hunt_score` instead of
/// `current_ovr`, since this lane's ranking question is different from the weakest-position
/// lane's. Built once per window, same cost discipline as 3.2.
pub fn gem_targets_by_position(
    pop: &Population,
    candidates: &[Vec<usize>; 3],
    elapsed_weeks: u32,
) -> [Vec<usize>; 3] {
    let mut out = candidates.clone();
    for list in &mut out {
        list.sort_by_key(|&i| {
            std::cmp::Reverse(gem_hunt_score(
                pop.current_ovr(i, elapsed_weeks),
                pop.potential_ovr[i],
                pop.age_years_at(i, elapsed_weeks),
            ))
        });
    }
    out
}

/// Whole-population, position-agnostic search: gem-hunting is proactive, not gap-filling
/// (per Tùng's explicit "not just reactive to squad gaps" requirement). Scans across all 3
/// position lists' top prefixes and picks the single highest-scoring affordable candidate
/// overall — gem-hunting doesn't care which position the gem plays.
pub fn gem_hunt_target(
    club_id: ClubId,
    pop: &Population,
    gem_lists: &[Vec<usize>; 3],
    lane_cap: i64,
    elapsed_weeks: u32,
) -> Option<usize> {
    gem_lists
        .iter()
        .flat_map(|list| list.iter().take(TARGET_SEARCH_PREFIX).copied())
        .filter(|&i| {
            pop.club[i] as usize != club_id
                && gem_hunt_score(
                    pop.current_ovr(i, elapsed_weeks),
                    pop.potential_ovr[i],
                    pop.age_years_at(i, elapsed_weeks),
                ) > 0
                && market_valuation(
                    pop.current_ovr(i, elapsed_weeks),
                    pop.potential_ovr[i],
                    pop.age_years_at(i, elapsed_weeks),
                ) <= lane_cap
        })
        .max_by_key(|&i| {
            gem_hunt_score(
                pop.current_ovr(i, elapsed_weeks),
                pop.potential_ovr[i],
                pop.age_years_at(i, elapsed_weeks),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `age_years` is chosen so `current_ovr == potential_ovr` at the 26..=31 development
    /// peak plateau — the same trick `economy.rs`'s own tests use — so tests can dial in an
    /// exact `current_ovr` via `potential_ovr` alone.
    fn push_peaked_player(pop: &mut Population, club: u16, position: u8, ovr: u8) -> usize {
        push_player(pop, club, position, ovr, 28)
    }

    fn push_player(
        pop: &mut Population,
        club: u16,
        position: u8,
        potential_ovr: u8,
        age_years: u32,
    ) -> usize {
        let idx = pop.len();
        pop.seed.push(idx as u64);
        pop.club.push(club);
        pop.nation.push(0);
        pop.position.push(position);
        pop.birth_age_weeks.push(age_years * 52);
        pop.potential_ovr.push(potential_ovr);
        pop.intake_week.push(0);
        pop.career_goals.push(0);
        pop.career_apps.push(0);
        pop.career_titles.push(0);
        idx
    }

    const DEFENDER: u8 = 0;
    const MIDFIELDER: u8 = 1;
    const FORWARD: u8 = 2;

    // ── Slice 3 TDD anchors ──────────────────────────────────────────────────────

    #[test]
    fn weakest_position_finds_the_real_gap() {
        let mut pop = Population::default();
        let squad: Vec<usize> = vec![
            push_peaked_player(&mut pop, 0, DEFENDER, 70),
            push_peaked_player(&mut pop, 0, DEFENDER, 65),
            push_peaked_player(&mut pop, 0, MIDFIELDER, 72),
            push_peaked_player(&mut pop, 0, MIDFIELDER, 68),
            // no forwards at all
        ];

        assert_eq!(
            weakest_position(&pop, &squad, 0),
            Some(FORWARD),
            "a squad with a strong defense/midfield and zero forwards must flag Forward \
             as the weakest position"
        );
    }

    #[test]
    fn target_search_never_returns_own_players_or_downgrades() {
        let mut pop = Population::default();
        const CLUB: u16 = 0;
        let squad: Vec<usize> = vec![
            push_peaked_player(&mut pop, CLUB, DEFENDER, 80),
            push_peaked_player(&mut pop, CLUB, MIDFIELDER, 80),
            push_peaked_player(&mut pop, CLUB, FORWARD, 60), // own_best forward = 60, weakest
        ];
        // A stronger forward at CLUB itself — must never be offered as a target for CLUB.
        push_peaked_player(&mut pop, CLUB, FORWARD, 95);
        // A weaker forward elsewhere — not an upgrade, must be skipped.
        push_peaked_player(&mut pop, 1, FORWARD, 55);
        // A genuine upgrade elsewhere.
        let upgrade = push_peaked_player(&mut pop, 1, FORWARD, 75);

        let candidates = candidates_by_position(&pop, 0);
        let target = weakest_position_target(CLUB as usize, &pop, &squad, &candidates, i64::MAX, 0);

        assert_eq!(
            target,
            Some(upgrade),
            "must return the genuine external upgrade"
        );
        let target_idx = target.unwrap();
        assert_ne!(
            pop.club[target_idx] as usize, CLUB as usize,
            "must never return a player already at club_id"
        );
        assert!(
            pop.current_ovr(target_idx, 0) > 60,
            "must never return a downgrade relative to own_best"
        );
    }

    #[test]
    fn target_search_respects_lane_cap() {
        let mut pop = Population::default();
        const CLUB: u16 = 0;
        let squad: Vec<usize> = vec![
            push_peaked_player(&mut pop, CLUB, DEFENDER, 80),
            push_peaked_player(&mut pop, CLUB, MIDFIELDER, 80),
            push_peaked_player(&mut pop, CLUB, FORWARD, 50), // weakest position
        ];
        // Top-ranked candidate by current_ovr, but expensive.
        let expensive = push_peaked_player(&mut pop, 1, FORWARD, 90);
        let affordable = push_peaked_player(&mut pop, 1, FORWARD, 60);

        let candidates = candidates_by_position(&pop, 0);
        let cap = market_valuation(60, 60, 28); // exactly the affordable candidate's price
        let target = weakest_position_target(CLUB as usize, &pop, &squad, &candidates, cap, 0);

        assert_ne!(
            target,
            Some(expensive),
            "a target whose market_valuation exceeds lane_cap must never be returned, \
             even when top-ranked"
        );
        assert_eq!(target, Some(affordable));
    }

    #[test]
    fn candidates_by_position_is_deterministic_and_sorted() {
        let mut pop = Population::default();
        push_peaked_player(&mut pop, 0, DEFENDER, 40);
        push_peaked_player(&mut pop, 1, DEFENDER, 80);
        push_peaked_player(&mut pop, 2, DEFENDER, 60);
        push_peaked_player(&mut pop, 0, MIDFIELDER, 55);
        push_peaked_player(&mut pop, 1, FORWARD, 90);

        let a = candidates_by_position(&pop, 0);
        let b = candidates_by_position(&pop, 0);
        assert_eq!(
            a, b,
            "same population twice must produce identical sorted lists"
        );

        for list in &a {
            let ovrs: Vec<u8> = list.iter().map(|&i| pop.current_ovr(i, 0)).collect();
            assert!(
                ovrs.windows(2).all(|w| w[0] >= w[1]),
                "every list must be non-increasing in current_ovr: {ovrs:?}"
            );
        }
    }

    // ── Slice 4 TDD anchors ──────────────────────────────────────────────────────

    #[test]
    fn gem_hunt_score_zero_past_max_age() {
        assert_eq!(gem_hunt_score(40, 90, GEM_HUNT_MAX_AGE + 1), 0);
        assert_eq!(gem_hunt_score(10, 99, 35), 0);
        // Sanity: within the age band, a real gap scores positive.
        assert!(gem_hunt_score(40, 90, GEM_HUNT_MAX_AGE) > 0);
    }

    #[test]
    fn gem_hunt_prefers_outlier_style_prospects() {
        let mut pop = Population::default();
        const CLUB: u16 = 1;
        // Round-3-outlier-style: young, low current_ovr, sky-high potential_ovr.
        let outlier = push_player(&mut pop, CLUB, FORWARD, 95, 17);
        // Ordinary anchor-formula players at the same club: peaked, no unrealized gap.
        push_peaked_player(&mut pop, CLUB, DEFENDER, 70);
        push_peaked_player(&mut pop, CLUB, MIDFIELDER, 65);

        let candidates = candidates_by_position(&pop, 0);
        let gem_lists = gem_targets_by_position(&pop, &candidates, 0);
        let target = gem_hunt_target(0, &pop, &gem_lists, i64::MAX, 0);

        assert_eq!(
            target,
            Some(outlier),
            "the outlier-style young high-ceiling prospect must be the top-ranked gem \
             target ahead of ordinary anchor-formula players"
        );
    }

    #[test]
    fn gem_hunt_ignores_position_gaps() {
        let mut pop = Population::default();
        const CLUB: u16 = 0;
        // Every position already strong for CLUB — no squad weakness for Slice 3 to find.
        push_peaked_player(&mut pop, CLUB, DEFENDER, 90);
        push_peaked_player(&mut pop, CLUB, MIDFIELDER, 90);
        push_peaked_player(&mut pop, CLUB, FORWARD, 90);
        // A young prospect elsewhere with real unrealized potential.
        let gem = push_player(&mut pop, 1, DEFENDER, 95, 18);

        let candidates = candidates_by_position(&pop, 0);
        let gem_lists = gem_targets_by_position(&pop, &candidates, 0);
        let target = gem_hunt_target(CLUB as usize, &pop, &gem_lists, i64::MAX, 0);

        assert_eq!(
            target,
            Some(gem),
            "gem-hunting must return a target even when the club has no squad weakness at all"
        );
    }
}
