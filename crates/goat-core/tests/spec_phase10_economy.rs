//! Phase 10 SPEC — economy core (TASK-10B.1, bible §8.8 / §2.4).
//!
//! Money is a real resource: wages/bonuses in, lifestyle upkeep + dev costs out, a
//! compounding business, and bankruptcy when you run deep into the red. Development
//! investment buys a growth edge that is ALWAYS ceiling-capped (never past potential),
//! and is neutral at level 0 so the no-spend path is byte-identical to existing goldens.

use goat_core::attrs::{AttrId, NUM_ATTRS};
use goat_core::generation::{CreationChoices, Position};
use goat_core::state::{reduce, Intent, WorldState};
use goat_core::week::{Intensity, Routine};
use goat_rng::GoatRng;

fn player(seed: u64) -> WorldState {
    let choices = CreationChoices {
        name: "Econ".into(),
        position: Position::Forward,
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
        focus_attrs: vec![AttrId::Finishing, AttrId::AttPositioning],
        intensity: Intensity::High,
    };
    reduce(s, Intent::SetRoutine { routine }, &mut GoatRng::new(0))
}

/// Golden: a fixed wealthy career's cashflow over 6 seasons is exactly reproducible.
#[test]
fn golden_cashflow_over_a_career() {
    let mut s = player(1);
    s.pc_wage_annual = 5_000; // a star's wage
    s.pc_savings = 10_000; // signing-bonus nest egg
    s = reduce(
        s,
        Intent::InvestInBusiness { amount: 2_000 },
        &mut GoatRng::new(0),
    );
    let mut rng = GoatRng::new(42);
    for _ in 0..6 {
        s = reduce(
            s,
            Intent::SettleSeasonEconomy {
                season_bonus: 1_000,
            },
            &mut rng,
        );
    }
    // Frozen from first green run.
    assert_eq!(s.pc_savings, 43_520i64, "savings drifted");
    assert_eq!(s.pc_business_value, 3_127i64, "business value drifted");
    assert!(!s.pc_bankrupt);
}

/// A low earner living large goes bankrupt — the floor is reachable.
#[test]
fn bankruptcy_is_reachable() {
    let mut s = player(2);
    s.pc_wage_annual = 20; // tiny wage
    s = reduce(
        s,
        Intent::SetLifestyle { lifestyle: 2 },
        &mut GoatRng::new(0),
    ); // Flashy upkeep
    let mut rng = GoatRng::new(7);
    let mut went_bankrupt = false;
    for _ in 0..20 {
        s = reduce(s, Intent::SettleSeasonEconomy { season_bonus: 0 }, &mut rng);
        if s.pc_bankrupt {
            went_bankrupt = true;
            break;
        }
    }
    assert!(went_bankrupt, "a flashy pauper must eventually go bankrupt");
    assert_eq!(s.pc_business_value, 0, "bankruptcy wipes the business");
}

/// Dev investment speeds growth but NEVER pushes a current attr past its potential (§2.4).
#[test]
fn dev_investment_respects_talent_ceiling() {
    let grow = |level: u8| {
        let mut s = player(5);
        s = reduce(s, Intent::SetDevInvestment { level }, &mut GoatRng::new(0));
        let pc = s.pc_player_id.unwrap();
        let mut rng = GoatRng::new(9);
        for _ in 0..120 {
            s = reduce(s, Intent::AdvanceWeek, &mut rng);
            for a in 0..NUM_ATTRS {
                assert!(
                    s.players.get_current(pc, a) <= s.players.get_potential(pc, a),
                    "dev level {level}: attr {a} exceeded potential"
                );
            }
        }
        let v = s.players.snapshot(pc);
        (0..NUM_ATTRS).map(|a| v.current[a].to_int()).sum::<i32>()
    };
    // Early in a career, a full performance team (level 3) develops faster than no spend.
    assert!(grow(3) >= grow(0), "dev investment should not slow growth");
}

/// Level 0 is exactly neutral — identical attrs to a player who never touched the economy.
#[test]
fn dev_level_zero_is_neutral() {
    let attrs = |set_level: bool| {
        let mut s = player(3);
        if set_level {
            s = reduce(
                s,
                Intent::SetDevInvestment { level: 0 },
                &mut GoatRng::new(0),
            );
        }
        let pc = s.pc_player_id.unwrap();
        let mut rng = GoatRng::new(11);
        for _ in 0..30 {
            s = reduce(s, Intent::AdvanceWeek, &mut rng);
        }
        let v = s.players.snapshot(pc);
        (0..NUM_ATTRS).map(|a| v.current[a]).collect::<Vec<_>>()
    };
    assert_eq!(attrs(true), attrs(false), "dev level 0 must be a no-op");
}
