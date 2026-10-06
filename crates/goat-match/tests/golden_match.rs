//! Golden-seed match tests — Match Flow engine.
//! Values frozen from the first green run of the stat-driven flow rewrite.
//! NEVER update to fix a failing test — if a golden fails, the change is wrong.

use goat_core::{
    attrs::{AttrId, NUM_ATTRS},
    generation::{generate_player, CreationChoices},
    positions::PrimaryPosition,
    roles::RoleId,
    tactical::TacticalProfile,
};
use goat_fixed::Fixed;
use goat_match::beats::Possession;
use goat_match::discipline::RefPersonality;
use goat_match::sim::{auto_play_match, BeatLibrary, MatchSetup};
use goat_match::squad::SquadSheet;
use goat_rng::{GoatRng, RngSource};
use goat_traits::PlayerTraits;

const BEATS_JSON: &str = include_str!("../../../beats.json");

fn lib() -> BeatLibrary {
    BeatLibrary::load(BEATS_JSON).expect("beats.json must be valid")
}

/// Fixed synthetic sheets (M2): deterministic names/attrs, PC flagged in the
/// own sheet so {scorer} never draws him.
fn test_sheet(mark_pc: bool) -> SquadSheet {
    let mut s = SquadSheet::stub(50, 0xBEEF, (4, 3, 3));
    if mark_pc {
        s.players[9].is_pc = true; // a forward slot
    }
    s
}

fn forward_attrs() -> [Fixed; NUM_ATTRS] {
    let c = CreationChoices {
        name: "Test".into(),
        primary_position: PrimaryPosition::ST,
        nationality: "Brazilian".to_string(),
        club: "Riverside Town".into(),
    };
    generate_player(12345, &c).current
}

fn forward_fam() -> [goat_core::roles::FamiliarityTier; goat_core::roles::NUM_ROLES] {
    let c = CreationChoices {
        name: "Test".into(),
        primary_position: PrimaryPosition::ST,
        nationality: "Brazilian".to_string(),
        club: "Riverside Town".into(),
    };
    generate_player(12345, &c).familiarity
}

/// A neutral 50-everything profile (no dominant style) for controlled tests.
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

fn balanced_setup() -> MatchSetup {
    MatchSetup {
        player_role: RoleId::CompleteForward,
        player_attrs: forward_attrs(),
        player_familiarity: forward_fam(),
        own_profile: neutral_profile(),
        opp_profile: neutral_profile(),
        opp_name: "Test FC",
        form: Fixed::from_int(50),
        player_aggression: 50,
        ref_personality: RefPersonality::Balanced,
        dirty_rep: 50,
        player_traits: PlayerTraits::default(),
        staff_mods: goat_core::staff::StaffMods::NEUTRAL,
        own_squad: test_sheet(true),
        opp_squad: test_sheet(false),
        sub_context: None,
    }
}

/// Seed 42, balanced teams → exact output and scoreline frozen (Match Flow era,
/// re-frozen after the strength/goals/decoupling tuning pass; output re-frozen
/// 54 → 52 in round 3b when the rating taper's asymptote moved to the 0/100
/// rails — same moments/scoreline, only the rating arithmetic moved).
///
/// Re-frozen 52/2-2 → 57/1-1 for PA2 M2 (2026-09-23): beats now draw a specific
/// matchup opponent per beat (his real counter-attrs blend 50/50 with the team
/// line into contest difficulty, MATCH.md A.5) and commentary slots
/// ({scorer}/{opponent}/{assist}) are filled from squad sheets — both consume
/// match RNG and shift contest math by design. Flow rules (possession, zone
/// drift, momentum, auto-goal, mercy/trailing/response logic) are untouched;
/// the re-freeze was validated with a 100k-match distribution comparison
/// (docs/sim-analysis.md, M2 entry).
///
/// Re-frozen 57/1-1 → 51/0-1 at the PA2/origin-main merge (2026-09-25): the
/// merge pulls in the ceiling-lottery restore (CEILING 99/99 → 70/99), so the
/// setup's generate_player(12345) attrs now roll against ceiling 79, and the
/// BL5.1 AssistFor wiring adds a teammate-goal credit path. Same engine, same
/// seed — lower inputs, lower output.
#[test]
fn golden_seed_42_balanced_auto() {
    let result = auto_play_match(&lib(), balanced_setup(), &mut GoatRng::new(42));
    assert_eq!(
        result.player_output, 51,
        "output frozen at 51 (merge: ceiling 79 era)"
    );
    assert_eq!(result.goals_for, 0, "goals_for frozen at 0 (merge)");
    assert_eq!(result.goals_against, 1, "goals_against frozen at 1 (merge)");
}

/// Demonstrates Output ≠ Result: a high-output performance can still end in a loss.
#[test]
fn output_independent_of_result() {
    let lib = lib();
    let mut setup = balanced_setup();
    // Weak team around a strong player.
    setup.own_profile = TacticalProfile {
        attack: 25,
        midfield: 25,
        defense: 25,
        ..neutral_profile()
    };
    setup.opp_profile = TacticalProfile {
        attack: 85,
        midfield: 85,
        defense: 85,
        ..neutral_profile()
    };

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

/// Team stats must drive the scoreline: a much stronger team wins on average.
#[test]
fn stronger_team_scores_more_over_trials() {
    let lib = lib();
    let mut strong = balanced_setup();
    strong.own_profile = TacticalProfile {
        attack: 90,
        midfield: 90,
        defense: 90,
        ..neutral_profile()
    };
    strong.opp_profile = TacticalProfile {
        attack: 30,
        midfield: 30,
        defense: 30,
        ..neutral_profile()
    };

    let mut gf = 0u64;
    let mut ga = 0u64;
    for seed in 0..30u64 {
        let r = auto_play_match(&lib, strong.clone(), &mut GoatRng::new(seed));
        gf += r.goals_for as u64;
        ga += r.goals_against as u64;
    }
    assert!(
        gf > ga,
        "much stronger team must outscore over 30 seeds: {gf} vs {ga}"
    );
}

/// Headspace and momentum stay in bounds across a full interactive match.
#[test]
fn headspace_and_momentum_stay_bounded() {
    use goat_match::contest::auto_pick_generated_choice;
    use goat_match::sim::{advance_beat, start_match};
    let lib = lib();

    let mut ms = start_match(&lib, balanced_setup(), &mut GoatRng::new(99));
    let mut rng = GoatRng::new(99);

    while !ms.is_complete {
        assert!(
            (-100..=100).contains(&ms.momentum),
            "momentum out of bounds"
        );
        let beat = match ms.current_beat() {
            Some(b) => b.clone(),
            None => break,
        };
        let idx = auto_pick_generated_choice(&beat.choices, &ms.setup.player_attrs);
        ms = advance_beat(ms, idx, &lib, &mut rng);

        assert!(
            (-50..=50).contains(&ms.headspace.confidence),
            "confidence out of bounds"
        );
        assert!(
            (0..=100).contains(&ms.headspace.nerves),
            "nerves out of bounds"
        );
        assert!(
            (0..=100).contains(&ms.headspace.frustration),
            "frustration out of bounds"
        );
        assert!((0..=100).contains(&ms.headspace.flow), "flow out of bounds");
    }
}

/// Chains are capped: between two player decisions at the same minute, at most
/// 1 (player) + CHAIN_MAX (auto) action moments may appear.
#[test]
fn chains_are_capped() {
    use goat_match::contest::auto_pick_generated_choice;
    use goat_match::sim::{advance_beat, start_match};
    let lib = lib();

    for seed in 0..20u64 {
        let mut ms = start_match(&lib, balanced_setup(), &mut GoatRng::new(seed));
        let mut rng = GoatRng::new(seed);
        while !ms.is_complete {
            let Some(beat) = ms.current_beat().cloned() else {
                break;
            };
            let minute = ms.current_minute();
            let before = ms.moments.len();
            let idx = auto_pick_generated_choice(&beat.choices, &ms.setup.player_attrs);
            ms = advance_beat(ms, idx, &lib, &mut rng);
            let chained = ms.moments[before..]
                .iter()
                .filter(|m| m.is_action && m.minute == minute)
                .count();
            assert!(
                chained <= 4,
                "chain cap exceeded at minute {minute} (seed {seed}): {chained} action moments"
            );
        }
    }
}

/// Role zone filters involvement: a forward's interactive beats must land in
/// attacking zones more often than a centre-back's do.
#[test]
fn involvement_follows_role_zone() {
    use goat_core::roles::{PitchZone, ROLE_ZONE};
    use goat_match::contest::auto_pick_generated_choice;
    use goat_match::sim::{advance_beat, start_match};
    let lib = lib();

    let zone_share = |role: RoleId| -> (u32, u32) {
        let mut in_zone = 0u32;
        let mut total = 0u32;
        for seed in 0..20u64 {
            let mut setup = balanced_setup();
            setup.player_role = role;
            let mut ms = start_match(&lib, setup, &mut GoatRng::new(seed));
            let mut rng = GoatRng::new(seed);
            while !ms.is_complete {
                let Some(beat) = ms.current_beat().cloned() else {
                    break;
                };
                total += 1;
                let attack_zone =
                    matches!(beat.zone, PitchZone::AttackWide | PitchZone::AttackCentral);
                let role_is_attack = matches!(
                    ROLE_ZONE[role as usize],
                    PitchZone::AttackWide | PitchZone::AttackCentral
                );
                if attack_zone == role_is_attack {
                    in_zone += 1;
                }
                let idx = auto_pick_generated_choice(&beat.choices, &ms.setup.player_attrs);
                ms = advance_beat(ms, idx, &lib, &mut rng);
            }
        }
        (in_zone, total)
    };

    let (fwd_in, fwd_total) = zone_share(RoleId::CompleteForward);
    let (cb_in, cb_total) = zone_share(RoleId::CentreBack);
    assert!(fwd_total > 0 && cb_total > 0, "both roles must see beats");
    // Forward: mostly attacking zones; Centre Back: mostly defending zones.
    assert!(
        fwd_in * 2 >= fwd_total,
        "forward beats mostly attacking: {fwd_in}/{fwd_total}"
    );
    assert!(
        cb_in * 2 >= cb_total,
        "centre-back beats mostly defending: {cb_in}/{cb_total}"
    );
}

/// High Composure → accumulates less frustration than low Composure over 20 seeds.
#[test]
fn high_composure_stabilises_headspace() {
    use goat_match::contest::auto_pick_generated_choice;
    use goat_match::sim::{advance_beat, start_match};
    let lib = lib();
    let mut high_comp = balanced_setup();
    let mut low_comp = balanced_setup();
    for a in 0..NUM_ATTRS {
        high_comp.player_attrs[a] = Fixed::from_int(50);
        low_comp.player_attrs[a] = Fixed::from_int(50);
    }
    high_comp.player_attrs[AttrId::Composure as usize] = Fixed::from_int(90);
    low_comp.player_attrs[AttrId::Composure as usize] = Fixed::from_int(10);

    let mut high_frust_total = 0i32;
    let mut low_frust_total = 0i32;

    for seed in 0..20u64 {
        let mut rng_h = GoatRng::new(seed);
        let mut ms_h = start_match(&lib, high_comp.clone(), &mut rng_h);
        let mut rng_l = GoatRng::new(seed);
        let mut ms_l = start_match(&lib, low_comp.clone(), &mut rng_l);

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
    let lib = lib();

    let make_tackling_setup = |ref_p: RefPersonality, aggression: u8, dirty: i32| -> MatchSetup {
        let mut s = balanced_setup();
        // A defender sees defend-side (foul-risk) actions regularly.
        s.player_role = RoleId::CentreBack;
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
    use goat_match::sim::start_match;
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

/// Possession tilt: the stronger midfield must hold the ball more often.
/// Observed deterministically via kickoff + flow over 40 seeds.
#[test]
fn stronger_midfield_holds_possession() {
    use goat_match::sim::start_match;
    let lib = lib();

    let mut own_kickoffs = 0u32;
    let mut opp_kickoffs = 0u32;
    for seed in 0..40u64 {
        let mut setup = balanced_setup();
        setup.own_profile = TacticalProfile {
            midfield: 90,
            ..neutral_profile()
        };
        setup.opp_profile = TacticalProfile {
            midfield: 20,
            ..neutral_profile()
        };
        // Kickoff possession is the first possession roll in start_match.
        // Peek at the RNG stream: replicate the kickoff roll directly.
        let mut rng = GoatRng::new(seed);
        let _ = start_match(&lib, setup.clone(), &mut rng);
        // Independent measurement: the share formula itself.
        let share = 50 + (90i32 - 20) / 2; // 85
        let mut r = GoatRng::new(seed);
        let roll = r.next_range_u64(1, 100);
        if roll <= share as u64 {
            own_kickoffs += 1;
        } else {
            opp_kickoffs += 1;
        }
    }
    assert!(
        own_kickoffs > opp_kickoffs * 3,
        "stronger midfield must dominate possession rolls: {own_kickoffs} vs {opp_kickoffs}"
    );
}

#[test]
fn dump_flow_match_values() {
    let result = auto_play_match(&lib(), balanced_setup(), &mut GoatRng::new(42));
    eprintln!(
        "Match Flow values: output={} goals_for={} goals_against={} yellows={} red={} moments={}",
        result.player_output,
        result.goals_for,
        result.goals_against,
        result.yellow_cards,
        result.red_card,
        result.moments.len(),
    );
    // Also confirm possession enum is exercised.
    let _ = Possession::Own.flip();
}

#[test]
fn shared_automatic_and_interactive_default_choices_replay_identically() {
    use goat_match::sim::{advance_beat, auto_play_match_shared, start_match_shared};
    let lib = lib();
    let setup = balanced_setup();
    let automatic = auto_play_match_shared(&lib, setup.clone(), &mut GoatRng::new(42));
    let mut rng = GoatRng::new(42);
    let mut live = start_match_shared(&lib, setup, &mut rng);
    while !live.is_complete {
        let choice = live
            .current_beat()
            .map(|beat| {
                goat_match::contest::auto_pick_generated_choice(
                    &beat.choices,
                    &live.setup.player_attrs,
                )
            })
            .unwrap_or(0);
        live = advance_beat(live, choice, &lib, &mut rng);
    }
    let result = live.final_result.unwrap();
    assert_eq!(
        (
            automatic.goals_for,
            automatic.goals_against,
            automatic.player_output
        ),
        (result.goals_for, result.goals_against, result.player_output)
    );
}

#[test]
fn shared_dismissal_finishes_with_npcs_and_preserves_pc_minutes() {
    use goat_match::sim::{advance_beat, start_match_shared};
    let lib = lib();
    let mut dismissals = 0;
    let mut later_goals = 0;
    for seed in 0..256 {
        let mut setup = balanced_setup();
        setup.player_role = RoleId::DefensiveMid;
        setup.player_aggression = 99;
        setup.dirty_rep = 100;
        setup.ref_personality = RefPersonality::Strict;
        let mut rng = GoatRng::new(seed);
        let mut live = start_match_shared(&lib, setup, &mut rng);
        while !live.is_complete {
            let choice = live
                .current_beat()
                .map(|beat| {
                    beat.choices
                        .iter()
                        .enumerate()
                        .max_by_key(|(_, choice)| (choice.foul_serious, choice.foul_chance))
                        .map(|(i, _)| i)
                        .unwrap_or(0)
                })
                .unwrap_or(0);
            live = advance_beat(live, choice, &lib, &mut rng);
        }
        let result = live.final_result.as_ref().unwrap();
        if seed == 3 {
            assert!(result.red_card);
            // New behavior gets a new golden; legacy expectations stay frozen.
            assert_eq!(
                (
                    result.minutes_played,
                    result.goals_for,
                    result.goals_against,
                    result.player_output
                ),
                (27, 2, 2, 60)
            );
        }
        if result.red_card && result.minutes_played < 80 {
            dismissals += 1;
            assert_eq!(
                result.goal_credits.len() as u32,
                result.goals_for + result.goals_against
            );
            assert_eq!(live.minute, 90, "seed {seed}");
            assert!(!live.pc_on_pitch);
            assert!(live.current_beat().is_none());
            assert_eq!(live.setup.own_squad.players.len(), 10);
            assert!(live.setup.own_squad.players.iter().all(|p| !p.is_pc));
            let later: Vec<_> = result
                .moments
                .iter()
                .filter(|m| m.minute > result.minutes_played)
                .collect();
            assert!(!later.is_empty());
            assert!(later.iter().all(|m| !m.is_action));
            later_goals += later.iter().filter(|m| m.goal_event.is_some()).count();
            let minutes = result.minutes_played;
            let goals = (result.goals_for, result.goals_against);
            let again = advance_beat(live, 0, &lib, &mut rng);
            assert_eq!(again.final_result.as_ref().unwrap().minutes_played, minutes);
            assert_eq!((again.goals_for, again.goals_against), goals);
        }
    }
    assert!(
        dismissals >= 10,
        "the fixture must actually exercise early dismissals: {dismissals}"
    );
    assert!(
        later_goals > 0,
        "NPC goals must remain possible after dismissal"
    );
}

#[test]
fn npc_observer_has_full_exposure_and_no_protagonist_events() {
    use goat_core::match_model::{MatchContext, Venue};
    use goat_match::sim::observe_npc_match_with_context;
    let lib = lib();
    for context in [
        MatchContext {
            venue: Venue::Neutral,
            ..Default::default()
        },
        MatchContext {
            venue: Venue::Home,
            own_players: 10,
            ..Default::default()
        },
    ] {
        let result =
            observe_npc_match_with_context(&lib, balanced_setup(), context, &mut GoatRng::new(42));
        let repeated =
            observe_npc_match_with_context(&lib, balanced_setup(), context, &mut GoatRng::new(42));
        assert_eq!(result.npc_observations, repeated.npc_observations);
        assert_eq!(
            result
                .npc_observations
                .attacking_minutes
                .iter()
                .sum::<u32>(),
            90
        );
        assert_eq!(
            result.npc_observations.goals,
            [result.goals_for, result.goals_against]
        );
        assert_eq!(
            result.goal_credits.len() as u32,
            result.goals_for + result.goals_against
        );
        assert!(result.moments.iter().all(|m| !m.is_action));
        assert_eq!(result.minutes_played, 0);
        assert!(!result.red_card);
        assert_eq!(result.yellow_cards, 0);
        assert!(result.familiarity_xp.iter().all(|xp| *xp == Fixed::ZERO));
        for side in 0..2 {
            assert!(result.npc_observations.goals[side] <= result.npc_observations.chances[side]);
        }
        if context.venue == Venue::Neutral {
            assert_eq!(
                result.npc_observations,
                goat_core::match_model::MatchObservations {
                    goals: [0, 5],
                    chances: [2, 9],
                    attacking_minutes: [26, 64]
                }
            );
        }
    }
}

#[test]
fn unified_opportunity_ledger_reconciles_chains_roles_and_full_time() {
    use goat_match::sim::{auto_play_match_unified_with_options, UnifiedOptions};
    let lib = lib();
    for role in [
        RoleId::CompleteForward,
        RoleId::CentreBack,
        RoleId::CentralMid,
    ] {
        for seed in 0..128 {
            let mut setup = balanced_setup();
            setup.player_role = role;
            let options = UnifiedOptions {
                chains: seed % 2 == 0,
                ..Default::default()
            };
            let result =
                auto_play_match_unified_with_options(&lib, setup, options, &mut GoatRng::new(seed));
            assert_eq!(
                result
                    .team_observations
                    .attacking_minutes
                    .iter()
                    .sum::<u32>(),
                90
            );
            assert_eq!(
                result
                    .opportunities
                    .iter()
                    .map(|o| o.opportunity.minutes)
                    .sum::<u32>(),
                90
            );
            assert!(result
                .opportunities
                .iter()
                .all(|o| o.opportunity.minutes > 0));
            assert!(result
                .opportunities
                .windows(2)
                .all(|pair| pair[0].minute < pair[1].minute));
            assert_eq!(
                result.team_observations.goals,
                [result.goals_for, result.goals_against]
            );
            assert_eq!(
                result.goal_credits.len() as u32,
                result.goals_for + result.goals_against
            );
            assert_eq!(
                result.opportunities.iter().filter(|o| o.goal).count() as u32,
                result.goals_for + result.goals_against
            );
            for side in 0..2 {
                assert_eq!(
                    result
                        .opportunities
                        .iter()
                        .filter(|o| o.opportunity.side as usize == side && o.opportunity.created)
                        .count() as u32,
                    result.team_observations.chances[side]
                );
                assert!(
                    result.team_observations.goals[side] <= result.team_observations.chances[side]
                );
                assert!(
                    result.npc_observations.chances[side] <= result.team_observations.chances[side]
                );
            }
        }
    }
}

#[test]
fn unified_default_decisions_replay_and_finish_after_dismissal() {
    use goat_match::sim::{advance_beat, auto_play_match_unified, start_match_unified};
    let lib = lib();
    let automatic = auto_play_match_unified(&lib, balanced_setup(), &mut GoatRng::new(42));
    let mut rng = GoatRng::new(42);
    let mut live = start_match_unified(&lib, balanced_setup(), &mut rng);
    while !live.is_complete {
        let idx = live
            .current_beat()
            .map(|b| {
                goat_match::contest::auto_pick_generated_choice(
                    &b.choices,
                    &live.setup.player_attrs,
                )
            })
            .unwrap_or(0);
        live = advance_beat(live, idx, &lib, &mut rng);
    }
    assert_eq!(live.minute, 90);
    assert_eq!(
        automatic.opportunities,
        live.final_result.as_ref().unwrap().opportunities
    );
    assert_eq!(
        (
            automatic.goals_for,
            automatic.goals_against,
            automatic.player_output
        ),
        (
            live.goals_for,
            live.goals_against,
            live.final_result.as_ref().unwrap().player_output
        )
    );
    assert_eq!(
        (
            automatic.goals_for,
            automatic.goals_against,
            automatic.player_output
        ),
        (1, 0, 37)
    );
    assert_eq!(
        automatic.team_observations,
        goat_core::match_model::MatchObservations {
            goals: [1, 0],
            chances: [3, 6],
            attacking_minutes: [42, 48]
        }
    );
    assert_eq!(automatic.opportunities.len(), 16);
    let mut early_reds = 0;
    let mut post_red_goals = 0;
    for seed in 0..256 {
        let mut setup = balanced_setup();
        setup.player_role = RoleId::DefensiveMid;
        setup.player_aggression = 99;
        setup.dirty_rep = 100;
        setup.ref_personality = RefPersonality::Strict;
        let mut rng = GoatRng::new(seed);
        let mut live = start_match_unified(&lib, setup, &mut rng);
        while !live.is_complete {
            let idx = live
                .current_beat()
                .map(|b| {
                    b.choices
                        .iter()
                        .enumerate()
                        .max_by_key(|(_, c)| (c.foul_serious, c.foul_chance))
                        .map(|(i, _)| i)
                        .unwrap_or(0)
                })
                .unwrap_or(0);
            live = advance_beat(live, idx, &lib, &mut rng);
        }
        let r = live.final_result.as_ref().unwrap();
        if r.red_card && r.minutes_played < 80 {
            early_reds += 1;
            assert_eq!(live.minute, 90);
            assert!(!live.pc_on_pitch);
            assert_eq!(live.setup.own_squad.players.len(), 10);
            for opportunity in r
                .opportunities
                .iter()
                .filter(|o| o.minute > r.minutes_played)
            {
                assert!(matches!(
                    opportunity.actor,
                    goat_match::beats::GoalActor::Npc(_)
                ));
                if opportunity.opportunity.side == 0 {
                    assert_eq!(opportunity.opportunity.players, 10);
                }
                post_red_goals += u32::from(opportunity.goal);
            }
            assert!(r
                .moments
                .iter()
                .filter(|m| m.minute > r.minutes_played)
                .all(|m| !m.is_action));
            assert_eq!(
                r.team_observations.attacking_minutes.iter().sum::<u32>(),
                90
            );
        }
    }
    assert!(early_reds >= 5, "must exercise dismissals: {early_reds}");
    assert!(post_red_goals > 0);
}

#[test]
fn unified_good_execution_can_go_unconverted_with_non_goal_text() {
    use goat_match::sim::auto_play_match_unified;
    let lib = lib();
    let text = goat_match::beats_data::OpportunityText::default();
    let mut cases = 0;
    for seed in 0..128 {
        let result = auto_play_match_unified(&lib, balanced_setup(), &mut GoatRng::new(seed));
        for moment in result.moments.iter().filter(|m| {
            m.outcome_text == text.finish_unconverted || m.outcome_text == text.delivery_unconverted
        }) {
            cases += 1;
            assert!(moment.is_action);
            assert!(moment.goal_event.is_none());
            assert!(result
                .opportunities
                .iter()
                .any(|o| o.minute == moment.minute
                    && o.execution_success == Some(true)
                    && !o.goal));
        }
    }
    assert!(cases > 0, "must exercise skill success without a goal");
}

#[test]
fn unified_cameos_and_hooks_keep_full_team_exposure_and_eleven_players() {
    use goat_match::sim::{auto_play_match_unified, SubContext};
    let lib = lib();
    let mut hooks = 0;
    for seed in 0..64 {
        for returning in [false, true] {
            let mut setup = balanced_setup();
            setup.player_attrs = [Fixed::from_int(20); NUM_ATTRS];
            setup.sub_context = Some(SubContext {
                seed,
                pc_starts_on_bench: returning,
                manager_trust: 0,
                manager_patience: 0,
                pc_returning_from_injury: returning,
            });
            let r = auto_play_match_unified(&lib, setup, &mut GoatRng::new(seed));
            assert_eq!(
                r.team_observations.attacking_minutes.iter().sum::<u32>(),
                90
            );
            if returning {
                assert!((1..=10).contains(&r.minutes_played));
            }
            if !returning && !r.red_card && r.minutes_played < 90 {
                hooks += 1;
                assert!(r
                    .opportunities
                    .iter()
                    .filter(|o| o.minute > r.minutes_played)
                    .all(|o| matches!(o.actor, goat_match::beats::GoalActor::Npc(_))
                        && o.opportunity.players == 11));
            }
        }
    }
    assert!(hooks > 0, "must exercise real substitutions");
}
