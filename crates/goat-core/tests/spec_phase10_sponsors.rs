//! Phase 10 SPEC — sponsors + marketability (TASK-10B.2, bible §8.5).
//!
//! Marketability gates sponsor tiers (local→national→global). Sponsors pay, but their
//! obligations cost energy (the same resource training needs), and cashing in beyond your
//! sporting merit dents reputation. Tier 0 (no sponsor) is neutral — byte-identical to
//! the existing goldens.

use goat_core::generation::CreationChoices;
use goat_core::positions::PrimaryPosition;
use goat_core::state::{reduce, Intent, WorldState};
use goat_core::week::{Intensity, Routine};
use goat_rng::GoatRng;

fn player(seed: u64) -> WorldState {
    let choices = CreationChoices {
        name: "Spon".into(),
        primary_position: PrimaryPosition::ST,
        nationality: "Brazilian",
        club: "Riverside Town",
    };
    let mut s = WorldState::new();
    s = reduce(
        s,
        Intent::CreatePlayer { seed, choices },
        &mut GoatRng::new(0),
    );
    let routine = Routine {
        focus_attrs: vec![goat_core::attrs::AttrId::Finishing],
        intensity: Intensity::High,
    };
    reduce(s, Intent::SetRoutine { routine }, &mut GoatRng::new(0))
}

#[test]
fn marketability_gates_sponsor_tier() {
    let mut s = player(1);
    s = reduce(
        s,
        Intent::SetMarketability { value: 30 },
        &mut GoatRng::new(0),
    );
    // Global (tier 3) needs marketability >= 75 — should be refused.
    s = reduce(s, Intent::SignSponsor { tier: 3 }, &mut GoatRng::new(0));
    assert_eq!(
        s.pc_sponsor_tier, 0,
        "global deal must be refused at low marketability"
    );
    // Local (tier 1, needs >=25) should land.
    s = reduce(s, Intent::SignSponsor { tier: 1 }, &mut GoatRng::new(0));
    assert_eq!(
        s.pc_sponsor_tier, 1,
        "local deal should land at marketability 30"
    );
}

#[test]
fn sponsor_income_flows_into_savings() {
    let mut s = player(2);
    s.pc_wage_annual = 0; // isolate sponsor income
    s = reduce(
        s,
        Intent::SetMarketability { value: 100 },
        &mut GoatRng::new(0),
    );
    s = reduce(s, Intent::SignSponsor { tier: 2 }, &mut GoatRng::new(0)); // national: 600/yr
    let before = s.pc_savings;
    s = reduce(
        s,
        Intent::SettleSeasonEconomy { season_bonus: 0 },
        &mut GoatRng::new(5),
    );
    // National income (600) minus Balanced upkeep (80) = +520.
    assert_eq!(
        s.pc_savings - before,
        520,
        "sponsor income should hit savings"
    );
}

#[test]
fn over_commercialising_dents_reputation() {
    let mut s = player(3);
    s = reduce(
        s,
        Intent::SetMarketability { value: 100 },
        &mut GoatRng::new(0),
    );
    s.pc_sporting_rep = 20; // a marketable but unproven player
    let rep_before = s.pc_sporting_rep;
    // Global deal: marketable enough to sign, but sporting merit (20) < threshold (75).
    s = reduce(s, Intent::SignSponsor { tier: 3 }, &mut GoatRng::new(0));
    assert_eq!(s.pc_sponsor_tier, 3);
    assert!(
        s.pc_sporting_rep < rep_before,
        "over-commercialising should dent Sporting reputation"
    );
}

#[test]
fn sponsor_obligations_drain_energy_but_none_is_neutral() {
    let energy_after = |tier: u8| {
        let mut s = player(4);
        s = reduce(
            s,
            Intent::SetMarketability { value: 100 },
            &mut GoatRng::new(0),
        );
        s = reduce(s, Intent::SignSponsor { tier }, &mut GoatRng::new(0));
        let pc = s.pc_player_id.unwrap();
        let mut rng = GoatRng::new(9);
        for _ in 0..10 {
            s = reduce(s, Intent::AdvanceWeek, &mut rng);
            s.pc_week_training_done = false; // week boundary — harness has no round loop
        }
        s.players.get_energy(pc)
    };
    // A global sponsor's obligations leave the player more tired than no sponsor.
    assert!(
        energy_after(3) < energy_after(0),
        "sponsor obligations must drain energy"
    );
}
