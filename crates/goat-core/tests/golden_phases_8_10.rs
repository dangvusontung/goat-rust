//! Phase 8–10 golden and property tests.
//!
//! Phase 8: contract/transfer/power ladder.
//! Phase 9: peer cohort batch-tick and rival crystallisation.
//! Phase 10: lifestyle effects on growth, retirement.

use goat_core::{
    attrs::{AttrId, NUM_ATTRS},
    generation::CreationChoices,
    positions::PrimaryPosition,
    state::{lifestyle_tier_from_score, reduce, Intent, PeerState, WorldState},
    week::{Intensity, Routine},
};
use goat_fixed::Fixed;
use goat_rng::GoatRng;

fn base_state() -> WorldState {
    let choices = CreationChoices {
        name: "Test Legend".into(),
        primary_position: PrimaryPosition::ST,
        nationality: "England".to_string(),
        club: "Burnley".to_string(),
    };
    let mut s = WorldState::new();
    s = reduce(
        s,
        Intent::CreatePlayer { seed: 42, choices },
        &mut GoatRng::new(0),
    );
    s
}

// ── Phase 8: Contract + Power Ladder ─────────────────────────────────────────

#[test]
fn agitate_advances_power_ladder() {
    let mut s = base_state();
    assert_eq!(s.pc_power_ladder, 0);
    s = reduce(s, Intent::AgitateForTransfer, &mut GoatRng::new(0));
    assert_eq!(s.pc_power_ladder, 1, "one agitation advances ladder");
    s = reduce(s, Intent::AgitateForTransfer, &mut GoatRng::new(0));
    assert_eq!(s.pc_power_ladder, 2);
    s = reduce(s, Intent::AgitateForTransfer, &mut GoatRng::new(0));
    assert_eq!(s.pc_power_ladder, 3);
    s = reduce(s, Intent::AgitateForTransfer, &mut GoatRng::new(0));
    assert_eq!(s.pc_power_ladder, 3, "capped at 3");
}

#[test]
fn agitate_burns_character_rep() {
    let mut s = base_state();
    let rep_before = s.pc_discipline_rep;
    s = reduce(s, Intent::AgitateForTransfer, &mut GoatRng::new(0));
    assert!(
        s.pc_discipline_rep > rep_before,
        "agitation should raise dirty rep (tightens officiating): {rep_before} → {}",
        s.pc_discipline_rep
    );
}

#[test]
fn accept_contract_resets_power_ladder() {
    let mut s = base_state();
    s = reduce(s, Intent::AgitateForTransfer, &mut GoatRng::new(0));
    assert_eq!(s.pc_power_ladder, 1);
    s = reduce(
        s,
        Intent::AcceptContract {
            new_wage: 80,
            new_length: 3,
            new_club_idx: 0,
        },
        &mut GoatRng::new(0),
    );
    assert_eq!(s.pc_power_ladder, 0, "contract resets power ladder");
    assert_eq!(s.pc_contract_seasons_left, 3);
    assert_eq!(s.pc_wage_annual, 80);
}

#[test]
fn transfer_changes_club() {
    let mut s = base_state();
    let old_savings = s.pc_savings;
    s = reduce(
        s,
        Intent::ExecuteTransfer {
            to_club_idx: 5,
            to_div_idx: 0,
            new_wage: 120,
            new_length: 3,
            new_club_name: "Arsenal",
            facilities_mult: Fixed::from_int(1),
            fee_bonus: 200,
        },
        &mut GoatRng::new(0),
    );
    assert_eq!(s.pc_club_idx, 5);
    assert_eq!(
        s.pc_savings,
        old_savings + 200,
        "fee bonus added to savings"
    );
    assert_eq!(s.pc_power_ladder, 0, "power ladder reset on transfer");
}

#[test]
fn collect_wage_increases_savings_and_counts_down_contract() {
    let mut s = base_state();
    s = reduce(
        s,
        Intent::AcceptContract {
            new_wage: 50,
            new_length: 4,
            new_club_idx: 0,
        },
        &mut GoatRng::new(0),
    );
    let savings_before = s.pc_savings;
    let contract_before = s.pc_contract_seasons_left;
    s = reduce(s, Intent::CollectWage, &mut GoatRng::new(0));
    assert_eq!(s.pc_savings, savings_before + 50, "wage added to savings");
    assert_eq!(
        s.pc_contract_seasons_left,
        contract_before - 1,
        "contract ticks down"
    );
}

// ── Phase 9: Peers + Rival ────────────────────────────────────────────────────

fn make_peers() -> Vec<PeerState> {
    (0..8)
        .map(|i| PeerState {
            seed: i as u64 * 12345,
            name: format!("Peer {i}"),
            nationality: "England".to_string(),
            career_goals: 0,
            career_matches: 0,
            avg_output: 0,
            titles: 0,
        })
        .collect()
}

#[test]
fn batch_tick_advances_peer_stats() {
    let mut s = base_state();
    s = reduce(
        s,
        Intent::InitPeers {
            peers: make_peers(),
        },
        &mut GoatRng::new(0),
    );
    assert_eq!(s.pc_peers.len(), 8);
    assert_eq!(s.pc_peers[0].career_matches, 0);

    s = reduce(
        s,
        Intent::BatchTickPeers { season: 1 },
        &mut GoatRng::new(0),
    );
    assert!(
        s.pc_peers[0].career_matches > 0,
        "peers should have matches after one batch-tick"
    );
}

#[test]
fn declare_rival_records_peer_and_season() {
    let mut s = base_state();
    s = reduce(
        s,
        Intent::InitPeers {
            peers: make_peers(),
        },
        &mut GoatRng::new(0),
    );
    assert!(s.pc_rival_idx.is_none());

    s = reduce(
        s,
        Intent::DeclareRival {
            peer_idx: 3,
            season: 7,
        },
        &mut GoatRng::new(0),
    );
    assert_eq!(s.pc_rival_idx, Some(3), "rival idx set");
    assert_eq!(
        s.pc_rival_declared_season,
        Some(7),
        "declaration season recorded"
    );
}

#[test]
fn peers_are_deterministic_across_runs() {
    let mut s1 = base_state();
    let mut s2 = base_state();
    s1 = reduce(
        s1,
        Intent::InitPeers {
            peers: make_peers(),
        },
        &mut GoatRng::new(0),
    );
    s2 = reduce(
        s2,
        Intent::InitPeers {
            peers: make_peers(),
        },
        &mut GoatRng::new(0),
    );

    for season in 1..=5u32 {
        s1 = reduce(s1, Intent::BatchTickPeers { season }, &mut GoatRng::new(0));
        s2 = reduce(s2, Intent::BatchTickPeers { season }, &mut GoatRng::new(0));
    }

    for i in 0..8 {
        assert_eq!(
            s1.pc_peers[i].career_matches, s2.pc_peers[i].career_matches,
            "peer {i} career_matches deterministic"
        );
        assert_eq!(
            s1.pc_peers[i].avg_output, s2.pc_peers[i].avg_output,
            "peer {i} avg_output deterministic"
        );
    }
}

// ── Phase 10: Lifestyle + Retirement ─────────────────────────────────────────

#[test]
fn professional_lifestyle_boosts_growth() {
    let routine = Routine {
        focus_attrs: vec![AttrId::Finishing, AttrId::CloseControl],
        intensity: Intensity::Medium,
    };

    // Lifestyle is now emergent (bible §8.5/§8.6) — seed the underlying score directly
    // to force each career onto its tier before the comparison, rather than picking it
    // via an intent.
    let mut s_pro = base_state();
    s_pro.pc_lifestyle_score = Fixed::raw(-1_000);
    s_pro.pc_lifestyle = lifestyle_tier_from_score(s_pro.pc_lifestyle_score);
    s_pro = reduce(
        s_pro,
        Intent::SetRoutine {
            routine: routine.clone(),
        },
        &mut GoatRng::new(0),
    );

    let mut s_bal = base_state();
    s_bal.pc_lifestyle_score = Fixed::ZERO;
    s_bal.pc_lifestyle = lifestyle_tier_from_score(s_bal.pc_lifestyle_score);
    s_bal = reduce(s_bal, Intent::SetRoutine { routine }, &mut GoatRng::new(0));

    let mut rng = GoatRng::new(555);
    for _ in 0..52 {
        s_pro = reduce(s_pro, Intent::AdvanceWeek, &mut rng.clone());
        s_pro.pc_week_training_done = false; // week boundary — harness has no round loop
        s_bal = reduce(s_bal, Intent::AdvanceWeek, &mut rng);
        s_bal.pc_week_training_done = false; // week boundary — harness has no round loop
    }

    let fin_pro = s_pro.players.get_current(0, AttrId::Finishing as usize);
    let fin_bal = s_bal.players.get_current(0, AttrId::Finishing as usize);
    assert!(
        fin_pro >= fin_bal,
        "professional lifestyle should grow at least as fast as balanced: {fin_pro:?} vs {fin_bal:?}"
    );
}

#[test]
fn flashy_lifestyle_has_lower_mult() {
    let s_flash = {
        let mut s = base_state();
        s.pc_lifestyle_score = Fixed::raw(1_000);
        s.pc_lifestyle = lifestyle_tier_from_score(s.pc_lifestyle_score);
        s
    };
    assert_eq!(s_flash.pc_lifestyle, 2);
}

#[test]
fn retire_sets_retired_flag() {
    let mut s = base_state();
    assert!(!s.pc_retired);
    s = reduce(s, Intent::Retire, &mut GoatRng::new(0));
    assert!(s.pc_retired, "retire intent sets pc_retired");
}

#[test]
fn attrs_stay_in_bounds_with_lifestyle_professional() {
    let routine = Routine {
        focus_attrs: vec![AttrId::Finishing],
        intensity: Intensity::High,
    };
    let mut s = base_state();
    s.pc_lifestyle_score = Fixed::raw(-1_000);
    s.pc_lifestyle = lifestyle_tier_from_score(s.pc_lifestyle_score);
    s = reduce(s, Intent::SetRoutine { routine }, &mut GoatRng::new(0));

    let mut rng = GoatRng::new(77);
    for _ in 0..200 {
        s = reduce(s, Intent::AdvanceWeek, &mut rng);
        s.pc_week_training_done = false; // week boundary — harness has no round loop
        for a in 0..NUM_ATTRS {
            let cur = s.players.get_current(0, a);
            let pot = s.players.get_potential(0, a);
            assert!(cur <= pot, "professional lifestyle: attr {a} cur > pot");
            assert!(
                cur >= Fixed::MIN_ATTR,
                "professional lifestyle: attr {a} below min"
            );
        }
    }
}

// ── Design round 1: Pantheon raw-signal evidence wiring ─────────────────────────

fn apply_season_end_legacy(
    s: WorldState,
    season_standout_matches: u32,
    season_transfer_requests: u32,
) -> WorldState {
    reduce(
        s,
        Intent::ApplySeasonEndLegacy {
            season_goals: 0,
            season_matches: 0,
            season_output_sum: 0,
            won_title: false,
            player_of_year: false,
            finish_position: 10,
            decisive_moments: 0,
            new_sporting_rep: 50,
            new_club_fan_rep: 50,
            season_standout_matches,
            season_transfer_requests,
        },
        &mut GoatRng::new(0),
    )
}

#[test]
fn season_end_legacy_folds_standout_and_transfer_counters() {
    let mut s = base_state();
    assert_eq!(s.pc_career_standout_matches, 0);
    assert_eq!(s.pc_career_transfer_requests, 0);

    s = apply_season_end_legacy(s, 5, 2);
    assert_eq!(
        s.pc_career_standout_matches, 5,
        "season 1 standout matches fold in"
    );
    assert_eq!(
        s.pc_career_transfer_requests, 2,
        "season 1 transfer requests fold in"
    );

    s = apply_season_end_legacy(s, 3, 1);
    assert_eq!(
        s.pc_career_standout_matches, 8,
        "season 2 standout matches accumulate on top of season 1"
    );
    assert_eq!(
        s.pc_career_transfer_requests, 3,
        "season 2 transfer requests accumulate on top of season 1"
    );
}

#[test]
fn season_end_legacy_best_ovr_is_a_running_max() {
    let mut s = base_state();
    let pc_id = s.pc_player_id.expect("base_state creates a PC");

    // Boost every attribute to its potential ceiling, then fold in a season-end check.
    for a in 0..NUM_ATTRS {
        let potential = s.players.get_potential(pc_id, a);
        s.players.set_current(pc_id, a, potential);
    }
    s = apply_season_end_legacy(s, 0, 0);
    let peak_after_high = s.pc_career_best_ovr;
    assert!(
        peak_after_high > 0,
        "peak OVR should be recorded from a high-attribute player"
    );

    // Now drop every attribute — a lower current OVR must not decrease the recorded peak.
    for a in 0..NUM_ATTRS {
        s.players.set_current(pc_id, a, Fixed::from_int(5));
    }
    s = apply_season_end_legacy(s, 0, 0);
    assert_eq!(
        s.pc_career_best_ovr, peak_after_high,
        "career_best_ovr is a running max — a lower current OVR must not decrease it"
    );
}

#[test]
fn round_result_counts_standout_matches_at_threshold() {
    use goat_core::tuning::STANDOUT_OUTPUT_THRESHOLD;

    let mut s = base_state();
    assert_eq!(s.pc_season_standout_matches, 0);

    let round = |s: WorldState, pc_output: i32| {
        reduce(
            s,
            Intent::ApplyRoundResult {
                pc_goals: 0,
                pc_output,
                pc_result: 0,
                round_results: Vec::new(),
                rest_weeks: 0,
                week_ends: true,
            },
            &mut GoatRng::new(0),
        )
    };

    s = round(s, STANDOUT_OUTPUT_THRESHOLD - 1);
    assert_eq!(
        s.pc_season_standout_matches, 0,
        "below threshold does not count"
    );

    s = round(s, STANDOUT_OUTPUT_THRESHOLD);
    assert_eq!(
        s.pc_season_standout_matches, 1,
        "at threshold counts as standout"
    );

    s = round(s, 100);
    assert_eq!(
        s.pc_season_standout_matches, 2,
        "above threshold counts as standout"
    );
}

#[test]
fn start_season_resets_standout_and_transfer_request_counters() {
    let mut s = base_state();
    s = reduce(s, Intent::AgitateForTransfer, &mut GoatRng::new(0));
    s = reduce(
        s,
        Intent::ApplyRoundResult {
            pc_goals: 0,
            pc_output: 100,
            pc_result: 0,
            round_results: Vec::new(),
            rest_weeks: 0,
            week_ends: true,
        },
        &mut GoatRng::new(0),
    );
    assert_eq!(s.pc_season_transfer_requests, 1);
    assert_eq!(s.pc_season_standout_matches, 1);

    s = reduce(s, Intent::StartSeason, &mut GoatRng::new(0));
    assert_eq!(
        s.pc_season_transfer_requests, 0,
        "StartSeason resets the live counter"
    );
    assert_eq!(
        s.pc_season_standout_matches, 0,
        "StartSeason resets the live counter"
    );
}
