//! PA2 M3 — goal credits: every goal is attributed to a real actor (PC or a
//! specific population NPC) so the live game can persist career stats.

use goat_core::{
    generation::{generate_player, CreationChoices, Position},
    roles::RoleId,
    tactical::TacticalProfile,
};
use goat_fixed::Fixed;
use goat_match::beats::{GoalActor, ScoreEvent};
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

/// Sheets with real population-style ids: own = 100.., opp = 200..; the PC
/// holds one own-sheet slot (id `None`, like the live game).
fn setup_with_ids() -> MatchSetup {
    let c = CreationChoices {
        name: "Test".into(),
        position: Position::Forward,
        nationality: "Brazilian",
        club: "Riverside Town".into(),
    };
    let pl = generate_player(12345, &c);
    let mut own_squad = SquadSheet::stub(65, 0xCAFE, (4, 3, 3));
    for (i, p) in own_squad.players.iter_mut().enumerate() {
        p.id = Some(100 + i as u32);
    }
    own_squad.players[9].is_pc = true;
    own_squad.players[9].id = None;
    let mut opp_squad = SquadSheet::stub(60, 0xBEEF, (4, 3, 3));
    for (i, p) in opp_squad.players.iter_mut().enumerate() {
        p.id = Some(200 + i as u32);
    }
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
    }
}

/// Every goal in the scoreline is credited exactly once, to an actor on the
/// correct side — own-sheet ids for GoalFor, opp-sheet ids for GoalAgainst.
#[test]
fn every_goal_is_credited_to_the_right_side() {
    let lib = lib();
    for seed in 0..30u64 {
        let r = auto_play_match(&lib, setup_with_ids(), &mut GoatRng::new(seed));
        assert_eq!(
            r.goal_credits.len() as u32,
            r.goals_for + r.goals_against,
            "seed {seed}: one credit per goal"
        );
        for c in &r.goal_credits {
            match (c.event, c.scorer) {
                (ScoreEvent::GoalFor, GoalActor::Pc) => {}
                (ScoreEvent::GoalFor, GoalActor::Npc(Some(id))) => {
                    assert!((100..111).contains(&id), "own scorer id: {id}");
                }
                (ScoreEvent::GoalAgainst, GoalActor::Npc(Some(id))) => {
                    assert!((200..211).contains(&id), "opp scorer id: {id}");
                }
                other => panic!("seed {seed}: bad credit {other:?}"),
            }
        }
    }
}

/// The attribution shapes all occur: PC goals (no assist), teammate goals
/// finished off the PC's delivery (assist = PC), auto-flow team goals (no
/// assist), and opponent goals.
#[test]
fn credit_shapes_cover_pc_goal_assist_and_opponent() {
    let lib = lib();
    let (mut pc_goals, mut pc_assists, mut auto_goals, mut opp_goals) = (0u32, 0u32, 0u32, 0u32);
    for seed in 0..60u64 {
        let r = auto_play_match(&lib, setup_with_ids(), &mut GoatRng::new(seed));
        for c in &r.goal_credits {
            match (c.event, c.scorer, c.assist) {
                (ScoreEvent::GoalFor, GoalActor::Pc, None) => pc_goals += 1,
                (ScoreEvent::GoalFor, GoalActor::Npc(_), Some(GoalActor::Pc)) => pc_assists += 1,
                (ScoreEvent::GoalFor, GoalActor::Npc(_), None) => auto_goals += 1,
                (ScoreEvent::GoalAgainst, GoalActor::Npc(_), None) => opp_goals += 1,
                other => panic!("seed {seed}: unexpected credit shape {other:?}"),
            }
        }
    }
    assert!(pc_goals > 0, "PC must score some goals himself");
    assert!(pc_assists > 0, "teammates must finish some PC deliveries");
    assert!(auto_goals > 0, "auto flow must produce team goals");
    assert!(opp_goals > 0, "opponents must score some goals");
}
