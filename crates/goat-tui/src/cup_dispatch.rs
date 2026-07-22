//! Minimal live dispatcher for the PC's domestic-cup run (Design round 4, Slice 5's
//! playable-gate proof: "PC picks up a domestic-cup suspension → next league fixture
//! still selects the PC normally → next domestic-cup fixture correctly benches them").
//!
//! Deliberately simplified from `goat_world::domestic_cup`'s full nation-wide bracket
//! engine (used elsewhere for the "generated but consistent" season-end cup result):
//! this is scoped to just the PC's own path through the bracket, which is all the
//! suspension-scoping proof needs. Each round's opponent is drawn from its own salted
//! stream rather than the shared nation-wide pool draw, so a live-diverged PC result
//! (a match played through the full beat/attribute engine, not `sim_team_match`) never
//! has to reconcile against the rest of the bracket's simulated progression. A
//! production-grade dispatcher would want the full reusable `draw_round`/`tier_clubs`
//! composition those primitives were built for; this is the smallest correct slice of
//! it needed to prove suspension scoping composes in real play.
//!
//! Calendar placement (which league-round index each cup round lands on) is a
//! placeholder cadence, not a real fixture-congestion pass — same TASK-TUNE status as
//! `goat-calendar::tuning`'s constants (§5.3 flags this same gap for the full
//! `Fixture`/`CalendarEngine`-routed wiring a later round should build).

use goat_rng::{GoatRng, RngSource};
use goat_world::domestic_cup::CUP_ROUNDS;
use goat_world::world::{ClubId, DivLevel, NationId, WorldGenesis};

/// Placeholder cup-round cadence relative to league rounds (TASK-TUNE — a real
/// fixture-congestion feel pass belongs to a later round; see the doc's §5.3 note).
pub const CUP_ROUND_SPACING: usize = 4;

/// League-round index the PC's tier's first cup tie is scheduled on (mirrors
/// `domestic_cup::entry_tier`, inverted).
pub fn entry_league_round(tier: DivLevel) -> usize {
    let entry_round = match tier {
        DivLevel::Top => 2,
        DivLevel::Second => 1,
        DivLevel::Third => 0,
    };
    entry_round * CUP_ROUND_SPACING
}

/// One domestic-cup fixture due for the PC's club.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CupFixtureDue {
    pub cup_round: usize,
    pub opponent: ClubId,
    pub pc_is_home: bool,
    pub is_final: bool,
}

/// If the PC's club (still alive in the cup, per the caller's session-tracked
/// `pc_alive`) has a tie due at `league_round`, draw this round's opponent.
#[allow(clippy::too_many_arguments)]
pub fn cup_fixture_due(
    world: &WorldGenesis,
    world_seed: u64,
    season: u32,
    nation: NationId,
    pc_club: ClubId,
    pc_tier: DivLevel,
    league_round: usize,
    pc_alive: bool,
) -> Option<CupFixtureDue> {
    if !pc_alive {
        return None;
    }
    let entry = entry_league_round(pc_tier);
    if league_round < entry {
        return None;
    }
    let delta = league_round - entry;
    if !delta.is_multiple_of(CUP_ROUND_SPACING) {
        return None;
    }
    let cup_round = delta / CUP_ROUND_SPACING;
    if cup_round >= CUP_ROUNDS {
        return None;
    }

    let seed = world_seed
        ^ (season as u64).rotate_left(19)
        ^ (nation as u64).rotate_left(23)
        ^ (cup_round as u64).rotate_left(41)
        ^ 0x0000_0000_0000_0C10;
    let mut rng = GoatRng::new(seed);

    let candidates: Vec<ClubId> = world
        .clubs_for_nation(nation)
        .map(|c| c.id)
        .filter(|&id| id != pc_club)
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let idx = rng.next_range_u32(0, candidates.len() as u32 - 1) as usize;
    let opponent = candidates[idx];
    let pc_is_home = rng.next_range_u32(0, 1) == 0;

    Some(CupFixtureDue {
        cup_round,
        opponent,
        pc_is_home,
        is_final: cup_round == CUP_ROUNDS - 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_round_scales_with_tier() {
        assert_eq!(entry_league_round(DivLevel::Third), 0);
        assert_eq!(entry_league_round(DivLevel::Second), CUP_ROUND_SPACING);
        assert_eq!(entry_league_round(DivLevel::Top), CUP_ROUND_SPACING * 2);
    }

    #[test]
    fn no_fixture_before_entry_round_or_when_eliminated() {
        let world = WorldGenesis::generate(7);
        let nation = world.clubs[0].nation;
        assert!(cup_fixture_due(&world, 7, 1, nation, 0, DivLevel::Top, 0, true).is_none());
        assert!(cup_fixture_due(&world, 7, 1, nation, 0, DivLevel::Third, 0, false).is_none());
    }

    #[test]
    fn fixture_due_is_deterministic_and_never_the_pc_themselves() {
        let world = WorldGenesis::generate(7);
        let nation = world.clubs[0].nation;
        let pc_club = 0;
        let a = cup_fixture_due(&world, 7, 1, nation, pc_club, DivLevel::Third, 0, true).unwrap();
        let b = cup_fixture_due(&world, 7, 1, nation, pc_club, DivLevel::Third, 0, true).unwrap();
        assert_eq!(a, b);
        assert_ne!(a.opponent, pc_club);
        assert_eq!(a.cup_round, 0);
    }

    #[test]
    fn spacing_gates_which_league_rounds_have_a_cup_fixture() {
        let world = WorldGenesis::generate(7);
        let nation = world.clubs[0].nation;
        let pc_club = 0;
        // Third-tier entry is round 0; the next cup round is CUP_ROUND_SPACING later.
        assert!(cup_fixture_due(&world, 7, 1, nation, pc_club, DivLevel::Third, 1, true).is_none());
        assert!(cup_fixture_due(
            &world,
            7,
            1,
            nation,
            pc_club,
            DivLevel::Third,
            CUP_ROUND_SPACING,
            true
        )
        .is_some());
    }
}
