//! Deterministic fixture generation — never stored, always recomputed.
//!
//! Algorithm: circle method for round-robin, home/away seeded per season.
//! `generate_fixtures(season, div_idx)` → `ROUNDS_PER_SEASON` rounds,
//! each round containing `CLUBS_PER_DIV / 2` matches.

use crate::world::{ClubId, CLUBS_PER_DIV, DIV_CLUBS};
use goat_rng::{GoatRng, RngSource};

pub const ROUNDS_PER_SEASON: usize = (CLUBS_PER_DIV - 1) * 2; // 30

/// A single fixture slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fixture {
    pub home: ClubId,
    pub away: ClubId,
    /// Match round within the season (0-indexed, 0..ROUNDS_PER_SEASON).
    pub round: usize,
}

/// All fixtures for one division, one season.
/// Returns a flat list sorted by round.
pub fn generate_fixtures(world_seed: u64, season: u32, div_idx: usize) -> Vec<Fixture> {
    let clubs = DIV_CLUBS[div_idx];
    let n = CLUBS_PER_DIV;
    debug_assert!(n.is_multiple_of(2));

    // Deterministic home/away flip per club-pair per season.
    let mut rng = GoatRng::new(world_seed ^ (season as u64 * 0xdeadbeef) ^ (div_idx as u64));

    // Build the two half-seasons (first: rounds 0..n-2; second: rounds n-1..2n-3).
    // Circle method: fix clubs[0], rotate clubs[1..n].
    // rotation[i] holds an index into `clubs`; clubs[0] is never in the rotation.
    let rotation: Vec<usize> = (1..n).collect(); // clubs[1]..clubs[n-1]

    let mut fixtures: Vec<Fixture> = Vec::with_capacity(n * (n - 1));

    for half in 0..2usize {
        let mut rot = rotation.clone();
        for local_round in 0..(n - 1) {
            let round = half * (n - 1) + local_round;
            // Fixed team is clubs[0]; rotating partner is clubs[rot[n-2]] (last slot).
            let fixed_team = clubs[0];
            let partner = clubs[rot[n - 2]];
            // Determine home/away for the fixed pair.
            let (home0, away0) = if rng.next_range_u64(0, 1) == 0 {
                (fixed_team, partner)
            } else {
                (partner, fixed_team)
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
                let (home, away) = if rng.next_range_u64(0, 1) == 0 {
                    (team_a, team_b)
                } else {
                    (team_b, team_a)
                };
                fixtures.push(Fixture { home, away, round });
            }

            // Rotate left by 1.
            rot.rotate_left(1);
        }
    }

    fixtures.sort_by_key(|f| f.round);
    fixtures
}

/// Returns all fixtures for a specific club in a given season/division.
pub fn fixtures_for_club(
    world_seed: u64,
    season: u32,
    div_idx: usize,
    club_id: ClubId,
) -> Vec<Fixture> {
    generate_fixtures(world_seed, season, div_idx)
        .into_iter()
        .filter(|f| f.home == club_id || f.away == club_id)
        .collect()
}

/// Find the fixture for a specific round involving a specific club.
pub fn fixture_for_round(
    world_seed: u64,
    season: u32,
    div_idx: usize,
    club_id: ClubId,
    round: usize,
) -> Option<Fixture> {
    generate_fixtures(world_seed, season, div_idx)
        .into_iter()
        .find(|f| f.round == round && (f.home == club_id || f.away == club_id))
}

/// All fixtures in a specific round (all `CLUBS_PER_DIV/2` matches).
pub fn round_fixtures(world_seed: u64, season: u32, div_idx: usize, round: usize) -> Vec<Fixture> {
    generate_fixtures(world_seed, season, div_idx)
        .into_iter()
        .filter(|f| f.round == round)
        .collect()
}
