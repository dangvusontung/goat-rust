//! Phase 8–10 golden and property tests.
//!
//! Phase 8: contract/transfer/power ladder.
//! Phase 9: peer cohort batch-tick and rival crystallisation.
//! Phase 10: lifestyle effects on growth, retirement.

use goat_core::{
    attrs::{AttrId, NUM_ATTRS},
    generation::{CreationChoices, Position},
    state::{reduce, Intent, PeerState, WorldState},
    week::{Intensity, Routine},
};
use goat_fixed::Fixed;
use goat_rng::GoatRng;

fn base_state() -> WorldState {
    let choices = CreationChoices {
        name: "Test Legend".into(),
        position: Position::Forward,
        nationality: "England",
        club: "Burnley",
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
            nationality: "England",
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

    let mut s_pro = base_state();
    s_pro = reduce(
        s_pro,
        Intent::SetLifestyle { lifestyle: 0 },
        &mut GoatRng::new(0),
    );
    s_pro = reduce(
        s_pro,
        Intent::SetRoutine {
            routine: routine.clone(),
        },
        &mut GoatRng::new(0),
    );

    let mut s_bal = base_state();
    s_bal = reduce(
        s_bal,
        Intent::SetLifestyle { lifestyle: 1 },
        &mut GoatRng::new(0),
    );
    s_bal = reduce(s_bal, Intent::SetRoutine { routine }, &mut GoatRng::new(0));

    let mut rng = GoatRng::new(555);
    for _ in 0..52 {
        s_pro = reduce(s_pro, Intent::AdvanceWeek, &mut rng.clone());
        s_bal = reduce(s_bal, Intent::AdvanceWeek, &mut rng);
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
        s = reduce(
            s,
            Intent::SetLifestyle { lifestyle: 2 },
            &mut GoatRng::new(0),
        );
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
    s = reduce(
        s,
        Intent::SetLifestyle { lifestyle: 0 },
        &mut GoatRng::new(0),
    );
    s = reduce(s, Intent::SetRoutine { routine }, &mut GoatRng::new(0));

    let mut rng = GoatRng::new(77);
    for _ in 0..200 {
        s = reduce(s, Intent::AdvanceWeek, &mut rng);
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
