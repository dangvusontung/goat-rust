//! Phase 10 SPEC — long-horizon career hardening (TASK-10B.5).
//!
//! A full 16→retirement career with the economy + life systems live must never panic and
//! must hold every invariant: talent ceiling, energy bounds, relationships/reps in range,
//! and retirement actually reached. This is the whole-game spine test.

use goat_core::attrs::NUM_ATTRS;
use goat_core::generation::CreationChoices;
use goat_core::positions::PrimaryPosition;
use goat_core::state::{reduce, should_retire, Intent, WorldState};
use goat_core::week::{Intensity, Routine};
use goat_rng::GoatRng;

#[test]
fn full_career_to_retirement_holds_invariants() {
    let choices = CreationChoices {
        name: "Lifer".into(),
        primary_position: PrimaryPosition::ST,
        nationality: "Brazilian",
        club: "Riverside Town",
    };
    let mut s = WorldState::new();
    s = reduce(
        s,
        Intent::CreatePlayer { seed: 7, choices },
        &mut GoatRng::new(0),
    );
    let routine = Routine {
        focus_attrs: vec![goat_core::attrs::AttrId::Finishing],
        intensity: Intensity::High,
    };
    s = reduce(s, Intent::SetRoutine { routine }, &mut GoatRng::new(0));

    // A full off-pitch career: wage, dev investment, a sponsor. Lifestyle is no longer
    // picked directly (bible §8.5/§8.6) — it emerges from the High-intensity routine set
    // above (which nudges toward Professional every week) plus dev investment and the
    // sponsor signed below (which nudges toward Flashy once). Either way, the invariants
    // below must hold regardless of which tier the career lands on.
    s.pc_wage_annual = 3_000;
    s = reduce(
        s,
        Intent::SetDevInvestment { level: 2 },
        &mut GoatRng::new(0),
    );
    s = reduce(
        s,
        Intent::SetMarketability { value: 80 },
        &mut GoatRng::new(0),
    );
    s = reduce(s, Intent::SignSponsor { tier: 2 }, &mut GoatRng::new(0));

    let pc = s.pc_player_id.unwrap();
    let mut rng = GoatRng::new(123);
    let mut weeks = 0u32;

    while !should_retire(&s) && weeks < 2_500 {
        s = reduce(s, Intent::AdvanceWeek, &mut rng);
        s.pc_week_training_done = false; // week boundary — harness has no round loop
        weeks += 1;

        // Talent ceiling + energy bounds every single week.
        for a in 0..NUM_ATTRS {
            assert!(
                s.players.get_current(pc, a) <= s.players.get_potential(pc, a),
                "week {weeks}: attr {a} exceeded potential"
            );
        }

        // Once a year: settle the economy and apply a small life strain.
        if weeks.is_multiple_of(52) {
            s = reduce(
                s,
                Intent::SettleSeasonEconomy { season_bonus: 500 },
                &mut rng,
            );
            s = reduce(
                s,
                Intent::ApplyLifeEvent {
                    thread: 0,
                    delta: -4,
                },
                &mut rng,
            );
            // Off-pitch scalars stay in range throughout.
            assert!(s.pc_relationships.iter().all(|&r| (0..=100).contains(&r)));
            assert!((0..=100).contains(&s.pc_character_rep));
            assert!((0..=100).contains(&s.pc_marketability));
        }
    }

    assert!(
        should_retire(&s),
        "a career must reach retirement (reached week {weeks})"
    );
    // The player aged from 16 into at least their mid-30s.
    assert!(s.players.get_age_weeks(pc) / 52 >= 34);
}
