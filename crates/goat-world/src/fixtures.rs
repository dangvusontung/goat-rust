//! Deterministic fixture generation — never stored, always recomputed.
//!
//! Algorithm: circle method for round-robin, home/away seeded per season.
//! `generate_fixtures(season, clubs)` → `ROUNDS_PER_SEASON` rounds,
//! each round containing `CLUBS_PER_DIV / 2` matches.

use crate::world::{ClubId, CLUBS_PER_DIV};
use goat_rng::{GoatRng, RngSource};

pub const ROUNDS_PER_SEASON: usize = (CLUBS_PER_DIV - 1) * 2; // 38

/// A single fixture slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fixture {
    pub home: ClubId,
    pub away: ClubId,
    /// Match round within the season (0-indexed, 0..ROUNDS_PER_SEASON).
    pub round: usize,
}

/// All fixtures for one league, one season. `league_id` seeds the home/away RNG (as
/// `div_idx` did before); `clubs` is that league's *current* membership for this season
/// (post promotion/relegation, if applicable) — the caller resolves membership, this
/// function only builds the round-robin off whatever list it's given.
/// Returns a flat list sorted by round.
pub fn generate_fixtures(world_seed: u64, season: u32, league_id: usize, clubs: &[ClubId]) -> Vec<Fixture> {
    let n = clubs.len();
    debug_assert!(n.is_multiple_of(2));

    // Deterministic home/away flip per club-pair per season.
    let mut rng = GoatRng::new(world_seed ^ (season as u64 * 0xdeadbeef) ^ (league_id as u64));

    // Build the two half-seasons (first: rounds 0..n-2; second: rounds n-1..2n-3).
    // Circle method: fix clubs[0], rotate clubs[1..n].
    // rotation[i] holds an index into `clubs`; clubs[0] is never in the rotation.
    // The rotation sequence is identical in both halves, so local_round X always
    // pairs the same two clubs in half 0 and half 1 — that pairing's home/away
    // swap is decided once (during half 0) and mirrored (flipped) in half 1,
    // rather than redrawn, so every pair meets exactly once home and once away.
    let rotation: Vec<usize> = (1..n).collect(); // clubs[1]..clubs[n-1]

    let mut fixtures: Vec<Fixture> = Vec::with_capacity(n * (n - 1));
    let mut swaps: Vec<Vec<bool>> = Vec::with_capacity(n - 1);

    for half in 0..2usize {
        let mut rot = rotation.clone();
        for local_round in 0..(n - 1) {
            let round = half * (n - 1) + local_round;
            if half == 0 {
                swaps.push(Vec::with_capacity(n / 2));
            }

            // Fixed team is clubs[0]; rotating partner is clubs[rot[n-2]] (last slot).
            let fixed_team = clubs[0];
            let partner = clubs[rot[n - 2]];
            let swap0 = if half == 0 {
                let s = rng.next_range_u64(0, 1) == 1;
                swaps[local_round].push(s);
                s
            } else {
                !swaps[local_round][0]
            };
            let (home0, away0) = if swap0 {
                (partner, fixed_team)
            } else {
                (fixed_team, partner)
            };
            fixtures.push(Fixture {
                home: home0,
                away: away0,
                round,
            });

            // Remaining n/2 - 1 pairs from opposing ends of the rotation.
            for i in 0..n / 2 - 1 {
                let team_a = clubs[rot[i]];
                let team_b = clubs[rot[n - 3 - i]];
                let swap = if half == 0 {
                    let s = rng.next_range_u64(0, 1) == 1;
                    swaps[local_round].push(s);
                    s
                } else {
                    !swaps[local_round][i + 1]
                };
                let (home, away) = if swap { (team_b, team_a) } else { (team_a, team_b) };
                fixtures.push(Fixture { home, away, round });
            }

            // Rotate left by 1.
            rot.rotate_left(1);
        }
    }

    fixtures.sort_by_key(|f| f.round);
    fixtures
}

/// Returns all fixtures for a specific club in a given season/league.
pub fn fixtures_for_club(
    world_seed: u64,
    season: u32,
    league_id: usize,
    clubs: &[ClubId],
    club_id: ClubId,
) -> Vec<Fixture> {
    generate_fixtures(world_seed, season, league_id, clubs)
        .into_iter()
        .filter(|f| f.home == club_id || f.away == club_id)
        .collect()
}

/// Find the fixture for a specific round involving a specific club.
pub fn fixture_for_round(
    world_seed: u64,
    season: u32,
    league_id: usize,
    clubs: &[ClubId],
    club_id: ClubId,
    round: usize,
) -> Option<Fixture> {
    generate_fixtures(world_seed, season, league_id, clubs)
        .into_iter()
        .find(|f| f.round == round && (f.home == club_id || f.away == club_id))
}

/// All fixtures in a specific round (all `CLUBS_PER_DIV/2` matches).
pub fn round_fixtures(
    world_seed: u64,
    season: u32,
    league_id: usize,
    clubs: &[ClubId],
    round: usize,
) -> Vec<Fixture> {
    generate_fixtures(world_seed, season, league_id, clubs)
        .into_iter()
        .filter(|f| f.round == round)
        .collect()
}
