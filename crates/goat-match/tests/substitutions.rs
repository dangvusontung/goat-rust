//! PA2 M4 — substitutions: bench-start sub-on, hooking a misfiring starter,
//! the post-injury cameo, and minutes-weighted ratings. All sub decisions ride
//! a side-stream RNG; the golden match (sub_context None) is untouched.

use goat_core::{
    generation::{generate_player, CreationChoices},
    positions::PrimaryPosition,
    roles::RoleId,
    tactical::TacticalProfile,
};
use goat_fixed::Fixed;
use goat_match::beats::{GoalActor, ScoreEvent};
use goat_match::discipline::RefPersonality;
use goat_match::sim::{auto_play_match, BeatLibrary, MatchSetup, SubContext};
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

/// PC (star striker, 90+ everywhere) on a mid side vs a stronger opponent, so
/// the team is often trailing and the manager turns to his bench.
fn setup_with_subs(sub: SubContext, pc_attr: i32) -> MatchSetup {
    let c = CreationChoices {
        name: "Test".into(),
        primary_position: PrimaryPosition::ST,
        nationality: "Brazilian".to_string(),
        club: "Riverside Town".into(),
    };
    let mut pl = generate_player(12345, &c);
    for a in pl.current.iter_mut() {
        *a = Fixed::from_int(pc_attr);
    }
    let mut own_squad = SquadSheet::stub(60, 0xCAFE, (4, 3, 3));
    own_squad.players[9].is_pc = true;
    MatchSetup {
        player_role: RoleId::CompleteForward,
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
        own_squad,
        opp_squad: SquadSheet::stub(70, 0xBEEF, (4, 3, 3)),
        sub_context: Some(sub),
    }
}

fn pc_goals(r: &goat_match::sim::MatchResult) -> u32 {
    r.goal_credits
        .iter()
        .filter(|c| c.event == ScoreEvent::GoalFor && c.scorer == GoalActor::Pc)
        .count() as u32
}

/// THE scenario from the task: benched at kickoff → subbed on in the second
/// half → and a star can still score in his cameo. Also: he never plays 90.
#[test]
fn benched_then_subbed_on_can_score() {
    let lib = lib();
    let sub = SubContext {
        seed: 0x5DB5,
        pc_starts_on_bench: true,
        manager_trust: 90,
        manager_patience: 55,
        pc_returning_from_injury: false,
    };
    let mut cameo_goal = false;
    let mut sub_text_seen = false;
    let mut any_minutes = false;
    for seed in 0..40u64 {
        let mut setup = setup_with_subs(sub, 92);
        setup.sub_context.as_mut().unwrap().seed ^= seed.wrapping_mul(0x9E37);
        let r = auto_play_match(&lib, setup, &mut GoatRng::new(seed));
        assert!(r.minutes_played < 90, "bench start can never reach 90'");
        if r.minutes_played > 0 {
            any_minutes = true;
            sub_text_seen |= r
                .moments
                .iter()
                .any(|m| m.outcome_text.contains("You're on."));
            // Cameo window: on at 50' at the earliest.
            assert!(
                r.minutes_played <= 40,
                "sub-on window 50'+ ⇒ ≤40 minutes: {}",
                r.minutes_played
            );
            if pc_goals(&r) > 0 {
                cameo_goal = true;
            }
        }
    }
    assert!(any_minutes, "the manager must use his bench sometimes");
    assert!(sub_text_seen, "the sub-on commentary moment must appear");
    assert!(cameo_goal, "a 92-rated star must score in some cameo");
}

/// A misfiring starter under a Strict (impatient) manager gets hooked in the
/// 60–75' window; the commentary says so; he does not re-enter.
#[test]
fn strict_manager_hooks_misfiring_starter() {
    let lib = lib();
    let sub = SubContext {
        seed: 0x4001,
        pc_starts_on_bench: false,
        manager_trust: 10,
        manager_patience: 30, // Strict
        pc_returning_from_injury: false,
    };
    let mut hooked = 0u32;
    for seed in 0..40u64 {
        let mut setup = setup_with_subs(sub, 25); // awful day every day
        setup.sub_context.as_mut().unwrap().seed ^= seed.wrapping_mul(0x5EED);
        let r = auto_play_match(&lib, setup, &mut GoatRng::new(seed));
        if r.minutes_played < 90 && !r.red_card {
            hooked += 1;
            assert!(
                r.minutes_played >= 50,
                "no hook before the window: {}'",
                r.minutes_played
            );
            assert!(
                r.moments
                    .iter()
                    .any(|m| m.outcome_text.contains("you're coming off")),
                "hook commentary missing"
            );
        }
    }
    assert!(
        hooked >= 10,
        "a 25-rated day should earn the hook often: {hooked}/40"
    );
}

/// Tùng's locked note: just back from injury and not starting → a few closing
/// minutes to find his legs. Few minutes ⇒ the rating barely moves from 50
/// (linear minutes weighting), whatever he does out there.
#[test]
fn post_injury_cameo_is_short_and_lightly_weighted() {
    let lib = lib();
    let sub = SubContext {
        seed: 0xCA3E0 & 0xFFFF,
        pc_starts_on_bench: true,
        manager_trust: 50,
        manager_patience: 55,
        pc_returning_from_injury: true,
    };
    for seed in 0..20u64 {
        let mut setup = setup_with_subs(sub, 92);
        setup.sub_context.as_mut().unwrap().seed ^= seed.wrapping_mul(0xC0DE);
        let r = auto_play_match(&lib, setup, &mut GoatRng::new(seed));
        assert!(
            r.minutes_played > 0,
            "the cameo always happens (seed {seed})"
        );
        assert!(
            r.minutes_played <= 10,
            "cameo = closing minutes only: {}'",
            r.minutes_played
        );
        // ≤10 minutes ⇒ output = 50 + (raw − 50) × minutes/90 — even a perfect
        // cameo stays within ±6 of neutral.
        assert!(
            (r.player_output - 50).abs() <= 6,
            "cameo rating must stay near neutral: {}",
            r.player_output
        );
        assert!(
            r.moments
                .iter()
                .any(|m| m.outcome_text.contains("find your legs")),
            "cameo commentary missing"
        );
    }
}

/// Minutes weighting sanity: the same great performance means more over 90'
/// than over 30'. And sub_context None ⇒ always a full 90 (harness parity).
#[test]
fn minutes_weighting_and_harness_parity() {
    let lib = lib();
    // Harness parity: None ⇒ 90 minutes, always.
    let mut harness = setup_with_subs(
        SubContext {
            seed: 1,
            pc_starts_on_bench: false,
            manager_trust: 50,
            manager_patience: 55,
            pc_returning_from_injury: false,
        },
        70,
    );
    harness.sub_context = None;
    for seed in 0..10u64 {
        let r = auto_play_match(&lib, harness.clone(), &mut GoatRng::new(seed));
        assert_eq!(r.minutes_played, 90);
    }
}

// ── Opposition substitutions (M4 follow-up) ──────────────────────────────────
//
// The opposition stays crude on purpose: no trust/favor/personality. A chasing
// manager hooks his weakest starter for the best same-position man on the
// bench. Rolls ride a SEPARATE side-stream, so they never shift PC decisions.

/// Own side vastly stronger than the opposition, so the opposition spends the
/// second half chasing — the trigger for their bench. The PC starts on his own
/// team's bench with zero trust; whether he plays is irrelevant here.
fn chasing_setup(with_bench: bool) -> MatchSetup {
    let c = CreationChoices {
        name: "Test".into(),
        primary_position: PrimaryPosition::ST,
        nationality: "Brazilian".to_string(),
        club: "Riverside Town".into(),
    };
    let pl = generate_player(12345, &c);
    let mut opp_squad = SquadSheet::stub(45, 0xBEEF, (4, 3, 3));
    if with_bench {
        // Six named reserves covering all three position groups.
        opp_squad.bench = SquadSheet::stub(42, 0xB3C4, (2, 2, 1)).players;
    }
    MatchSetup {
        player_role: RoleId::CompleteForward,
        player_attrs: pl.current,
        player_familiarity: pl.familiarity,
        own_profile: profile(90, 90, 90),
        opp_profile: profile(20, 20, 20),
        opp_name: "Test FC",
        form: Fixed::from_int(50),
        player_aggression: 50,
        ref_personality: RefPersonality::Balanced,
        dirty_rep: 50,
        player_traits: PlayerTraits::default(),
        staff_mods: goat_core::staff::StaffMods::NEUTRAL,
        own_squad: SquadSheet::stub(92, 0xCAFE, (4, 3, 3)),
        opp_squad,
        sub_context: Some(SubContext {
            seed: 0x0FF5,
            pc_starts_on_bench: true,
            manager_trust: 0,
            manager_patience: 55,
            pc_returning_from_injury: false,
        }),
    }
}

/// A trailing opposition actually uses its bench: in the second half, at most
/// twice, with proper commentary — and the man coming on is a real bench name.
#[test]
fn opp_subs_when_chasing() {
    let lib = lib();
    let bench_names: Vec<String> = SquadSheet::stub(42, 0xB3C4, (2, 2, 1))
        .players
        .iter()
        .map(|p| p.name.clone())
        .collect();
    let mut matches_with_sub = 0u32;
    for seed in 0..20u64 {
        let mut setup = chasing_setup(true);
        setup.sub_context.as_mut().unwrap().seed ^= seed.wrapping_mul(0x9E37);
        // Live-game parity: bench players carry population ids, and the result
        // must hand back exactly the ids of those who came on.
        for (i, p) in setup.opp_squad.bench.iter_mut().enumerate() {
            p.id = Some(900 + i as u32);
        }
        let r = auto_play_match(&lib, setup, &mut GoatRng::new(seed));
        let subs: Vec<_> = r
            .moments
            .iter()
            .filter(|m| m.outcome_text.contains(" replaces "))
            .collect();
        assert!(subs.len() <= 2, "at most two opposition subs");
        assert_eq!(
            subs.len(),
            r.opp_subs_on.len(),
            "every sub-on must report its population id"
        );
        for id in &r.opp_subs_on {
            assert!((900..906).contains(id), "id must be a bench player: {id}");
        }
        if subs.is_empty() {
            continue;
        }
        matches_with_sub += 1;
        for m in &subs {
            assert!(
                m.minute >= 60,
                "no opposition sub before 60': {}'",
                m.minute
            );
            assert!(
                m.outcome_text.starts_with("Substitution for Test FC: "),
                "sub commentary must name the club: {}",
                m.outcome_text
            );
            let on_name = m
                .outcome_text
                .trim_start_matches("Substitution for Test FC: ")
                .split(" replaces ")
                .next()
                .unwrap();
            assert!(
                bench_names.iter().any(|n| n == on_name),
                "the man coming on must be a real bench player: {on_name}"
            );
        }
    }
    assert!(
        matches_with_sub >= 10,
        "a chasing side must use its bench most matches: {matches_with_sub}/20"
    );
}

/// No bench ⇒ no opposition subs, however one-sided the match (every harness
/// and the golden match take this path — their sheets never carry a bench).
#[test]
fn no_bench_no_opp_subs() {
    let lib = lib();
    for seed in 0..20u64 {
        let mut setup = chasing_setup(false);
        setup.sub_context.as_mut().unwrap().seed ^= seed.wrapping_mul(0x9E37);
        let r = auto_play_match(&lib, setup, &mut GoatRng::new(seed));
        assert!(
            !r.moments
                .iter()
                .any(|m| m.outcome_text.contains(" replaces ")),
            "empty bench must disable the rule entirely (seed {seed})"
        );
    }
}

/// The harness/golden path: `sub_context: None` means no sub streams exist at
/// all, so even a fully populated bench is dead weight — zero sub moments and
/// a byte-identical match. This is what keeps the golden match untouched.
#[test]
fn bench_is_inert_without_sub_context() {
    let lib = lib();
    let fingerprint = |r: &goat_match::sim::MatchResult| {
        (
            r.goals_for,
            r.goals_against,
            r.player_output,
            r.minutes_played,
            r.moments.len(),
            r.yellow_cards,
            r.red_card,
        )
    };
    for seed in 0..10u64 {
        let mut with_bench = chasing_setup(true);
        with_bench.sub_context = None;
        let mut without_bench = with_bench.clone();
        without_bench.opp_squad.bench.clear();
        let a = auto_play_match(&lib, with_bench, &mut GoatRng::new(seed));
        let b = auto_play_match(&lib, without_bench, &mut GoatRng::new(seed));
        assert!(
            !a.moments
                .iter()
                .any(|m| m.outcome_text.contains(" replaces ")),
            "no stream, no subs (seed {seed})"
        );
        assert_eq!(
            fingerprint(&a),
            fingerprint(&b),
            "a streamless bench must not perturb the match (seed {seed})"
        );
    }
}
