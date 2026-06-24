//! Phase 10 SPEC — retirement trigger (TASK-10B.4, bible §8.6).
//!
//! Nobody plays past the hard age; past the soft age a player whose contract has run out
//! (offers drying up) retires. The player may always choose to retire earlier.

use goat_core::generation::{CreationChoices, Position};
use goat_core::state::{reduce, should_retire, Intent, WorldState};
use goat_rng::GoatRng;

fn player_at_age(age_years: u32, contract_left: u32) -> WorldState {
    let choices = CreationChoices {
        name: "Vet".into(),
        position: Position::Forward,
        nationality: "Brazilian",
        club: "Riverside Town",
    };
    let mut s = WorldState::new();
    s = reduce(
        s,
        Intent::CreatePlayer { seed: 1, choices },
        &mut GoatRng::new(0),
    );
    let pc = s.pc_player_id.unwrap();
    s.players.set_age_weeks(pc, age_years * 52);
    s.pc_contract_seasons_left = contract_left;
    s
}

#[test]
fn young_contracted_player_does_not_retire() {
    assert!(!should_retire(&player_at_age(24, 3)));
    assert!(!should_retire(&player_at_age(33, 0))); // below soft age, even out of contract
}

#[test]
fn out_of_contract_veteran_retires() {
    // Past the soft age (34) with no contract → offers drying up → retire.
    assert!(should_retire(&player_at_age(35, 0)));
    // Still under contract at 35 → plays on.
    assert!(!should_retire(&player_at_age(35, 2)));
}

#[test]
fn nobody_plays_past_the_hard_age() {
    assert!(should_retire(&player_at_age(40, 5))); // even mid-contract
}
