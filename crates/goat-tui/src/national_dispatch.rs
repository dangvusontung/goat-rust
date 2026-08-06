//! Minimal live dispatcher for the PC's national-team tournament cycle (Design round 4,
//! Slice 5 follow-up — wiring `goat_world::national_tournament`'s World Cup /
//! continental-championship engine, built in Slice 4, into the live TUI the same way
//! `cup_dispatch.rs`/`continental_dispatch.rs` wired the domestic cup and continental
//! tiers for Slice 5's playable gate).
//!
//! Deliberate cadence simplification (TASK-TUNE, same status as every other
//! dispatcher's calendar placement): the real engine's qualifying campaign is spec'd as
//! a 5-round single round-robin spread over 3 international-break windows across the 3
//! seasons before a tournament (`national_tournament.rs`'s own doc comment). Spreading
//! it that way here would mean tracking a qualifying campaign *across* season
//! boundaries, which risks colliding with the OTHER tournament cadence running 2
//! seasons out of phase (a World-Cup cycle's qualifying window can land on a
//! continental-championship *tournament* season, and vice versa — `is_tournament_season`
//! in `calendar_loop.rs` fires on every odd season, alternating type). To avoid tracking
//! two concurrent cross-season campaigns, this dispatcher instead resolves an entire
//! tournament cycle — qualifying, group stage, knockout — within the ONE tournament
//! season itself: all 5 qualifying rounds play out at that season's own
//! `InternationalBreak` flashpoint (day 30), and (if the PC's nation reaches the
//! tournament) the group stage + knockout play out at that same season's `OffSeason`
//! flashpoint (day 300) — chronologically later in the same season, so the "qualify
//! first, then play the tournament" shape still holds.
//!
//! Group/knockout opponent identity is drawn from the PC's own salted stream rather
//! than the real 8-finalist field (which would need the other 3 qualifying groups
//! fully simulated) — same reconciliation-avoidance tradeoff `continental_dispatch.rs`
//! documents, for the same reason. The PC's own qualifying-group *membership* IS real
//! (`qualifying_group_partition`), since that's a free, already-deterministic lookup.

use goat_rng::{GoatRng, RngSource};
use goat_world::national_tournament::{
    is_continental_championship_season, is_world_cup_season, national_team_strength,
    qualifying_group_partition, round_robin_schedule, NationStanding,
};
use goat_world::population::Population;
use goat_world::world::{NationId, WorldGenesis, NUM_NATIONS};

/// Which of the two national-team tournaments a cycle is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TournamentKind {
    WorldCup,
    ContinentalChampionship,
}

impl TournamentKind {
    /// The tournament (if any) `season` is the tournament season for (bible §4.1:
    /// `season % 4 == 1` is a World Cup year, `== 3` a continental-championship year —
    /// the two can never coincide).
    pub fn for_season(season: u32) -> Option<Self> {
        if is_world_cup_season(season) {
            Some(TournamentKind::WorldCup)
        } else if is_continental_championship_season(season) {
            Some(TournamentKind::ContinentalChampionship)
        } else {
            None
        }
    }

    pub fn is_world_cup(self) -> bool {
        matches!(self, TournamentKind::WorldCup)
    }

    pub fn label(self) -> &'static str {
        match self {
            TournamentKind::WorldCup => "World Cup",
            TournamentKind::ContinentalChampionship => "Continental Championship",
        }
    }
}

fn record_national_result(standing: &mut NationStanding, gf: u32, ga: u32) {
    standing.gf += gf;
    standing.ga += ga;
    if gf > ga {
        standing.w += 1;
    } else if gf == ga {
        standing.d += 1;
    } else {
        standing.l += 1;
    }
}

/// Elapsed-weeks anchor the real engine's own `national_team_strength` calls use
/// (mirrors `simulate_qualifying_cycle`/`simulate_tournament_group_stage`'s internal
/// convention exactly, so background-resolved fixtures use the same population
/// snapshot age the PC's own live fixtures do).
pub fn elapsed_weeks_for(tournament_season: u32) -> u32 {
    tournament_season.saturating_sub(1) * 52
}

/// The PC's real qualifying group (`qualifying_group_partition`) and its live-tracked
/// 5-round single round-robin campaign.
pub struct QualifyingCampaign {
    pub tournament_season: u32,
    /// Real group membership (5 nations, including `pc_nation`).
    pub group: Vec<NationId>,
    pub pc_local_idx: usize,
    /// Standings, index-aligned with `group`.
    pub standings: Vec<NationStanding>,
    /// Next round to play, 0..5.
    pub round: usize,
    /// This round's PC opponent (local index), set by `play_round`, consumed by
    /// `record_pc_result`.
    current_opponent_local: Option<usize>,
}

impl QualifyingCampaign {
    pub fn start(world_seed: u64, tournament_season: u32, pc_nation: NationId) -> Self {
        let groups = qualifying_group_partition(world_seed, tournament_season);
        let (group, pc_local_idx) = groups
            .into_iter()
            .find_map(|g| g.iter().position(|&n| n == pc_nation).map(|i| (g, i)))
            .expect("pc_nation must be in exactly one qualifying group");
        let standings = group
            .iter()
            .map(|&nation| NationStanding {
                nation,
                w: 0,
                d: 0,
                l: 0,
                gf: 0,
                ga: 0,
            })
            .collect();
        QualifyingCampaign {
            tournament_season,
            group,
            pc_local_idx,
            standings,
            round: 0,
            current_opponent_local: None,
        }
    }

    pub fn total_rounds(&self) -> usize {
        round_robin_schedule(self.group.len()).len()
    }

    pub fn is_complete(&self) -> bool {
        self.round >= self.total_rounds()
    }

    /// Resolve this round's fixtures except the PC's own (background nations only, on
    /// their own salted stream), and return the PC's opponent nation this round
    /// (`None` on the PC's bye round).
    pub fn play_round(
        &mut self,
        world: &WorldGenesis,
        pop: &Population,
        seed: u64,
    ) -> Option<NationId> {
        let elapsed_weeks = elapsed_weeks_for(self.tournament_season);
        let schedule = round_robin_schedule(self.group.len());
        let this_round = schedule[self.round].clone();
        self.current_opponent_local = None;
        for (a, b) in this_round.pairs {
            if a == self.pc_local_idx || b == self.pc_local_idx {
                self.current_opponent_local = Some(if a == self.pc_local_idx { b } else { a });
                continue;
            }
            let mut rng =
                GoatRng::new(seed ^ ((a as u64) << 8) ^ (b as u64) ^ 0x0000_0000_0000_F00D);
            let nation_a = self.group[a];
            let nation_b = self.group[b];
            let str_a = national_team_strength(
                pop,
                world.nations[nation_a].stature,
                nation_a,
                elapsed_weeks,
            );
            let str_b = national_team_strength(
                pop,
                world.nations[nation_b].stature,
                nation_b,
                elapsed_weeks,
            );
            let (ga, gb) = goat_world::sim_team_match(str_a, str_b, &mut rng);
            record_national_result(&mut self.standings[a], ga, gb);
            record_national_result(&mut self.standings[b], gb, ga);
        }
        self.current_opponent_local.map(|i| self.group[i])
    }

    /// Record the PC's own result for the round just played via `play_round`.
    pub fn record_pc_result(&mut self, gf: u32, ga: u32) {
        let opp_local = match self.current_opponent_local {
            Some(i) => i,
            None => return,
        };
        record_national_result(&mut self.standings[self.pc_local_idx], gf, ga);
        record_national_result(&mut self.standings[opp_local], ga, gf);
    }

    pub fn advance_round(&mut self) {
        self.round += 1;
        self.current_opponent_local = None;
    }

    /// True if the PC's nation finishes top-2 of its qualifying group (seeded
    /// coin-flip tiebreak on a genuine tie, own salted stream — never shares
    /// `national_tournament.rs`'s own tiebreak stream).
    pub fn pc_qualifies(&self, tiebreak_seed: u64) -> bool {
        let mut rng = GoatRng::new(tiebreak_seed);
        let keys: Vec<u64> = self.standings.iter().map(|_| rng.next_u64()).collect();
        let mut order: Vec<usize> = (0..self.standings.len()).collect();
        order.sort_by(|&x, &y| {
            self.standings[y]
                .points()
                .cmp(&self.standings[x].points())
                .then(
                    self.standings[y]
                        .goal_diff()
                        .cmp(&self.standings[x].goal_diff()),
                )
                .then(self.standings[y].gf.cmp(&self.standings[x].gf))
                .then(keys[y].cmp(&keys[x]))
        });
        order.iter().take(2).any(|&i| i == self.pc_local_idx)
    }
}

/// One phase of the PC's tournament-proper run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TournamentPhase {
    /// 0, 1 or 2 — the group stage's 3 single round-robin matchdays.
    GroupMatchday(u8),
    Semifinal,
    Final,
    Done,
}

/// Session-local (never persisted) tracker for the PC's tournament-proper run, once
/// their nation has qualified. Group opponents (and every knockout opponent) are drawn
/// from the PC's own salted stream — see this module's doc comment.
pub struct TournamentRun {
    pub kind: TournamentKind,
    pub tournament_season: u32,
    pub opponents: [NationId; 3],
    /// Group standings, index 0 = PC's nation, 1..=3 = `opponents`.
    pub standings: [NationStanding; 4],
    pub phase: TournamentPhase,
    pub knockout_opponent: Option<NationId>,
}

fn draw_distinct_nations(seed: u64, exclude: NationId, n: usize) -> Vec<NationId> {
    let mut rng = GoatRng::new(seed);
    let mut pool: Vec<NationId> = (0..NUM_NATIONS).filter(|&id| id != exclude).collect();
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        if pool.is_empty() {
            break;
        }
        let idx = rng.next_range_u32(0, pool.len() as u32 - 1) as usize;
        out.push(pool.swap_remove(idx));
    }
    out
}

impl TournamentRun {
    pub fn start(
        world_seed: u64,
        kind: TournamentKind,
        tournament_season: u32,
        pc_nation: NationId,
    ) -> Self {
        let seed = world_seed ^ (tournament_season as u64).rotate_left(19) ^ 0x0000_0000_0000_FA01;
        let drawn = draw_distinct_nations(seed, pc_nation, 3);
        let opponents = [drawn[0], drawn[1], drawn[2]];
        let standings = [
            NationStanding {
                nation: pc_nation,
                w: 0,
                d: 0,
                l: 0,
                gf: 0,
                ga: 0,
            },
            NationStanding {
                nation: opponents[0],
                w: 0,
                d: 0,
                l: 0,
                gf: 0,
                ga: 0,
            },
            NationStanding {
                nation: opponents[1],
                w: 0,
                d: 0,
                l: 0,
                gf: 0,
                ga: 0,
            },
            NationStanding {
                nation: opponents[2],
                w: 0,
                d: 0,
                l: 0,
                gf: 0,
                ga: 0,
            },
        ];
        TournamentRun {
            kind,
            tournament_season,
            opponents,
            standings,
            phase: TournamentPhase::GroupMatchday(0),
            knockout_opponent: None,
        }
    }

    fn other_pair_for_matchday(md: u8) -> (usize, usize) {
        match md {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        }
    }

    pub fn group_opponent(&self, md: u8) -> NationId {
        self.opponents[md as usize]
    }

    pub fn sim_other_pair(&mut self, world: &WorldGenesis, pop: &Population, seed: u64, md: u8) {
        let elapsed_weeks = elapsed_weeks_for(self.tournament_season);
        let (a, b) = Self::other_pair_for_matchday(md);
        let mut rng = GoatRng::new(seed ^ ((md as u64) << 4) ^ 0x0000_0000_0000_FA02);
        let nation_a = self.standings[a + 1].nation;
        let nation_b = self.standings[b + 1].nation;
        let str_a = national_team_strength(
            pop,
            world.nations[nation_a].stature,
            nation_a,
            elapsed_weeks,
        );
        let str_b = national_team_strength(
            pop,
            world.nations[nation_b].stature,
            nation_b,
            elapsed_weeks,
        );
        let (ga, gb) = goat_world::sim_team_match(str_a, str_b, &mut rng);
        record_national_result(&mut self.standings[a + 1], ga, gb);
        record_national_result(&mut self.standings[b + 1], gb, ga);
    }

    pub fn record_pc_group_result(&mut self, gf: u32, ga: u32) {
        let opp_idx = match self.phase {
            TournamentPhase::GroupMatchday(md) => md as usize + 1,
            _ => return,
        };
        record_national_result(&mut self.standings[0], gf, ga);
        record_national_result(&mut self.standings[opp_idx], ga, gf);
    }

    /// Advance past a group matchday (or draw the semifinal opponent once the group
    /// stage concludes with the PC in the top 2).
    pub fn advance_group(&mut self, world_seed: u64) {
        match self.phase {
            TournamentPhase::GroupMatchday(md) if md < 2 => {
                self.phase = TournamentPhase::GroupMatchday(md + 1);
            }
            TournamentPhase::GroupMatchday(_) => {
                let mut order: Vec<usize> = (0..4).collect();
                order.sort_by(|&x, &y| {
                    self.standings[y]
                        .points()
                        .cmp(&self.standings[x].points())
                        .then(
                            self.standings[y]
                                .goal_diff()
                                .cmp(&self.standings[x].goal_diff()),
                        )
                        .then(self.standings[y].gf.cmp(&self.standings[x].gf))
                });
                let pc_rank = order.iter().position(|&i| i == 0).unwrap();
                if pc_rank < 2 {
                    let seed = world_seed
                        ^ (self.tournament_season as u64).rotate_left(23)
                        ^ 0x0000_0000_0000_FA03;
                    let drawn = draw_distinct_nations(seed, self.standings[0].nation, 1);
                    self.knockout_opponent = Some(drawn[0]);
                    self.phase = TournamentPhase::Semifinal;
                } else {
                    self.phase = TournamentPhase::Done;
                }
            }
            _ => {}
        }
    }

    /// Resolve the just-played knockout match (semifinal or final): advances to the
    /// final (drawing a fresh opponent) on a semifinal win, or ends the run either way
    /// on a final. Returns `true` if the PC's nation won this match.
    pub fn resolve_knockout_match(
        &mut self,
        world_seed: u64,
        pc_nation: NationId,
        pc_goals: u32,
        opp_goals: u32,
        tiebreak_rng: &mut GoatRng,
    ) -> bool {
        let opp = self
            .knockout_opponent
            .expect("knockout opponent must be set");
        let winner =
            goat_world::domestic_cup::break_tie(pc_nation, opp, pc_goals, opp_goals, tiebreak_rng);
        let pc_wins = winner == pc_nation;
        match self.phase {
            TournamentPhase::Semifinal if pc_wins => {
                let seed = world_seed
                    ^ (self.tournament_season as u64).rotate_left(29)
                    ^ 0x0000_0000_0000_FA04;
                let drawn = draw_distinct_nations(seed, pc_nation, 1);
                self.knockout_opponent = Some(drawn[0]);
                self.phase = TournamentPhase::Final;
            }
            _ => self.phase = TournamentPhase::Done,
        }
        pc_wins
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tournament_kind_matches_engine_cadence() {
        assert_eq!(
            TournamentKind::for_season(1),
            Some(TournamentKind::WorldCup)
        );
        assert_eq!(
            TournamentKind::for_season(3),
            Some(TournamentKind::ContinentalChampionship)
        );
        assert_eq!(TournamentKind::for_season(2), None);
        assert_eq!(TournamentKind::for_season(4), None);
    }

    #[test]
    fn qualifying_campaign_finds_pc_in_exactly_one_group() {
        let campaign = QualifyingCampaign::start(42, 5, 3);
        assert_eq!(campaign.group.len(), 5);
        assert_eq!(campaign.group[campaign.pc_local_idx], 3);
        assert_eq!(campaign.total_rounds(), 5);
    }

    #[test]
    fn qualifying_round_robin_completes_after_five_rounds() {
        let world = WorldGenesis::generate(42);
        let pop = goat_world::population::genesis(42, &world);
        let mut campaign = QualifyingCampaign::start(42, 5, 3);
        for _ in 0..5 {
            assert!(!campaign.is_complete());
            let opp = campaign.play_round(&world, &pop, 999);
            if opp.is_some() {
                campaign.record_pc_result(1, 0);
            }
            campaign.advance_round();
        }
        assert!(campaign.is_complete());
        // Every group member played exactly 4 games (5-team single round-robin).
        for s in &campaign.standings {
            assert_eq!(s.w + s.d + s.l, 4);
        }
    }

    #[test]
    fn tournament_run_draws_three_distinct_opponents_never_the_pc_nation() {
        let run = TournamentRun::start(7, TournamentKind::WorldCup, 5, 2);
        assert!(!run.opponents.contains(&2));
        let mut sorted = run.opponents.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 3);
    }

    #[test]
    fn group_stage_advances_matchday_by_matchday() {
        let mut run = TournamentRun::start(7, TournamentKind::WorldCup, 5, 2);
        assert_eq!(run.phase, TournamentPhase::GroupMatchday(0));
        run.record_pc_group_result(2, 0);
        run.advance_group(7);
        assert_eq!(run.phase, TournamentPhase::GroupMatchday(1));
    }
}
