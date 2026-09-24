//! Regression test (PA2 M4 follow-up): population display names must be
//! diverse. The opposition-substitution smoke exposed every NPC as
//! "Rafael Novak" — `name_from_seed` fed raw `player_seed`s into xorshift,
//! whose first draws on the 16-entry name pools sampled only the low bits
//! that `player_seed` leaves constant per world.

use std::collections::HashSet;

#[test]
fn population_names_are_diverse() {
    let world = goat_world::world::WorldGenesis::generate(12345);
    let pop = goat_world::population::genesis(12345, &world);
    let names: HashSet<String> = (0..500usize)
        .map(|i| goat_world::history::name_from_seed(pop.seed[i]))
        .collect();
    assert!(
        names.len() > 200,
        "population names collapsed: {} distinct in 500 players",
        names.len()
    );
}

#[test]
fn name_from_seed_is_stable() {
    // The function is the display-name source of truth across reloads — same
    // seed in, same name out, forever.
    for seed in [0u64, 1, 42, 12345, u64::MAX] {
        assert_eq!(
            goat_world::history::name_from_seed(seed),
            goat_world::history::name_from_seed(seed)
        );
    }
}
