//! Shared chance creation/conversion for detailed and aggregate matches.
//! All probabilities are per thousand; time exposure is explicit.
use goat_rng::RngSource;

/// A team's effective lines, after selection, tactics and availability.
#[derive(Clone, Copy, Debug)]
pub struct TeamLines {
    pub attack: u8,
    pub midfield: u8,
    pub defense: u8,
}

/// Match context shared by both resolutions; a score deficit is not a free goal.
#[derive(Clone, Copy, Debug)]
pub struct AttackContext {
    pub minutes: u32,
    pub home: bool,
    pub players: u8,
}

/// Nominal opportunities per five minutes of attacking possession.
pub const CHANCE_CREATION_PER_1000: u32 = 620;
/// Conversion of an equally matched opportunity (about one in four).
pub const CONVERSION_SCALE: u32 = 500;
/// Home advantage applies to creation, not an unconditional score bonus.
pub const HOME_CREATION_BONUS: u32 = 25;

/// Midfield controls opportunity share; swapping teams complements the probability.
pub fn possession_per_1000(own: TeamLines, opp: TeamLines) -> u32 {
    (500 + (own.midfield as i32 - opp.midfield as i32) * 5).clamp(100, 900) as u32
}

/// Separate creation and conversion makes missed opportunities explicit and permits
/// aggregate resolution without authored text or a fixed maximum of five goals.
pub fn sample_attack(
    own: TeamLines,
    opp: TeamLines,
    context: AttackContext,
    rng: &mut impl RngSource,
) -> (bool, bool) {
    let opportunity = allocate_opportunity(0, context, rng);
    let goal = opportunity.created
        && opportunity.conversion_roll
            < conversion_per_1000(own.attack, opp.defense, opportunity.quality);
    (opportunity.created, goal)
}

/// Conversion law used by automatic finishing and individual decision contests.
/// Quality represents the semantic opening/pressure, with 1.0 as the baseline.
pub fn conversion_per_1000(attack: u8, defense: u8, quality: goat_fixed::Fixed) -> u32 {
    let base = CONVERSION_SCALE * attack as u32 / (attack as u32 + defense as u32 + 1);
    (goat_fixed::Fixed::from_int(base as i32)
        * quality.clamp(goat_fixed::Fixed::ZERO, goat_fixed::Fixed::from_int(2)))
    .to_int()
    .clamp(0, 999) as u32
}

/// One team's opportunity for an elapsed interval. Reserve the conversion draw
/// once; observing or chaining decisions must not create another goal attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TeamOpportunity {
    pub side: u8,
    pub minutes: u32,
    pub players: u8,
    pub quality: goat_fixed::Fixed,
    pub created: bool,
    pub conversion_roll: u32,
}

/// Allocate before deciding whose action the camera follows. Draws match the
/// two-stage kernel, including when no opportunity is created.
pub fn allocate_opportunity(
    side: u8,
    context: AttackContext,
    rng: &mut impl RngSource,
) -> TeamOpportunity {
    let creation = (CHANCE_CREATION_PER_1000 + if context.home { HOME_CREATION_BONUS } else { 0 })
        * context.minutes.min(90)
        / 5
        * context.players.min(11) as u32
        / 11;
    TeamOpportunity {
        side,
        minutes: context.minutes,
        players: context.players.min(11),
        quality: goat_fixed::Fixed::ONE,
        created: rng.next_range_u64(0, 999) < creation.min(1000) as u64,
        conversion_roll: rng.next_range_u32(0, 999),
    }
}

/// Venue is relative to the first team; neutral fixtures give neither side a bonus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Venue {
    Home,
    Away,
    Neutral,
}

/// Explicit initial availability for aggregate matches and controlled experiments.
#[derive(Clone, Copy, Debug)]
pub struct MatchContext {
    pub venue: Venue,
    pub own_players: u8,
    pub opp_players: u8,
}

impl Default for MatchContext {
    fn default() -> Self {
        Self {
            venue: Venue::Home,
            own_players: 11,
            opp_players: 11,
        }
    }
}

/// Numeric observations are derived outputs, never simulation inputs or save fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MatchObservations {
    pub goals: [u32; 2],
    pub chances: [u32; 2],
    pub attacking_minutes: [u32; 2],
}

/// Short-handed sides lose midfield and defensive coverage; chance creation handles
/// attacking availability separately, avoiding a double penalty to attack strength.
pub fn available_lines(mut lines: TeamLines, players: u8) -> TeamLines {
    lines.midfield = (lines.midfield as u32 * players.min(11) as u32 / 11) as u8;
    lines.defense = (lines.defense as u32 * players.min(11) as u32 / 11) as u8;
    lines
}

/// Resolve a complete NPC match using the same attack kernel as detailed matches.
/// This wrapper preserves the frozen default home/away stream and score.
pub fn simulate_match(home: TeamLines, away: TeamLines, rng: &mut impl RngSource) -> (u32, u32) {
    let result = simulate_match_with_context(home, away, MatchContext::default(), rng);
    (result.goals[0], result.goals[1])
}

/// Context-aware aggregate resolution exposes observations for causal calibration.
pub fn simulate_match_with_context(
    own: TeamLines,
    opp: TeamLines,
    context: MatchContext,
    rng: &mut impl RngSource,
) -> MatchObservations {
    let own = available_lines(own, context.own_players);
    let opp = available_lines(opp, context.opp_players);
    let mut result = MatchObservations::default();
    for _ in 0..18 {
        let own_ball = rng.next_range_u64(0, 999) < possession_per_1000(own, opp) as u64;
        let side = usize::from(!own_ball);
        let (attacking, defending, players, home) = if own_ball {
            (own, opp, context.own_players, context.venue == Venue::Home)
        } else {
            (opp, own, context.opp_players, context.venue == Venue::Away)
        };
        let (chance, goal) = sample_attack(
            attacking,
            defending,
            AttackContext {
                minutes: 5,
                home,
                players,
            },
            rng,
        );
        result.attacking_minutes[side] += 5;
        result.chances[side] += u32::from(chance);
        result.goals[side] += u32::from(goal);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use goat_rng::GoatRng;
    #[test]
    fn allocated_attempt_uses_the_existing_creation_conversion_law() {
        let lines = TeamLines {
            attack: 60,
            midfield: 60,
            defense: 60,
        };
        for seed in 0..1000 {
            let context = AttackContext {
                minutes: 5,
                home: false,
                players: 11,
            };
            let direct = sample_attack(lines, lines, context, &mut GoatRng::new(seed));
            let allocated = allocate_opportunity(0, context, &mut GoatRng::new(seed));
            assert_eq!(
                direct,
                (
                    allocated.created,
                    allocated.created
                        && allocated.conversion_roll
                            < conversion_per_1000(60, 60, allocated.quality)
                )
            );
        }
    }

    #[test]
    fn venue_and_manpower_change_opportunities_in_the_expected_direction() {
        let lines = TeamLines {
            attack: 60,
            midfield: 60,
            defense: 60,
        };
        let neutral = MatchContext {
            venue: Venue::Neutral,
            ..Default::default()
        };
        let mut totals = [[0u64; 4]; 3];
        for seed in 0..20_000 {
            for (i, ctx) in [
                neutral,
                MatchContext {
                    venue: Venue::Home,
                    ..neutral
                },
                MatchContext {
                    own_players: 10,
                    ..neutral
                },
            ]
            .into_iter()
            .enumerate()
            {
                let r = simulate_match_with_context(lines, lines, ctx, &mut GoatRng::new(seed));
                assert_eq!(r.attacking_minutes.iter().sum::<u32>(), 90);
                for side in 0..2 {
                    assert!(r.goals[side] <= r.chances[side]);
                }
                totals[i][0] += r.chances[0] as u64;
                totals[i][1] += r.goals[0] as u64;
                totals[i][2] += r.chances[1] as u64;
                totals[i][3] += r.goals[1] as u64;
            }
        }
        assert!(totals[1][0] > totals[0][0]);
        assert!(totals[1][1] > totals[0][1]);
        assert!(totals[2][0] < totals[0][0]);
        assert!(totals[2][1] < totals[0][1]);
        assert!(totals[2][3] > totals[0][3]);
    }

    #[test]
    fn shared_seed_42_score_is_frozen() {
        let lines = TeamLines {
            attack: 75,
            midfield: 75,
            defense: 75,
        };
        assert_eq!(simulate_match(lines, lines, &mut GoatRng::new(42)), (1, 0));
    }

    #[test]
    fn stronger_attack_and_missing_players_have_causal_effects() {
        let neutral = TeamLines {
            attack: 60,
            midfield: 60,
            defense: 60,
        };
        let strong = TeamLines {
            attack: 95,
            ..neutral
        };
        let mut goals = [0; 3];
        for seed in 0..20_000 {
            for (i, (team, players)) in [(neutral, 11), (strong, 11), (neutral, 10)]
                .into_iter()
                .enumerate()
            {
                let (_, goal) = sample_attack(
                    team,
                    neutral,
                    AttackContext {
                        minutes: 5,
                        home: false,
                        players,
                    },
                    &mut GoatRng::new(seed),
                );
                goals[i] += u32::from(goal);
            }
        }
        assert!(goals[1] > goals[0]);
        assert!(goals[2] < goals[0]);
    }
    #[test]
    fn complete_matches_are_replayable_and_not_capped_at_five() {
        let lines = TeamLines {
            attack: 99,
            midfield: 50,
            defense: 1,
        };
        assert_eq!(
            simulate_match(lines, lines, &mut GoatRng::new(42)),
            simulate_match(lines, lines, &mut GoatRng::new(42))
        );
        assert!((0..10_000).any(|seed| {
            let (a, b) = simulate_match(lines, lines, &mut GoatRng::new(seed));
            a > 5 || b > 5
        }));
    }
}
