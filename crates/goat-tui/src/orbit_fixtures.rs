//! Bridges `goat-world`'s round-robin league fixtures into `goat_calendar::Fixture`
//! values for `Intent::StartSeason`.
//!
//! Lives here (not in `goat-core`) because `goat-core` stays headless with respect to
//! `goat-world` (see that crate's own doc comment: "This crate depends on `goat-core`
//! but is NOT depended on by it. The TUI links `goat-world` and `goat-core` together").
//! `goat-tui` already depends on both, plus `goat-calendar`, so this is the natural home
//! for the conversion `goat-core`'s `calendar_loop` module docs point to.
//!
//! Included via `#[path]` in both `main.rs` and `career_sim.rs` (two standalone `[[bin]]`
//! targets in this crate, no shared `lib.rs`).

use goat_calendar::{Fixture, FixtureImportance};
use goat_core::calendar_loop::LEAGUE_COMPETITION_ID;
use goat_rng::{GoatRng, RngSource};
use goat_world::world::ClubId;
use goat_world::{fixtures_for_club, round_to_week, week_day_offset, week_to_rounds};

/// Build the PC's league orbit fixtures for one season: every fixture from
/// `fixtures_for_club`, converted into a `goat_calendar::Fixture` with a deterministic
/// id (bible's `FixtureId` doc: "hash(seed, competition, season, round, slot)") and a
/// season-relative `scheduled_day` from `goat_world::calendar`'s week grid.
pub fn build_season_orbit_fixtures(
    world_seed: u64,
    season: u32,
    league_id: usize,
    div_clubs: &[ClubId],
    pc_club_id: ClubId,
) -> Vec<Fixture> {
    fixtures_for_club(world_seed, season, league_id, div_clubs, pc_club_id)
        .into_iter()
        .map(|f| {
            let week = round_to_week(f.round);
            let slot = f.round - week_to_rounds(week).start;
            let day = week_day_offset(week, slot);
            let id = fixture_id(world_seed, LEAGUE_COMPETITION_ID, season, f.round, slot);
            Fixture {
                id,
                competition_id: LEAGUE_COMPETITION_ID,
                scheduled_day: day,
                original_day: day,
                is_orbit: true,
                importance: FixtureImportance::League,
                leg_for_id: None,
            }
        })
        .collect()
}

fn fixture_id(world_seed: u64, competition_id: u32, season: u32, round: usize, slot: usize) -> u64 {
    let mixed = world_seed
        ^ ((competition_id as u64) << 48)
        ^ ((season as u64) << 24)
        ^ ((round as u64) << 8)
        ^ (slot as u64);
    GoatRng::new(mixed).next_u64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_deterministic_and_distinct_per_round() {
        let a = fixture_id(1, 1, 1, 3, 0);
        let b = fixture_id(1, 1, 1, 3, 0);
        let c = fixture_id(1, 1, 1, 4, 0);
        assert_eq!(a, b, "same inputs must produce the same id");
        assert_ne!(a, c, "different rounds must produce different ids");
    }
}
