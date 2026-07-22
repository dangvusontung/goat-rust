//! Phase 9 SPEC — genesis is a stable, deterministic universe (Slice 9A.1).
//!
//! Determinism is the spine of Phase 9: a fixed `world_seed` must produce bit-for-bit the
//! same population on every run and platform. Post world-genesis scale-up (Design round 2:
//! a generated ~1,200-club world replaced the old fixed `CLUBS`/`DIV_CLUBS` consts, so
//! `genesis`/`backfill_history`/`batch_tick_season` all now take a `&WorldGenesis`), these
//! assert determinism/variance invariants rather than frozen hex fingerprints — following the
//! existing "assert invariants, not frozen exact values, for new behavior" convention
//! (`TASK-DESIGN-round1-pantheon-saves.md`) — since the old frozen values were computed
//! against an algorithm that no longer exists.

use goat_world::batch_tick::batch_tick_season;
use goat_world::history::backfill_history;
use goat_world::population::{genesis, SQUAD_SIZE};
use goat_world::rival::{crystallise_rival, RivalVerdict};
use goat_world::world::{WorldGenesis, NUM_CLUBS};

#[test]
fn genesis_fingerprint_is_stable() {
    for seed in [1u64, 7, 42] {
        let world = WorldGenesis::generate(seed);
        assert_eq!(
            genesis(seed, &world).fingerprint(),
            genesis(seed, &world).fingerprint(),
            "genesis({seed}) fingerprint must be deterministic"
        );
    }
    let world1 = WorldGenesis::generate(1);
    let world7 = WorldGenesis::generate(7);
    assert_ne!(
        genesis(1, &world1).fingerprint(),
        genesis(7, &world7).fingerprint(),
        "different seeds must produce different populations"
    );
}

#[test]
fn genesis_headcount_is_fixed() {
    let world = WorldGenesis::generate(99);
    assert_eq!(genesis(99, &world).len(), NUM_CLUBS * SQUAD_SIZE);
}

/// Batch-ticking the outer world is deterministic: a fixed seed + season sequence yields
/// a stable career fingerprint.
#[test]
fn batch_tick_world_fingerprint_is_stable() {
    let run = |seed: u64| {
        let world = WorldGenesis::generate(seed);
        let league_clubs = world.static_league_clubs();
        let mut pop = genesis(seed, &world);
        for season in 1..=5u32 {
            batch_tick_season(&mut pop, &world, &league_clubs, seed, season, season * 52);
        }
        pop.career_fingerprint()
    };
    assert_eq!(run(7), run(7), "batch-tick must be deterministic");
    assert_ne!(
        run(7),
        run(11),
        "different seeds must produce different career fingerprints"
    );
}

/// The backfilled pre-history is a stable, derivable canon for a fixed seed.
#[test]
fn history_fingerprint_is_stable() {
    let world = WorldGenesis::generate(7);
    assert_eq!(
        backfill_history(7, 30, &world).fingerprint(),
        backfill_history(7, 30, &world).fingerprint(),
        "history canon fingerprint must be deterministic"
    );
    let world2 = WorldGenesis::generate(11);
    assert_ne!(
        backfill_history(7, 30, &world).fingerprint(),
        backfill_history(11, 30, &world2).fingerprint(),
        "different seeds must produce different history canons"
    );
}

/// Rival crystallisation is deterministic and the weak-era branch is real: both outcomes
/// (a rival crystallises, or nobody keeps pace) must occur across a seed sweep.
#[test]
fn rival_verdict_pattern_is_stable() {
    let verdict = |seed: u64| -> bool {
        let world = WorldGenesis::generate(seed);
        let league_clubs = world.static_league_clubs();
        let mut pop = genesis(seed, &world);
        for s in 1..=14u32 {
            batch_tick_season(&mut pop, &world, &league_clubs, seed, s, s * 52);
        }
        matches!(
            crystallise_rival(&pop, 16 * 52, 300, 8),
            RivalVerdict::Rival { .. }
        )
    };
    let mut saw_rival = false;
    let mut saw_weak_era = false;
    for seed in 0..24u64 {
        if verdict(seed) {
            saw_rival = true;
        } else {
            saw_weak_era = true;
        }
    }
    assert!(
        saw_rival && saw_weak_era,
        "rivalry has no variance across seeds"
    );
}
