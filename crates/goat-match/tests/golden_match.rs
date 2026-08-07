//! Golden-seed match tests.
//! Values frozen from first green run after the generated-beat engine landed.
//! NEVER update to fix a failing test — if a golden fails, the change is wrong.

use goat_core::{
    attrs::NUM_ATTRS,
    generation::{generate_player, CreationChoices},
    positions::PrimaryPosition,
    roles::RoleId,
};
use goat_fixed::Fixed;
use goat_match::discipline::RefPersonality;
use goat_match::sim::{auto_play_match, BeatLibrary, MatchSetup};
use goat_rng::GoatRng;
use goat_traits::PlayerTraits;

const BEATS_JSON: &str = include_str!("../../../beats.json");

fn lib() -> BeatLibrary {
    BeatLibrary::load(BEATS_JSON).expect("beats.json must be valid")
}

fn forward_attrs() -> [Fixed; NUM_ATTRS] {
    let c = CreationChoices {
        name: "Test".into(),
        primary_position: PrimaryPosition::ST,
        primary_role: None,
        nationality: "Brazilian".to_string(),
        club: "Riverside Town".to_string(),
    };
    generate_player(12345, &c).current
}

fn forward_fam() -> [goat_core::roles::FamiliarityTier; goat_core::roles::NUM_ROLES] {
    let c = CreationChoices {
        name: "Test".into(),
        primary_position: PrimaryPosition::ST,
        primary_role: None,
        nationality: "Brazilian".to_string(),
        club: "Riverside Town".to_string(),
    };
    generate_player(12345, &c).familiarity
}

fn balanced_setup() -> MatchSetup {
    MatchSetup {
        player_role: RoleId::Trequartista,
        player_attrs: forward_attrs(),
        player_familiarity: forward_fam(),
        own_strength: 50,
        opp_strength: 50,
        opp_name: "Test FC".to_string(),
        form: Fixed::from_int(50),
        player_aggression: 50,
        ref_personality: RefPersonality::Balanced,
        dirty_rep: 50,
        player_traits: PlayerTraits::default(),
    }
}

/// Seed 42, balanced teams → exact output and scoreline frozen.
/// Re-frozen after C.9 tuning (KEY=95, IMP=92, SEC=88 — much higher starting attrs
/// for seed 12345 Forward, output hits cap at 100; scoreline unchanged as team
/// strengths are fixed 50v50 and goals come from team sim not player attrs).
/// Re-frozen again for the §A.3 position/role selector bias fix: `pick_beat`
/// previously ignored `player_role` entirely, so a Forward (this test uses
/// `RoleId::Trequartista`) drew defend-phase beats at the same rate as attack
/// beats. The old values (output=100, goals_for=2, goals_against=3) baked in
/// that pre-fix, unbiased beat selection for seed 42; the new values reflect
/// the corrected, role-weighted selection (see `sim.rs::role_bias_pct`).
#[test]
fn golden_seed_42_balanced_auto() {
    let result = auto_play_match(&lib(), balanced_setup(), &mut GoatRng::new(42));
    assert_eq!(result.player_output, 87, "output frozen at 87");
    assert_eq!(result.goals_for, 4, "goals_for frozen at 4");
    assert_eq!(result.goals_against, 1, "goals_against frozen at 1");
}

/// Demonstrates Output ≠ Result: a high-output performance can still end in a loss.
#[test]
fn output_independent_of_result() {
    let lib = lib();
    let mut setup = balanced_setup();
    setup.own_strength = 25;
    setup.opp_strength = 85;

    let mut found_high_output_loss = false;
    for seed in 0..50u64 {
        let r = auto_play_match(&lib, setup.clone(), &mut GoatRng::new(seed));
        if r.player_output > 60 && r.goals_against > r.goals_for {
            found_high_output_loss = true;
            break;
        }
    }
    assert!(
        found_high_output_loss,
        "must find at least one seed where output > 60 but team lost"
    );
}

/// Auto-play output must be in [0, 100].
#[test]
fn skip_and_play_use_same_engine() {
    let skip_result = auto_play_match(&lib(), balanced_setup(), &mut GoatRng::new(7));
    assert!(
        skip_result.player_output >= 0 && skip_result.player_output <= 100,
        "auto-play output must be in [0, 100]"
    );
}

/// Higher relevant attributes → higher average output over many seeded trials.
#[test]
fn high_attr_beats_low_attr_over_trials() {
    use goat_core::attrs::AttrId;
    let lib = lib();
    let mut high_setup = balanced_setup();
    let mut low_setup = balanced_setup();

    for a in [
        AttrId::Finishing,
        AttrId::CloseControl,
        AttrId::BallControl,
        AttrId::Agility,
        AttrId::LongShots,
        AttrId::Vision,
    ] {
        high_setup.player_attrs[a as usize] = Fixed::from_int(90);
        low_setup.player_attrs[a as usize] = Fixed::from_int(30);
    }

    let mut high_total = 0i32;
    let mut low_total = 0i32;
    for seed in 0..20u64 {
        high_total +=
            auto_play_match(&lib, high_setup.clone(), &mut GoatRng::new(seed)).player_output;
        low_total +=
            auto_play_match(&lib, low_setup.clone(), &mut GoatRng::new(seed)).player_output;
    }
    assert!(
        high_total > low_total,
        "higher attrs must produce higher average output: {high_total} vs {low_total}"
    );
}

/// Headspace stays in bounds across a full match.
#[test]
fn headspace_stays_bounded() {
    use goat_match::contest::auto_pick_generated_choice;
    use goat_match::sim::{advance_beat, start_match};
    let lib = lib();

    let mut ms = start_match(&lib, balanced_setup(), &mut GoatRng::new(99));
    let mut rng = GoatRng::new(99);

    while !ms.is_complete {
        let beat = match ms.current_beat() {
            Some(b) => b.clone(),
            None => break,
        };
        let idx = auto_pick_generated_choice(&beat.choices, &ms.setup.player_attrs);
        ms = advance_beat(ms, idx, &lib, &mut rng);

        assert!(
            ms.headspace.confidence >= -50 && ms.headspace.confidence <= 50,
            "confidence out of bounds"
        );
        assert!(
            ms.headspace.nerves >= 0 && ms.headspace.nerves <= 100,
            "nerves out of bounds"
        );
        assert!(
            ms.headspace.frustration >= 0 && ms.headspace.frustration <= 100,
            "frustration out of bounds"
        );
        assert!(
            ms.headspace.flow >= 0 && ms.headspace.flow <= 100,
            "flow out of bounds"
        );
    }
}

// ── Phase 6 discipline tests ──────────────────────────────────────────────────

use goat_core::attrs::AttrId;
use goat_match::contest::auto_pick_generated_choice;
use goat_match::sim::{advance_beat, start_match};

/// High Composure → accumulates less frustration than low Composure over 20 seeds.
#[test]
fn high_composure_stabilises_headspace() {
    let lib = lib();
    let mut high_comp = balanced_setup();
    let mut low_comp = balanced_setup();
    for a in 0..goat_core::attrs::NUM_ATTRS {
        high_comp.player_attrs[a] = Fixed::from_int(50);
        low_comp.player_attrs[a] = Fixed::from_int(50);
    }
    high_comp.player_attrs[AttrId::Composure as usize] = Fixed::from_int(90);
    low_comp.player_attrs[AttrId::Composure as usize] = Fixed::from_int(10);

    let mut high_frust_total = 0i32;
    let mut low_frust_total = 0i32;

    for seed in 0..20u64 {
        let mut ms_h = start_match(&lib, high_comp.clone(), &mut GoatRng::new(seed));
        let mut ms_l = start_match(&lib, low_comp.clone(), &mut GoatRng::new(seed));
        let mut rng_h = GoatRng::new(seed);
        let mut rng_l = GoatRng::new(seed);

        while !ms_h.is_complete {
            if let Some(b) = ms_h.current_beat().cloned() {
                let i = auto_pick_generated_choice(&b.choices, &ms_h.setup.player_attrs);
                ms_h = advance_beat(ms_h, i, &lib, &mut rng_h);
            } else {
                break;
            }
        }
        while !ms_l.is_complete {
            if let Some(b) = ms_l.current_beat().cloned() {
                let i = auto_pick_generated_choice(&b.choices, &ms_l.setup.player_attrs);
                ms_l = advance_beat(ms_l, i, &lib, &mut rng_l);
            } else {
                break;
            }
        }

        high_frust_total += ms_h.headspace.frustration;
        low_frust_total += ms_l.headspace.frustration;
    }

    assert!(
        low_frust_total >= high_frust_total,
        "low composure should accumulate more frustration ({low_frust_total} vs {high_frust_total})"
    );
}

/// Strict ref + high Aggression → more cards than lenient ref + low Aggression.
#[test]
fn strict_ref_and_aggression_produces_more_cards() {
    use goat_match::discipline::RefPersonality;
    let lib = lib();

    let make_tackling_setup = |ref_p: RefPersonality, aggression: u8, dirty: i32| -> MatchSetup {
        let mut s = balanced_setup();
        s.player_attrs[AttrId::StandingTackle as usize] = Fixed::from_int(99);
        s.player_attrs[AttrId::Aggression as usize] = Fixed::from_int(aggression as i32);
        s.ref_personality = ref_p;
        s.player_aggression = aggression;
        s.dirty_rep = dirty;
        s
    };

    let strict_setup = make_tackling_setup(RefPersonality::Strict, 90, 90);
    let lenient_setup = make_tackling_setup(RefPersonality::Lenient, 10, 10);

    let mut strict_cards = 0u32;
    let mut lenient_cards = 0u32;

    for seed in 0..100u64 {
        let r_strict = auto_play_match(&lib, strict_setup.clone(), &mut GoatRng::new(seed));
        let r_lenient = auto_play_match(&lib, lenient_setup.clone(), &mut GoatRng::new(seed));
        strict_cards += r_strict.yellow_cards as u32 + if r_strict.red_card { 3 } else { 0 };
        lenient_cards += r_lenient.yellow_cards as u32 + if r_lenient.red_card { 3 } else { 0 };
    }

    assert!(
        strict_cards > lenient_cards,
        "strict+aggressive should get more cards: {strict_cards} vs {lenient_cards}"
    );
}

/// Form influence on headspace: in-form player starts with better confidence/nerves.
#[test]
fn form_influences_starting_headspace() {
    let lib = lib();
    let mut high_form = balanced_setup();
    high_form.form = Fixed::from_int(85);
    let mut low_form = balanced_setup();
    low_form.form = Fixed::from_int(15);

    let mut rng = GoatRng::new(0);
    let ms_high = start_match(&lib, high_form, &mut rng);
    let mut rng = GoatRng::new(0);
    let ms_low = start_match(&lib, low_form, &mut rng);

    assert!(
        ms_high.headspace.confidence > ms_low.headspace.confidence,
        "high form should start with more confidence: {} vs {}",
        ms_high.headspace.confidence,
        ms_low.headspace.confidence
    );
    assert!(
        ms_high.headspace.nerves < ms_low.headspace.nerves,
        "high form should start calmer: {} vs {}",
        ms_high.headspace.nerves,
        ms_low.headspace.nerves
    );
}

// ── Position bias (§A.3) ──────────────────────────────────────────────────────

/// A Forward must face far fewer on-ball defensive-decision beats than a
/// Defender across many matches, and a Defender far fewer on-ball attacking
/// beats than a Forward — but neither count may hit zero across the sample,
/// since role biases the selector's weights, it does not lock a beat list.
#[test]
fn position_bias_suppresses_but_does_not_lock_off_role_beats() {
    use goat_match::beats::MatchPhase;

    let lib = lib();
    let traits = PlayerTraits::default();

    let mut forward_defend = 0u32;
    let mut forward_attack = 0u32;
    let mut defender_defend = 0u32;
    let mut defender_attack = 0u32;

    for seed in 0..200u64 {
        let mut rng_f = GoatRng::new(seed);
        for b in lib.generate_match(&mut rng_f, RoleId::CompleteForward, &traits) {
            match b.phase {
                MatchPhase::OpenPlayDefend => forward_defend += 1,
                MatchPhase::OpenPlayAttack => forward_attack += 1,
                _ => {}
            }
        }

        let mut rng_d = GoatRng::new(seed);
        for b in lib.generate_match(&mut rng_d, RoleId::CentreBack, &traits) {
            match b.phase {
                MatchPhase::OpenPlayDefend => defender_defend += 1,
                MatchPhase::OpenPlayAttack => defender_attack += 1,
                _ => {}
            }
        }
    }

    assert!(
        forward_defend < defender_defend,
        "forward should face fewer defensive beats than a defender: {forward_defend} vs {defender_defend}"
    );
    assert!(
        defender_attack < forward_attack,
        "defender should face fewer attacking beats than a forward: {defender_attack} vs {forward_attack}"
    );
    assert!(
        forward_defend > 0,
        "off-role defensive beats must still be reachable for a forward (bias, not lock)"
    );
    assert!(
        defender_attack > 0,
        "off-role attacking beats must still be reachable for a defender (bias, not lock)"
    );
}

#[test]
fn dump_c9_match_values() {
    let result = auto_play_match(&lib(), balanced_setup(), &mut GoatRng::new(42));
    eprintln!(
        "C.9 match values: output={} goals_for={} goals_against={} yellows={} red={}",
        result.player_output,
        result.goals_for,
        result.goals_against,
        result.yellow_cards,
        result.red_card
    );
}
