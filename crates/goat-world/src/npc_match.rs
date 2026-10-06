//! Versioned minute-exposed NPC football. Legacy deep fixtures remain unchanged.
use goat_core::{
    match_model::{available_lines, possession_per_1000, sample_attack, AttackContext, TeamLines},
    tactical::TacticalProfile,
};
use goat_rng::RngSource;

#[derive(Clone, Debug)]
pub struct Candidate {
    pub player: u32,
    /// Outfield group: 0 defender, 1 midfielder, 2 forward.
    pub position: u8,
    pub keeper: bool,
    pub rating: u8,
    pub keeping: u8,
    pub energy: u8,
    pub stamina: u8,
    pub aggression: u8,
    pub returning: bool,
    pub banned: bool,
}
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Appearance {
    pub player: u32,
    pub minutes: u16,
    pub goals: u8,
    pub assists: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Card {
    pub player: u32,
    pub minute: u8,
    pub kind: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Substitution {
    pub off: u32,
    pub on: u32,
    pub minute: u8,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchDetail {
    pub goals: [u32; 2],
    pub appearances: [Vec<Appearance>; 2],
    pub cards: Vec<Card>,
    pub substitutions: [Vec<Substitution>; 2],
    /// Starting keeper identities; appearances include any replacements.
    pub keeper: [Option<u32>; 2],
    /// Whether each side kicked off with an emergency keeper.
    pub emergency_keeper: [bool; 2],
    pub chasing_intervals: [u8; 2],
    pub protecting_intervals: [u8; 2],
}
#[derive(Clone, Copy)]
/// Per-team card probabilities in each five-minute interval.
pub struct MatchRules {
    pub yellow_per_1000: u32,
    pub red_per_1000: u32,
}
impl Default for MatchRules {
    fn default() -> Self {
        Self {
            yellow_per_1000: 80,
            red_per_1000: 1,
        }
    }
}

struct Team<'a> {
    candidates: &'a [Candidate],
    profile: TacticalProfile,
    active: Vec<usize>,
    used: Vec<bool>,
    yellows: Vec<u8>,
    appearances: Vec<Appearance>,
    substitutions: Vec<Substitution>,
    windows: Vec<u8>,
    keeper: Option<usize>,
    emergency: bool,
}
impl<'a> Team<'a> {
    fn new(candidates: &'a [Candidate], profile: TacticalProfile) -> Self {
        let mut ranked: Vec<_> = (0..candidates.len())
            .filter(|&i| !candidates[i].banned)
            .collect();
        ranked.sort_by_key(|&i| {
            (
                std::cmp::Reverse(
                    candidates[i].rating as i32 * 70 + candidates[i].energy as i32 * 30
                        - i32::from(candidates[i].returning) * 2500,
                ),
                candidates[i].player,
            )
        });
        let keeper = ranked
            .iter()
            .copied()
            .filter(|&i| candidates[i].keeper)
            .max_by_key(|&i| {
                (
                    candidates[i].keeping as i32 * 80 + candidates[i].energy as i32 * 20
                        - i32::from(candidates[i].returning) * 2500,
                    std::cmp::Reverse(candidates[i].player),
                )
            })
            .or_else(|| {
                ranked.iter().copied().max_by_key(|&i| {
                    (
                        candidates[i].keeping,
                        std::cmp::Reverse(candidates[i].player),
                    )
                })
            });
        let mut active: Vec<_> = keeper.into_iter().collect();
        let (defenders, midfielders, forwards) = profile.formation_slots();
        for (position, count) in [(0, defenders), (1, midfielders), (2, forwards)] {
            active.extend(
                ranked
                    .iter()
                    .copied()
                    .filter(|&i| {
                        Some(i) != keeper
                            && !candidates[i].keeper
                            && candidates[i].position == position
                    })
                    .take(count),
            );
        }
        for i in ranked {
            if active.len() == 11 {
                break;
            }
            if !active.contains(&i) {
                active.push(i);
            }
        }
        let mut used = vec![false; candidates.len()];
        for &i in &active {
            used[i] = true;
        }
        Self {
            candidates,
            profile,
            emergency: keeper.is_some_and(|i| !candidates[i].keeper),
            active,
            used,
            yellows: vec![0; candidates.len()],
            appearances: candidates
                .iter()
                .map(|c| Appearance {
                    player: c.player,
                    ..Default::default()
                })
                .collect(),
            substitutions: Vec::new(),
            windows: Vec::new(),
            keeper,
        }
    }
    fn energy(&self, i: usize) -> i32 {
        let player = &self.candidates[i];
        (player.energy as i32
            - self.appearances[i].minutes as i32
                * (130 - player.stamina as i32 + self.profile.pressing as i32 / 5)
                / 270)
            .max(0)
    }
    fn lines(&self, score: i32, minute: u8) -> TeamLines {
        let mean = |position| {
            let group: Vec<_> = self
                .active
                .iter()
                .copied()
                .filter(|&i| Some(i) != self.keeper && self.candidates[i].position == position)
                .collect();
            if group.is_empty() {
                return 1u32;
            }
            group
                .iter()
                .map(|&i| {
                    self.candidates[i].rating as u32 * (700 + self.energy(i) as u32 * 3) / 1000
                })
                .sum::<u32>()
                / match position {
                    0 => self.profile.formation_slots().0,
                    1 => self.profile.formation_slots().1,
                    _ => self.profile.formation_slots().2,
                }
                .max(1) as u32
        };
        let mut attack = mean(2);
        let mut midfield = mean(1);
        let keeping = self.keeper.map_or(1, |i| {
            self.candidates[i].keeping as u32 * if self.emergency { 600 } else { 1000 } / 1000
        });
        let mut defense = (mean(0) * 4 + keeping) / 5;
        if minute >= 55 && score < 0 {
            attack = attack * 110 / 100;
            midfield = midfield * 103 / 100;
            defense = defense * 90 / 100;
        }
        if minute >= 65 && score > 0 {
            attack = attack * 92 / 100;
            defense = defense * 108 / 100;
        }
        available_lines(
            TeamLines {
                attack: attack.clamp(1, 99) as u8,
                midfield: midfield.clamp(1, 99) as u8,
                defense: defense.clamp(1, 99) as u8,
            },
            self.active.len() as u8,
        )
    }
    fn swap(&mut self, off: usize, on: usize, minute: u8) -> bool {
        let permitted = minute == 45 || self.windows.contains(&minute) || self.windows.len() < 3;
        if !permitted
            || self.substitutions.len() >= 5
            || self.used[on]
            || self.candidates[on].banned
        {
            return false;
        }
        let Some(slot) = self.active.iter().position(|&i| i == off) else {
            return false;
        };
        self.active[slot] = on;
        self.used[on] = true;
        if self.keeper == Some(off) {
            self.keeper = Some(on);
            self.emergency = !self.candidates[on].keeper;
        }
        if minute != 45 && !self.windows.contains(&minute) {
            self.windows.push(minute);
        }
        self.substitutions.push(Substitution {
            off: self.candidates[off].player,
            on: self.candidates[on].player,
            minute,
        });
        true
    }
    fn substitute(&mut self, minute: u8, deficit: i32) {
        let forced_return = self
            .active
            .iter()
            .any(|&i| self.candidates[i].returning && self.appearances[i].minutes >= 30);
        if !forced_return && ![45, 60, 75, 85].contains(&minute) {
            return;
        }
        let mut off = self.active.clone();
        off.sort_by_key(|&i| {
            (
                std::cmp::Reverse(
                    self.candidates[i].returning && self.appearances[i].minutes >= 30,
                ),
                self.energy(i),
                self.candidates[i].player,
            )
        });
        let budget = if forced_return {
            5
        } else if minute == 60 || minute == 75 {
            2
        } else {
            1
        };
        let mut replaced = 0;
        for i in off {
            let returning = self.candidates[i].returning && self.appearances[i].minutes >= 30;
            if !returning
                && (self.keeper == Some(i) || minute == 45 && self.energy(i) >= 50 || minute < 60)
            {
                continue;
            }
            if !returning && minute >= 60 && self.energy(i) > 75 && !(minute >= 75 && deficit != 0)
            {
                continue;
            }
            let keeper = self.keeper == Some(i);
            let target = if deficit > 0 && minute >= 75 && !keeper {
                2
            } else if deficit < 0 && minute >= 75 && !keeper {
                0
            } else {
                self.candidates[i].position
            };
            let on = (0..self.candidates.len())
                .filter(|&j| {
                    !self.used[j]
                        && !self.candidates[j].banned
                        && self.candidates[j].keeper == keeper
                })
                .max_by_key(|&j| {
                    (
                        i32::from(self.candidates[j].position == target) * 1500
                            + self.candidates[j].rating as i32 * 70
                            + self.candidates[j].energy as i32 * 30
                            - i32::from(self.candidates[j].returning && minute < 60) * 2500,
                        std::cmp::Reverse(self.candidates[j].player),
                    )
                });
            if let Some(on) = on {
                if self.swap(i, on, minute) {
                    replaced += 1;
                }
            }
            if replaced >= budget {
                break;
            }
        }
    }
    fn dismiss(&mut self, i: usize, minute: u8) {
        self.active.retain(|&j| j != i);
        if self.keeper == Some(i) {
            self.keeper = None;
            let backup = (0..self.candidates.len()).find(|&j| {
                self.candidates[j].keeper && !self.used[j] && !self.candidates[j].banned
            });
            if let (Some(on), Some(off)) = (backup, self.active.last().copied()) {
                if self.swap(off, on, minute) {
                    self.keeper = Some(on);
                    self.emergency = false;
                }
            }
            if self.keeper.is_none() {
                self.keeper = self.active.iter().copied().max_by_key(|&j| {
                    (
                        self.candidates[j].keeping,
                        std::cmp::Reverse(self.candidates[j].player),
                    )
                });
                self.emergency = true;
            }
        }
    }
    fn weighted(&self, weights: impl Fn(usize) -> u32, rng: &mut impl RngSource) -> Option<usize> {
        let total: u32 = self.active.iter().map(|&i| weights(i)).sum();
        if total == 0 {
            return self.active.first().copied();
        }
        let mut roll = rng.next_range_u32(0, total - 1);
        for &i in &self.active {
            let weight = weights(i);
            if roll < weight {
                return Some(i);
            }
            roll -= weight;
        }
        None
    }
}

pub fn simulate(
    candidates: [&[Candidate]; 2],
    profiles: [TacticalProfile; 2],
    rules: MatchRules,
    rng: &mut impl RngSource,
) -> MatchDetail {
    let mut teams = [
        Team::new(candidates[0], profiles[0]),
        Team::new(candidates[1], profiles[1]),
    ];
    let initial_keepers = teams
        .each_ref()
        .map(|team| team.keeper.map(|i| team.candidates[i].player));
    let emergency = [teams[0].emergency, teams[1].emergency];
    let mut goals = [0u32; 2];
    let mut cards = Vec::new();
    let mut chasing = [0; 2];
    let mut protecting = [0; 2];
    for minute in (0..90u8).step_by(5) {
        for side in 0..2 {
            let deficit = goals[1 - side] as i32 - goals[side] as i32;
            teams[side].substitute(minute, deficit);
            if minute >= 55 && deficit > 0 {
                chasing[side] += 1;
            }
            if minute >= 65 && deficit < 0 {
                protecting[side] += 1;
            }
        }
        let lines = [
            teams[0].lines(goals[0] as i32 - goals[1] as i32, minute),
            teams[1].lines(goals[1] as i32 - goals[0] as i32, minute),
        ];
        let side =
            usize::from(rng.next_range_u32(0, 999) >= possession_per_1000(lines[0], lines[1]));
        let (_, goal) = sample_attack(
            lines[side],
            lines[1 - side],
            AttackContext {
                minutes: 5,
                home: side == 0,
                players: teams[side].active.len() as u8,
            },
            rng,
        );
        if goal {
            let team = &mut teams[side];
            if let Some(scorer) = team.weighted(
                |i| {
                    if team.keeper == Some(i) {
                        0
                    } else {
                        [100, 300, 600][team.candidates[i].position as usize]
                            * team.candidates[i].rating as u32
                    }
                },
                rng,
            ) {
                goals[side] += 1;
                team.appearances[scorer].goals += 1;
                if rng.next_range_u32(0, 99) < 75 {
                    if let Some(assist) = team.weighted(
                        |i| {
                            if i == scorer || team.keeper == Some(i) {
                                0
                            } else {
                                team.candidates[i].rating as u32
                            }
                        },
                        rng,
                    ) {
                        if assist != scorer {
                            team.appearances[assist].assists += 1;
                        }
                    }
                }
            }
        }
        for team in &mut teams {
            for &i in &team.active {
                team.appearances[i].minutes += 5;
            }
            let foul = rng.next_range_u32(0, 999);
            if foul >= rules.yellow_per_1000 + rules.red_per_1000 {
                continue;
            }
            if let Some(i) = team.weighted(
                |i| {
                    if team.keeper == Some(i) {
                        1
                    } else {
                        team.candidates[i].aggression as u32 + 100 - team.energy(i) as u32
                    }
                },
                rng,
            ) {
                let kind = if foul < rules.red_per_1000 {
                    2
                } else if team.yellows[i] > 0 {
                    1
                } else {
                    0
                };
                cards.push(Card {
                    player: team.candidates[i].player,
                    minute: minute + 5,
                    kind,
                });
                if kind == 0 {
                    team.yellows[i] += 1;
                } else {
                    team.dismiss(i, minute + 5);
                }
            }
        }
    }
    MatchDetail {
        goals,
        appearances: teams.each_ref().map(|t| {
            t.appearances
                .iter()
                .filter(|a| a.minutes > 0)
                .cloned()
                .collect()
        }),
        cards,
        substitutions: [
            teams[0].substitutions.clone(),
            teams[1].substitutions.clone(),
        ],
        keeper: initial_keepers,
        emergency_keeper: emergency,
        chasing_intervals: chasing,
        protecting_intervals: protecting,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use goat_rng::GoatRng;
    fn squad(offset: u32, rating: u8) -> Vec<Candidate> {
        (0..22)
            .map(|i| Candidate {
                player: offset + i,
                position: (i % 3) as u8,
                keeper: i >= 20,
                rating,
                keeping: rating,
                energy: 90,
                stamina: 70,
                aggression: 50,
                returning: false,
                banned: false,
            })
            .collect()
    }
    fn profiles() -> [TacticalProfile; 2] {
        [
            TacticalProfile::derive(70, 0, 42),
            TacticalProfile::derive(70, 1, 42),
        ]
    }
    #[test]
    fn minutes_goal_credits_sub_limits_and_keeper_identity_are_conserved() {
        let home = squad(0, 70);
        let away = squad(100, 70);
        for seed in 0..300 {
            let detail = simulate(
                [&home, &away],
                profiles(),
                MatchRules::default(),
                &mut GoatRng::new(seed),
            );
            assert_eq!(
                detail,
                simulate(
                    [&home, &away],
                    profiles(),
                    MatchRules::default(),
                    &mut GoatRng::new(seed)
                )
            );
            for side in 0..2 {
                let rows = &detail.appearances[side];
                let missed: u32 = detail
                    .cards
                    .iter()
                    .filter(|c| c.kind > 0 && (c.player >= 100) == (side == 1))
                    .map(|c| 90 - c.minute as u32)
                    .sum();
                assert_eq!(
                    rows.iter().map(|a| a.minutes as u32).sum::<u32>(),
                    990 - missed
                );
                assert_eq!(
                    rows.iter().map(|a| a.goals as u32).sum::<u32>(),
                    detail.goals[side]
                );
                assert!(rows.iter().all(|a| a.minutes <= 90));
                let subs = &detail.substitutions[side];
                assert!(subs.len() <= 5);
                assert!(
                    subs.iter()
                        .filter(|s| s.minute != 45)
                        .map(|s| s.minute)
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        <= 3
                );
                let keeper = detail.keeper[side].unwrap();
                assert!(keeper % 100 >= 20);
                assert!(!detail.emergency_keeper[side]);
                for card in detail
                    .cards
                    .iter()
                    .filter(|c| c.kind > 0 && (c.player >= 100) == (side == 1))
                {
                    let row = rows.iter().find(|a| a.player == card.player).unwrap();
                    assert!(row.minutes <= card.minute as u16);
                }
            }
        }
    }
    #[test]
    fn banned_players_are_dnp_and_returning_players_get_shorter_load() {
        let mut home = squad(0, 70);
        let away = squad(100, 70);
        home[0].banned = true;
        home[1].returning = true;
        home[1].rating = 99;
        let detail = simulate(
            [&home, &away],
            profiles(),
            MatchRules {
                yellow_per_1000: 0,
                red_per_1000: 0,
            },
            &mut GoatRng::new(42),
        );
        assert!(!detail.appearances[0].iter().any(|a| a.player == 0));
        assert!(detail.appearances[0]
            .iter()
            .find(|a| a.player == 1)
            .is_none_or(|a| a.minutes <= 30));
    }
    #[test]
    fn dismissing_a_keeper_replaces_an_outfielder_or_uses_an_emergency_keeper() {
        let home = squad(0, 70);
        let profile = profiles()[0];
        let mut team = Team::new(&home, profile);
        let keeper = team.keeper.unwrap();
        team.dismiss(keeper, 35);
        assert_eq!(team.active.len(), 10);
        assert!(team.keeper.is_some());
        assert!(!team.emergency);
        assert_eq!(team.substitutions[0].minute, 35);
        assert!(team.candidates[team.keeper.unwrap()].keeper);
        let mut no_backup = home.clone();
        no_backup[21].banned = true;
        let mut team = Team::new(&no_backup, profile);
        let keeper = team.keeper.unwrap();
        team.dismiss(keeper, 35);
        assert_eq!(team.active.len(), 10);
        assert!(team.emergency);
        assert!(team.keeper.is_some());
    }
    #[test]
    fn score_changes_tactics_and_late_substitution_role() {
        let home = squad(0, 70);
        let mut team = Team::new(&home, profiles()[0]);
        let chasing = team.lines(-1, 75);
        let protecting = team.lines(1, 75);
        assert!(chasing.attack > protecting.attack);
        assert!(chasing.defense < protecting.defense);
        team.substitute(75, 1);
        assert!(!team.substitutions.is_empty());
        let on = team.substitutions[0].on;
        assert_eq!(
            team.candidates
                .iter()
                .find(|p| p.player == on)
                .unwrap()
                .position,
            2
        );
    }
    #[test]
    fn fresh_high_stamina_players_are_not_replaced_without_a_tactical_reason() {
        let mut home = squad(0, 70);
        for player in &mut home {
            player.energy = 99;
            player.stamina = 99;
        }
        let mut team = Team::new(&home, profiles()[0]);
        team.substitute(60, 0);
        assert!(team.substitutions.is_empty());
        team.substitute(75, 1);
        assert!(!team.substitutions.is_empty());
    }
    #[test]
    fn stronger_teams_and_better_keepers_change_outcomes_without_a_free_goal() {
        let strong = squad(0, 85);
        let weak = squad(100, 40);
        let mut totals = [0u32; 2];
        let rules = MatchRules {
            yellow_per_1000: 0,
            red_per_1000: 0,
        };
        for seed in 0..1500 {
            let result = simulate([&strong, &weak], profiles(), rules, &mut GoatRng::new(seed));
            totals[0] += result.goals[0];
            totals[1] += result.goals[1];
        }
        assert!(totals[0] > totals[1] * 2);
        let mut poor_keeper = weak.clone();
        for c in &mut poor_keeper {
            if c.keeper {
                c.keeping = 1;
            }
        }
        let mut conceded = [0; 2];
        for seed in 0..1000 {
            conceded[0] +=
                simulate([&strong, &weak], profiles(), rules, &mut GoatRng::new(seed)).goals[0];
            conceded[1] += simulate(
                [&strong, &poor_keeper],
                profiles(),
                rules,
                &mut GoatRng::new(seed),
            )
            .goals[0];
        }
        assert!(conceded[1] > conceded[0]);
    }
}
