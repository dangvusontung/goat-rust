//! Paired-input experiments, not a substitute for real-football validation.
//! cargo run --release -p goat-match --example calibrate_match -- 10000
use goat_core::{
    attrs::NUM_ATTRS,
    match_model::{simulate_match_with_context, MatchContext, MatchObservations, TeamLines, Venue},
    roles::{FamiliarityTier, RoleId, NUM_ROLES},
    staff::StaffMods,
    tactical::TacticalProfile,
};
use goat_fixed::Fixed;
use goat_match::{
    discipline::RefPersonality,
    sim::{
        auto_play_match_shared, auto_play_match_unified_with_options,
        observe_npc_match_unified_with_context, observe_npc_match_with_context, BeatLibrary,
        MatchSetup, UnifiedOptions,
    },
    squad::SquadSheet,
};
use goat_rng::GoatRng;
use goat_traits::PlayerTraits;

fn setup(own: TeamLines, opp: TeamLines, role: RoleId, quality: i32) -> MatchSetup {
    let profile = |lines: TeamLines| TacticalProfile {
        attack: lines.attack,
        midfield: lines.midfield,
        defense: lines.defense,
        pressing: 25,
        possession: 25,
        counter: 25,
        wing_play: 25,
    };
    let mut sheet = SquadSheet::stub(60, 42, (4, 3, 3));
    let position = match goat_core::roles::ROLE_POSITION_FAMILY[role as usize] {
        goat_core::roles::PositionFamily::Defender => 0,
        goat_core::roles::PositionFamily::Midfielder => 1,
        goat_core::roles::PositionFamily::Forward => 2,
    };
    let pc_slot = sheet
        .players
        .iter()
        .position(|p| p.position == position)
        .unwrap();
    sheet.players[pc_slot].is_pc = true;
    for player in &mut sheet.players {
        player.attrs = [Fixed::from_int(60); NUM_ATTRS];
    }
    let mut opponents = sheet.clone();
    for player in &mut opponents.players {
        player.is_pc = false;
    }
    sheet.players[pc_slot].attrs = [Fixed::from_int(quality); NUM_ATTRS];
    MatchSetup {
        player_role: role,
        player_attrs: [Fixed::from_int(quality); NUM_ATTRS],
        player_familiarity: [FamiliarityTier::Natural; NUM_ROLES],
        own_profile: profile(own),
        opp_profile: profile(opp),
        opp_name: "Controlled FC",
        form: Fixed::from_int(50),
        player_aggression: 50,
        ref_personality: RefPersonality::Balanced,
        dirty_rep: 50,
        player_traits: PlayerTraits::default(),
        staff_mods: StaffMods::NEUTRAL,
        own_squad: sheet.clone(),
        opp_squad: opponents,
        sub_context: None,
    }
}

#[derive(Default)]
struct Totals {
    stats: MatchObservations,
    wins: u64,
    draws: u64,
    losses: u64,
    pc_actions: u64,
    cards: u64,
    goals: [u64; 2],
    team_chances: [u64; 2],
    exposure: u64,
    slots: u64,
    pc_resolved_chances: u64,
    pc_finishing_attempts: u64,
    cancelled_chances: u64,
    pc_for_stakes: u64,
    pc_against_stakes: u64,
}
impl Totals {
    fn add(&mut self, goals: [u32; 2], observations: MatchObservations, actions: u64, red: bool) {
        for (i, goal) in goals.iter().enumerate() {
            self.goals[i] += *goal as u64;
            self.stats.goals[i] += observations.goals[i];
            self.stats.chances[i] += observations.chances[i];
            self.stats.attacking_minutes[i] += observations.attacking_minutes[i];
        }
        match goals[0].cmp(&goals[1]) {
            std::cmp::Ordering::Greater => self.wins += 1,
            std::cmp::Ordering::Equal => self.draws += 1,
            std::cmp::Ordering::Less => self.losses += 1,
        }
        self.pc_actions += actions;
        self.cards += u64::from(red);
    }
    fn observe_ledger(&mut self, result: &goat_match::sim::MatchResult) {
        self.team_chances[0] += result.team_observations.chances[0] as u64;
        self.team_chances[1] += result.team_observations.chances[1] as u64;
        self.exposure += result
            .team_observations
            .attacking_minutes
            .iter()
            .sum::<u32>() as u64;
        self.slots += result.opportunities.len() as u64;
        self.pc_finishing_attempts += result
            .opportunities
            .iter()
            .filter(|o| o.execution_success.is_some())
            .count() as u64;
        self.cancelled_chances += result
            .opportunities
            .iter()
            .filter(|o| {
                o.opportunity.created
                    && o.actor == goat_match::beats::GoalActor::Pc
                    && o.execution_success.is_none()
            })
            .count() as u64;
        self.pc_resolved_chances += result
            .opportunities
            .iter()
            .filter(|o| o.opportunity.created && o.actor == goat_match::beats::GoalActor::Pc)
            .count() as u64;
    }
    fn print(&self, scenario: &str, engine: &str, n: u64, seed_start: u64) {
        // Floats format measurement outputs only; no value feeds back into the sim.
        let rate = |value: u64| value as f64 / n as f64;
        println!("{scenario},{engine},{n},{seed_start},{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},{:.5}",
            rate(self.goals[0]),rate(self.goals[1]),rate(self.wins),rate(self.draws),rate(self.losses),
            rate(self.stats.chances[0] as u64),rate(self.stats.chances[1] as u64),
            rate(self.stats.goals[0] as u64),rate(self.stats.goals[1] as u64),
            rate(self.stats.attacking_minutes[0] as u64),rate(self.stats.attacking_minutes[1] as u64),
            rate(self.pc_actions),rate(self.cards),rate(self.pc_for_stakes),rate(self.pc_against_stakes),rate(self.team_chances[0]),rate(self.team_chances[1]),rate(self.exposure),rate(self.slots),rate(self.pc_resolved_chances),rate(self.pc_finishing_attempts),rate(self.cancelled_chances));
    }
}

fn main() {
    let n: u64 = std::env::args()
        .nth(1)
        .map(|s| s.parse().expect("positive match count"))
        .unwrap_or(10_000);
    assert!((1..=1_000_000).contains(&n));
    let seed_start: u64 = std::env::args()
        .nth(2)
        .map(|s| s.parse().expect("seed start"))
        .unwrap_or(0);
    let seed_end = seed_start.checked_add(n).expect("seed range overflow");
    let legacy = std::env::args().any(|arg| arg == "--v4");
    let no_ablations = std::env::args().any(|arg| arg == "--no-ablations");
    let lib = BeatLibrary::load(include_str!("../../../beats.json")).unwrap();
    let neutral = TeamLines {
        attack: 60,
        midfield: 60,
        defense: 60,
    };
    let ctx = MatchContext {
        venue: Venue::Neutral,
        ..Default::default()
    };
    let scenarios = [
        ("equal_neutral", neutral, neutral, ctx),
        (
            "equal_home",
            neutral,
            neutral,
            MatchContext {
                venue: Venue::Home,
                ..ctx
            },
        ),
        (
            "equal_away",
            neutral,
            neutral,
            MatchContext {
                venue: Venue::Away,
                ..ctx
            },
        ),
        (
            "attack_90",
            TeamLines {
                attack: 90,
                ..neutral
            },
            neutral,
            ctx,
        ),
        (
            "defense_90",
            TeamLines {
                defense: 90,
                ..neutral
            },
            neutral,
            ctx,
        ),
        (
            "midfield_90",
            TeamLines {
                midfield: 90,
                ..neutral
            },
            neutral,
            ctx,
        ),
        (
            "own_ten",
            neutral,
            neutral,
            MatchContext {
                own_players: 10,
                ..ctx
            },
        ),
        (
            "opp_ten",
            neutral,
            neutral,
            MatchContext {
                opp_players: 10,
                ..ctx
            },
        ),
    ];
    println!("scenario,engine,n,seed_start,goals_for,goals_against,win_share,draw_share,loss_share,npc_chances_for,npc_chances_against,npc_goals_for,npc_goals_against,npc_attack_minutes_for,npc_attack_minutes_against,pc_actions,red_share,pc_for_scoring_stakes,pc_against_scoring_stakes,team_chances_for,team_chances_against,team_exposure,slots,pc_resolved_chances,pc_finishing_attempts,cancelled_chances");
    for (label, own, opp, context) in scenarios {
        let mut aggregate = Totals::default();
        let mut detailed = Totals::default();
        let fixture = setup(own, opp, RoleId::CompleteForward, 60);
        for seed in seed_start..seed_end {
            let stats = simulate_match_with_context(own, opp, context, &mut GoatRng::new(seed));
            aggregate.add(stats.goals, stats, 0, false);
            let observer = if legacy {
                observe_npc_match_with_context
            } else {
                observe_npc_match_unified_with_context
            };
            let result = observer(&lib, fixture.clone(), context, &mut GoatRng::new(seed));
            detailed.observe_ledger(&result);
            detailed.add(
                [result.goals_for, result.goals_against],
                result.npc_observations,
                0,
                result.red_card,
            );
        }
        aggregate.print(label, "aggregate", n, seed_start);
        detailed.print(label, "detailed_npc", n, seed_start);
    }
    let defaults = UnifiedOptions::default();
    for (label, role, quality, options) in [
        ("pc_st_60", RoleId::CompleteForward, 60, defaults),
        ("pc_cb_60", RoleId::CentreBack, 60, defaults),
        ("pc_cm_60", RoleId::CentralMid, 60, defaults),
        ("pc_st_90", RoleId::CompleteForward, 90, defaults),
        (
            "pc_st_60_no_zone",
            RoleId::CompleteForward,
            60,
            UnifiedOptions {
                zone_pull: false,
                ..defaults
            },
        ),
        (
            "pc_st_60_no_chain",
            RoleId::CompleteForward,
            60,
            UnifiedOptions {
                chains: false,
                ..defaults
            },
        ),
        (
            "pc_st_60_first",
            RoleId::CompleteForward,
            60,
            UnifiedOptions {
                first_choice: true,
                ..defaults
            },
        ),
    ] {
        if (legacy || no_ablations) && (label.contains("no_") || label.ends_with("first")) {
            continue;
        }
        let mut total = Totals::default();
        let fixture = setup(neutral, neutral, role, quality);
        for seed in seed_start..seed_end {
            let result = if legacy {
                auto_play_match_shared(&lib, fixture.clone(), &mut GoatRng::new(seed))
            } else {
                auto_play_match_unified_with_options(
                    &lib,
                    fixture.clone(),
                    options,
                    &mut GoatRng::new(seed),
                )
            };
            total.observe_ledger(&result);
            use goat_match::beats::ScoreEvent;
            total.pc_for_stakes += result
                .moments
                .iter()
                .filter(|m| {
                    m.is_action
                        && [m.success_event, m.failure_event].iter().any(|event| {
                            matches!(event, Some(ScoreEvent::GoalFor | ScoreEvent::AssistFor))
                        })
                })
                .count() as u64;
            total.pc_against_stakes += result
                .moments
                .iter()
                .filter(|m| {
                    m.is_action
                        && [m.success_event, m.failure_event]
                            .contains(&Some(ScoreEvent::GoalAgainst))
                })
                .count() as u64;
            total.add(
                [result.goals_for, result.goals_against],
                result.npc_observations,
                result.moments.iter().filter(|m| m.is_action).count() as u64,
                result.red_card,
            );
        }
        total.print(label, "detailed_pc", n, seed_start);
    }
}
