//! PA2 M2 — squad-driven matches: individual matchups (A.5) + real-name commentary.

use goat_core::{
    generation::{generate_player, CreationChoices, Position},
    roles::RoleId,
    tactical::TacticalProfile,
};
use goat_fixed::Fixed;
use goat_match::discipline::RefPersonality;
use goat_match::sim::{auto_play_match, BeatLibrary, MatchSetup};
use goat_match::squad::SquadSheet;
use goat_rng::GoatRng;
use goat_traits::PlayerTraits;

const BEATS_JSON: &str = include_str!("../../../beats.json");

fn lib() -> BeatLibrary {
    BeatLibrary::load(BEATS_JSON).expect("beats.json must be valid")
}

fn neutral_profile() -> TacticalProfile {
    TacticalProfile {
        attack: 50,
        midfield: 50,
        defense: 50,
        pressing: 25,
        possession: 25,
        counter: 25,
        wing_play: 25,
    }
}

fn setup_with_opp_strength(opp_attr: i32) -> MatchSetup {
    let c = CreationChoices {
        name: "Test".into(),
        position: Position::Forward,
        nationality: "Brazilian",
        club: "Riverside Town".into(),
    };
    let pl = generate_player(12345, &c);
    let mut opp_squad = SquadSheet::stub(50, 0xBEEF, (4, 3, 3));
    // Identical TEAM profile (50 everywhere) — only the individuals differ, so
    // any output gap comes from the A.5 matchup read, not the line scalars.
    for p in opp_squad.players.iter_mut() {
        for a in p.attrs.iter_mut() {
            *a = Fixed::from_int(opp_attr);
        }
    }
    let mut own_squad = SquadSheet::stub(50, 0xCAFE, (4, 3, 3));
    own_squad.players[9].is_pc = true;
    MatchSetup {
        player_role: RoleId::CompleteForward,
        player_attrs: pl.current,
        player_familiarity: pl.familiarity,
        own_profile: neutral_profile(),
        opp_profile: neutral_profile(),
        opp_name: "Test FC",
        form: Fixed::from_int(50),
        player_aggression: 50,
        ref_personality: RefPersonality::Balanced,
        dirty_rep: 50,
        player_traits: PlayerTraits::default(),
        staff_mods: goat_core::staff::StaffMods::NEUTRAL,
        own_squad,
        opp_squad,
        sub_context: None,
    }
}

/// A.5 locked principle: the specific man on the far side of the contest is a
/// REAL attribute read — a squad of 90-rated individuals is measurably harder
/// to play against than a squad of 30-rated individuals with the SAME team
/// profile.
#[test]
fn strong_matchup_harder_than_weak_partner() {
    let lib = lib();
    let strong = setup_with_opp_strength(90);
    let weak = setup_with_opp_strength(30);

    let mut strong_out = 0i64;
    let mut weak_out = 0i64;
    for seed in 0..40u64 {
        strong_out +=
            auto_play_match(&lib, strong.clone(), &mut GoatRng::new(seed)).player_output as i64;
        weak_out +=
            auto_play_match(&lib, weak.clone(), &mut GoatRng::new(seed)).player_output as i64;
    }
    assert!(
        weak_out > strong_out,
        "weak individuals must yield higher PC output: {weak_out} vs {strong_out}"
    );
}

/// M2 commentary: real squad names fill the template slots; the old generic
/// placeholders are gone from the output.
#[test]
fn commentary_uses_real_names_not_generic_slots() {
    let lib = lib();
    let setup = setup_with_opp_strength(50);
    let stub_names: [&str; 5] = [
        "R. Stone",
        "M. Kessler",
        "D. Varga",
        "T. Okafor",
        "S. Lindqvist",
    ];

    let mut all_text = String::new();
    for seed in 0..20u64 {
        let r = auto_play_match(&lib, setup.clone(), &mut GoatRng::new(seed));
        for m in &r.moments {
            all_text.push_str(&m.setup_text);
            all_text.push('\n');
            all_text.push_str(&m.outcome_text);
            all_text.push('\n');
        }
    }

    for generic in [
        "your teammate",
        "Their forward",
        "their forward",
        "their winger",
    ] {
        assert!(
            !all_text.contains(generic),
            "generic placeholder survived: {generic:?}"
        );
    }
    assert!(
        !all_text.contains('{') && !all_text.contains('}'),
        "unfilled template slot in commentary"
    );
    let named = stub_names.iter().any(|n| all_text.contains(n));
    assert!(named, "no real squad name appeared in 20 matches");
}

/// The PC is never named as his own team's {scorer}/{assist} — he is "you".
#[test]
fn pc_never_fills_teammate_slots() {
    let lib = lib();
    let mut setup = setup_with_opp_strength(50);
    for p in setup.own_squad.players.iter_mut() {
        if p.is_pc {
            p.name = "THE PC HIMSELF".into();
        }
    }
    let mut all_text = String::new();
    for seed in 0..10u64 {
        let r = auto_play_match(&lib, setup.clone(), &mut GoatRng::new(seed));
        for m in &r.moments {
            all_text.push_str(&m.outcome_text);
        }
    }
    assert!(
        !all_text.contains("THE PC HIMSELF"),
        "the PC must never be drawn into teammate slots"
    );
}
