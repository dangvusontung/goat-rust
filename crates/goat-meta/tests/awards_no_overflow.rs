//! Regression test for the season-end awards hash overflow (awards.rs:51).
//!
//! `ai_competitor_score`'s RNG seed mix used `season as u64 * 0x9e3779b97f4a7c15`,
//! which overflows `u64` starting at season 2 (0x9e3779b97f4a7c15 > u64::MAX / 2).
//! `cargo test` runs the dev profile, which has `overflow-checks = true` by default,
//! so this panicked on any career that reached a second season — exactly what a real
//! playtest or dev/interactive session does. This test drives many seasons/seeds
//! through the public award functions (the only entry points to the hash) so it
//! would have caught the bug before it shipped.

use goat_meta::{compute_golden_boot, compute_player_of_year};

#[test]
fn award_hashing_survives_a_full_career_without_overflow() {
    // A full career is ~60+ seasons (see docs/PLAYTEST-FULLCAREER-2026-07-22.md,
    // age 16->77). Cover well past that, across several world seeds and
    // candidate indices (0..16, matching the two award pools).
    for world_seed in [0u64, 1, 42, 12345, u64::MAX / 3, u64::MAX] {
        for season in 1u32..=80 {
            // The assertions matter less than the fact that these calls complete
            // without panicking under overflow-checked (dev-profile) arithmetic.
            let poty = compute_player_of_year("PC", 70, 12, season, world_seed);
            assert_eq!(poty.pc_score, 70);

            let boot = compute_golden_boot("PC", 15, season, world_seed);
            assert_eq!(boot.pc_score, 15);
        }
    }
}
