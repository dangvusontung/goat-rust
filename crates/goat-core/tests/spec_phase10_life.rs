//! Phase 10 SPEC — relationships, scandals, media flashpoints (TASK-10B.3, bible §8.6/§8.7).
//!
//! A few exception-only relationship threads (the deeper web is parked, §11). A thread
//! rupturing triggers a scandal that hits Character rep + marketability. Media flashpoints
//! are a real trade-off: contrite rebuilds Character at a Sporting cost; defiant the
//! reverse. These are explicit intents (renderer drives them by exception) — they never
//! touch the week/growth loop, so existing goldens are untouched.

use goat_core::generation::{CreationChoices, Position};
use goat_core::state::{reduce, Intent, WorldState};
use goat_rng::GoatRng;

fn player() -> WorldState {
    let choices = CreationChoices {
        name: "Life".into(),
        position: Position::Forward,
        nationality: "Brazilian",
        club: "Riverside Town",
    };
    let mut s = WorldState::new();
    s = reduce(
        s,
        Intent::CreatePlayer { seed: 1, choices },
        &mut GoatRng::new(0),
    );
    s
}

#[test]
fn rupture_triggers_a_scandal() {
    let mut s = player();
    let char_before = s.pc_character_rep;
    let mkt_before = s.pc_marketability;
    // Partner thread starts at 70; a big strain drops it past the rupture threshold (25).
    s = reduce(
        s,
        Intent::ApplyLifeEvent {
            thread: 0,
            delta: -60,
        },
        &mut GoatRng::new(0),
    );
    assert!(s.pc_relationships[0] < 25, "thread should have ruptured");
    assert!(
        s.pc_character_rep < char_before,
        "scandal should dent Character rep"
    );
    assert!(
        s.pc_marketability < mkt_before,
        "scandal should dent marketability"
    );
}

#[test]
fn scandal_fires_once_not_every_dip() {
    let mut s = player();
    // First dip past threshold → scandal.
    s = reduce(
        s,
        Intent::ApplyLifeEvent {
            thread: 1,
            delta: -60,
        },
        &mut GoatRng::new(0),
    );
    let after_first = s.pc_character_rep;
    // A further strain while already ruptured must NOT re-trigger the scandal hit.
    s = reduce(
        s,
        Intent::ApplyLifeEvent {
            thread: 1,
            delta: -5,
        },
        &mut GoatRng::new(0),
    );
    assert_eq!(
        s.pc_character_rep, after_first,
        "scandal must fire once, on rupture"
    );
}

#[test]
fn media_response_is_a_real_tradeoff() {
    let base = player();
    let mut contrite = base.clone();
    contrite = reduce(
        contrite,
        Intent::RespondToMedia { contrite: true },
        &mut GoatRng::new(0),
    );
    let mut defiant = base.clone();
    defiant = reduce(
        defiant,
        Intent::RespondToMedia { contrite: false },
        &mut GoatRng::new(0),
    );

    // Contrite: Character up, Sporting down. Defiant: the reverse. You cannot win both.
    assert!(contrite.pc_character_rep > base.pc_character_rep);
    assert!(contrite.pc_sporting_rep < base.pc_sporting_rep);
    assert!(defiant.pc_sporting_rep > base.pc_sporting_rep);
    assert!(defiant.pc_character_rep < base.pc_character_rep);
}

#[test]
fn relationships_and_reps_stay_in_range() {
    let mut s = player();
    let mut rng = GoatRng::new(3);
    for i in 0..50 {
        let thread = (i % 3) as u8;
        let delta = if i % 2 == 0 { -40 } else { 30 };
        s = reduce(s, Intent::ApplyLifeEvent { thread, delta }, &mut rng);
        s = reduce(
            s,
            Intent::RespondToMedia {
                contrite: i % 2 == 0,
            },
            &mut rng,
        );
        for r in s.pc_relationships {
            assert!((0..=100).contains(&r), "relationship out of range: {r}");
        }
        assert!((0..=100).contains(&s.pc_character_rep));
        assert!((0..=100).contains(&s.pc_sporting_rep));
    }
}
