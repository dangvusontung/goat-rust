//! Phase 9 SPEC — genesis is a stable, deterministic universe (Slice 9A.1).
//!
//! The fingerprint golden is the spine of Phase 9 determinism: a fixed `world_seed` must
//! produce bit-for-bit the same population on every run and platform. Values frozen from
//! the first green run — NEVER edit to "fix" a failing test; a break means genesis logic
//! (or the SoA layout) changed.
//!
//! Re-frozen once for the World Scale-Up (Phase A): the world grew from 2 nations /
//! 64 clubs to 50 nations / 2,544 clubs, so every fingerprint below legitimately changed.
//! The invariants (determinism, both rival outcomes occurring) are unchanged.

use goat_world::batch_tick::batch_tick_season;
use goat_world::history::backfill_history;
use goat_world::population::{genesis, POP_SIZE};
use goat_world::rival::{crystallise_rival, RivalVerdict};

#[test]
fn genesis_fingerprint_is_stable() {
    // (world_seed, expected fingerprint) — frozen from first green run.
    let golden: [(u64, u64); 3] = [
        (1, 0xa7b1_c8fa_e756_caec),
        (7, 0x1181_f2ef_3bac_6136),
        (42, 0x9b7c_5862_6d20_42e4),
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
    assert_eq!(run(7), 0x3361_ae92_eacf_dd9e, "career fingerprint drifted");
}

/// The backfilled pre-history is a stable, derivable canon for a fixed seed. Frozen.
#[test]
fn history_fingerprint_is_stable() {
    assert_eq!(
        backfill_history(7, 30).fingerprint(),
        0xa5ae_a09f_159d_5872,
        "history canon fingerprint drifted"
    );
}

/// Rival crystallisation is deterministic and the weak-era branch is real: the pattern of
/// who gets a rival vs who reigns alone is stable across a seed sweep. Frozen as a bitmask
/// (bit i set = seed i produced a rival for a fixed mid-tier PC).
///
/// The PC record bar was recalibrated for the 50-nation world: with 159 divisions there
/// are 159 league titles per season (not 4), so the old bar (200 goals / 5 titles) made
/// every seed produce a rival — no variance. (250 goals / 6 titles) restores both
/// outcomes.
#[test]
fn rival_verdict_pattern_is_stable() {
    let verdict = |seed: u64| -> bool {
        let mut pop = genesis(seed);
        for s in 1..=14u32 {
            batch_tick_season(&mut pop, seed, s, s * 52);
        }
        matches!(
            crystallise_rival(&pop, 16 * 52, 250, 6),
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
    assert_eq!(mask, 0x000c_9004, "rival verdict pattern drifted");
}
