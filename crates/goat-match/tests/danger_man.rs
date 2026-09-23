//! Danger-man duels (Tùng-locked): the strongest man in the matchup pool —
//! but only above an absolute bar (mean attr ≥ 70 OR real form ≥ 60), else
//! the match has NO danger man. Counter is per-match, feeds nothing but the
//! recap line and a light pc_form nudge live-side; it consumes no RNG, so
//! outcomes stay byte-identical.

use goat_core::{
    generation::{generate_player, CreationChoices, Position},
    roles::RoleId,
    tactical::TacticalProfile,
};
use goat_fixed::Fixed;
use goat_match::discipline::RefPersonality;
use goat_match::sim::{auto_play_match, danger_form_delta, BeatLibrary, MatchSetup};
use goat_match::squad::SquadSheet;
use goat_rng::GoatRng;
use goat_traits::PlayerTraits;

const BEATS_JSON: &str = include_str!("../../../beats.json");

fn lib() -> BeatLibrary {
    BeatLibrary::load(BEATS_JSON).expect("beats.json must be valid")
}

fn profile(att: i32, mid: i32, def: i32) -> TacticalProfile {
    TacticalProfile {
        attack: att as u8,
        midfield: mid as u8,
        defense: def as u8,
        pressing: 25,
        possession: 25,
        counter: 25,
        wing_play: 25,
    }
}

/// PC centre-back on a mid side against an opponent whose squad stub strength
/// is `opp_str` — strong opponents field a danger man, weak ones don't.
fn setup_cb(opp_str: u8) -> MatchSetup {
    let c = CreationChoices {
        name: "Test".into(),
        position: Position::Defender,
        nationality: "Brazilian",
        club: "Riverside Town".into(),
    };
    let pl = generate_player(999, &c);
    MatchSetup {
        player_role: RoleId::CentreBack,
        player_attrs: pl.current,
        player_familiarity: pl.familiarity,
        own_profile: profile(55, 50, 45),
        opp_profile: profile(70, 65, 65),
        opp_name: "Test FC",
        form: Fixed::from_int(50),
        player_aggression: 50,
        ref_personality: RefPersonality::Balanced,
        dirty_rep: 50,
        player_traits: PlayerTraits::default(),
        staff_mods: goat_core::staff::StaffMods::NEUTRAL,
        own_squad: SquadSheet::stub(60, 0xCAFE, (4, 3, 3)),
        opp_squad: SquadSheet::stub(opp_str, 0xBEEF, (4, 3, 3)),
        sub_context: None,
    }
}

#[test]
fn danger_man_threshold_quality_form_or_nothing() {
    // Elite pool (mean ~75): the strongest man clears the OVR bar.
    let elite = SquadSheet::stub(75, 7, (4, 3, 3));
    let pool: Vec<usize> = (0..elite.players.len()).collect();
    assert!(elite.danger_man_in(&pool).is_some());

    // Poor pool (mean ~45): best-of-a-bad-bunch is NOT a danger man.
    let poor = SquadSheet::stub(45, 7, (4, 3, 3));
    let poor_pool: Vec<usize> = (0..poor.players.len()).collect();
    assert_eq!(poor.danger_man_in(&poor_pool), None);

    // Same poor pool but everyone is red-hot: the strongest man clears the
    // form bar instead.
    let mut hot = SquadSheet::stub(45, 7, (4, 3, 3));
    for p in hot.players.iter_mut() {
        p.form = 65;
    }
    let hot_pool: Vec<usize> = (0..hot.players.len()).collect();
    assert!(hot.danger_man_in(&hot_pool).is_some());
}

#[test]
fn strong_opponents_produce_duels_weak_ones_none() {
    let lib = lib();
    let mut strong_with_duels = 0u32;
    for seed in 0..30u64 {
        let r = auto_play_match(&lib, setup_cb(88), &mut GoatRng::new(seed));
        let duels = r.danger_duels_won as u32 + r.danger_duels_lost as u32;
        if duels > 0 {
            strong_with_duels += 1;
            assert!(
                r.danger_man_name.is_some(),
                "a faced danger man must be named"
            );
        }
        // Determinism: identical setup+seed → identical duel accounting.
        let r2 = auto_play_match(&lib, setup_cb(88), &mut GoatRng::new(seed));
        assert_eq!(
            (r.danger_duels_won, r.danger_duels_lost),
            (r2.danger_duels_won, r2.danger_duels_lost)
        );
    }
    assert!(
        strong_with_duels >= 20,
        "a str-88 side should field a danger man most matches: {strong_with_duels}/30"
    );

    for seed in 0..30u64 {
        let r = auto_play_match(&lib, setup_cb(45), &mut GoatRng::new(seed));
        assert_eq!(
            (r.danger_duels_won, r.danger_duels_lost),
            (0, 0),
            "a str-45 stub (mean ~45, form 50) has no danger man (seed {seed})"
        );
    }
}

#[test]
fn form_delta_is_light_and_symmetric() {
    assert_eq!(danger_form_delta(0, 0), 0);
    assert_eq!(danger_form_delta(2, 1), 2);
    assert_eq!(danger_form_delta(1, 2), -2);
    assert_eq!(danger_form_delta(10, 0), 6, "capped at +6");
    assert_eq!(danger_form_delta(0, 10), -6, "capped at -6");
}
