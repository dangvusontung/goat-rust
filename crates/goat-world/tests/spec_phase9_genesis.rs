//! Phase 9 SPEC — genesis is a stable, deterministic universe (Slice 9A.1).
//!
//! The fingerprint golden is the spine of Phase 9 determinism: a fixed `world_seed` must
//! produce bit-for-bit the same population on every run and platform. Values frozen from
//! the first green run — NEVER edit to "fix" a failing test; a break means genesis logic
//! (or the SoA layout) changed.

use goat_world::batch_tick::batch_tick_season;
use goat_world::history::backfill_history;
use goat_world::population::{genesis, POP_SIZE};
use goat_world::rival::{crystallise_rival, RivalVerdict};

#[test]
fn genesis_fingerprint_is_stable() {
    // (world_seed, expected fingerprint) — frozen from first green run.
    let golden: [(u64, u64); 3] = [
        (1, 0xed9c_5444_dde1_4f13),
        (7, 0x6d56_a00b_a1d6_f25a),
        (42, 0x2bce_efe7_611f_087d),
    ];
    for (seed, expected) in golden {
        assert_eq!(
            genesis(seed).fingerprint(),
            expected,
            "genesis({seed}) fingerprint drifted — determinism break"
        );
    }
}

#[test]
fn genesis_headcount_is_fixed() {
    assert_eq!(genesis(99).len(), POP_SIZE);
}

/// Batch-ticking the outer world is deterministic: a fixed seed + season sequence yields
/// a stable career fingerprint. Frozen from first green run.
#[test]
fn batch_tick_world_fingerprint_is_stable() {
    let run = |seed: u64| {
        let mut pop = genesis(seed);
        for season in 1..=5u32 {
            batch_tick_season(&mut pop, seed, season, season * 52);
        }
        pop.career_fingerprint()
    };
    assert_eq!(run(7), run(7), "batch-tick must be deterministic");
    // Re-frozen after the fixtures.rs home/away mirroring fix (see fixture_mirror_check.rs):
    // home/away for each pairing is now decided once and mirrored for the return leg
    // instead of independently re-rolled, so simulated results (and this fingerprint)
    // legitimately changed.
    assert_eq!(run(7), 0x0e0e_dac6_9e84_5141, "career fingerprint drifted");
}

/// The backfilled pre-history is a stable, derivable canon for a fixed seed. Frozen.
#[test]
fn history_fingerprint_is_stable() {
    assert_eq!(
        backfill_history(7, 30).fingerprint(),
        0xe0c9_3dbd_e1c4_720e,
        "history canon fingerprint drifted"
    );
}

/// Rival crystallisation is deterministic and the weak-era branch is real: the pattern of
/// who gets a rival vs who reigns alone is stable across a seed sweep. Frozen as a bitmask
/// (bit i set = seed i produced a rival for a fixed mid-tier PC).
#[test]
fn rival_verdict_pattern_is_stable() {
    let verdict = |seed: u64| -> bool {
        let mut pop = genesis(seed);
        for s in 1..=14u32 {
            batch_tick_season(&mut pop, seed, s, s * 52);
        }
        matches!(
            crystallise_rival(&pop, 16 * 52, 200, 5),
            RivalVerdict::Rival { .. }
        )
    };
    let mut mask = 0u32;
    for seed in 0..24u64 {
        if verdict(seed) {
            mask |= 1 << seed;
        }
    }
    // Both outcomes must occur (not all-rivals, not all-weak-era).
    assert!(
        mask != 0 && mask != (1 << 24) - 1,
        "rivalry has no variance"
    );
    // Re-frozen after the fixtures.rs home/away mirroring fix — see comment above.
    assert_eq!(mask, 0x008b_748c, "rival verdict pattern drifted");
}
