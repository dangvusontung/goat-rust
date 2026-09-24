//! Regression test: every pair of clubs must meet exactly once at home and
//! once away over a season's double round-robin (see fixtures.rs).

use goat_world::{generate_fixtures, CLUBS_PER_DIV};
use std::collections::HashMap;

#[test]
fn every_pair_mirrors_home_away_across_seeds() {
    let clubs: Vec<usize> = (0..CLUBS_PER_DIV).collect();
    let mut total_bad = 0;
    let mut total_pairs_checked = 0;
    for world_seed in 0..50u64 {
        for season in 1..=3u32 {
            let fixtures = generate_fixtures(world_seed, season, 0, &clubs);
            let mut pair_sides: HashMap<(usize, usize), Vec<(usize, usize)>> = HashMap::new();
            for f in &fixtures {
                let key = if f.home < f.away {
                    (f.home, f.away)
                } else {
                    (f.away, f.home)
                };
                pair_sides.entry(key).or_default().push((f.home, f.away));
            }
            for sides in pair_sides.values() {
                assert_eq!(sides.len(), 2, "every pair should meet exactly twice");
                total_pairs_checked += 1;
                let (h0, a0) = sides[0];
                let (h1, a1) = sides[1];
                if !(h0 == a1 && a0 == h1) {
                    total_bad += 1;
                }
            }
        }
    }
    assert_eq!(
        total_bad, 0,
        "{total_bad}/{total_pairs_checked} pairs did not alternate home/away"
    );
}

/// `round_to_week` always returns the week that *contains* the given round,
/// so it can never resolve to a break week (a week with zero rounds) — break
/// weeks exist in the season template but are never the "current" week for
/// any in-progress round. `goat-bridge::advance_weeks`'s pending-match guard
/// relies on this: it's why that guard caps to one week instead of fully
/// blocking (a full block would make fast-forward permanently inert, since
/// the current week is never break-free during an active season).
#[test]
fn current_round_never_resolves_to_a_break_week() {
    use goat_world::{is_break_week, round_to_week, ROUNDS_PER_SEASON, SEASON_CALENDAR_WEEKS};
    for r in 0..ROUNDS_PER_SEASON {
        assert!(
            !is_break_week(round_to_week(r)),
            "round {r} resolved to a break week — pending-match detection assumption broken"
        );
    }
    // Sanity: break weeks do exist somewhere in the template (just unreachable as "current").
    assert!(
        (0..SEASON_CALENDAR_WEEKS).any(is_break_week),
        "expected at least one break week in the season template"
    );
}
