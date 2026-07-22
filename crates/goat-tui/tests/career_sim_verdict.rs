//! Regression test for the `career-sim` CAREER VERDICT "Lifestyle:" line.
//!
//! It used to print the CLI `--lifestyle` seed's label verbatim
//! (`career_sim.rs:1394`, pre-fix) instead of the actual final
//! `state.pc_lifestyle` after simulation. Lifestyle is an emergent,
//! weekly-nudged readout (bible §8.5/§8.6) — at a fixed `--intensity`, the
//! weekly nudge pulls the real tier to the same value regardless of the CLI
//! seed within the first few seasons, so all three `--lifestyle` seeds must
//! report the *same* real tier in the verdict at fixed `--intensity high`,
//! not three different (stale) labels.

use std::process::Command;

fn run_career_sim(lifestyle: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_career-sim"))
        .args([
            "--seed",
            "42",
            "--intensity",
            "high",
            "--lifestyle",
            lifestyle,
        ])
        .output()
        .expect("failed to run career-sim binary");
    String::from_utf8(output.stdout).expect("career-sim stdout should be utf8")
}

/// Extract just the lifestyle tier word ("Professional"/"Balanced"/"Flashy") from
/// the verdict's "Lifestyle      : <tier>  Intensity: ..." line — injured-weeks
/// legitimately varies by lifestyle (it affects injury risk), so the test must not
/// compare the whole line, only the tier label the fix is about.
fn lifestyle_tier_word(stdout: &str) -> &str {
    let line = stdout
        .lines()
        .find(|l| l.contains("Lifestyle"))
        .unwrap_or_else(|| panic!("expected a Lifestyle line in the CAREER VERDICT box:\n{stdout}"));
    line.split(':')
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .unwrap_or_else(|| panic!("could not parse a lifestyle tier word out of: {line:?}"))
}

#[test]
fn verdict_lifestyle_label_reflects_actual_drifted_tier_not_cli_seed() {
    let professional = run_career_sim("professional");
    let flashy = run_career_sim("flashy");

    let professional_tier = lifestyle_tier_word(&professional);
    let flashy_tier = lifestyle_tier_word(&flashy);

    assert_eq!(
        professional_tier, flashy_tier,
        "the Lifestyle tier must report the same real (intensity-driven) value \
         regardless of the --lifestyle seed at fixed --intensity high — a mismatch \
         means the verdict is printing the stale CLI seed's label again \
         (professional run tier: {professional_tier:?}, flashy run tier: {flashy_tier:?})"
    );
    assert_eq!(
        professional_tier, "Professional",
        "expected the drifted tier to read Professional under --intensity high, got {professional_tier:?}"
    );
}
