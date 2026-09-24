//! Minimal live dispatcher for the PC's continental-tier campaign (Design round 4,
//! Slice 5 follow-up — wiring the continental club competitions, built in
//! `goat_world::continental` (Slices 2-3), into the live TUI the same way
//! `cup_dispatch.rs` wired the domestic cup for Slice 5's playable gate).
//!
//! Same deliberate simplification cup_dispatch.rs documents: this is scoped to just
//! the PC's own path through one tier's group + knockout, not the full continental
//! field. Qualification (which tier, if any) reuses the real engine primitive
//! (`continental::continental_slots_for_nation` against the PC's own final league
//! position) so *whether* the PC qualifies is grounded in the real rule. The PC's 3
//! group opponents and every knockout-round opponent are drawn from their own salted
//! stream rather than the shared nation-wide qualify/draw pipeline (which needs every
//! other nation's final Tier-1 table to run) — exactly cup_dispatch's tradeoff, for
//! exactly the same reason: a live-diverged PC result never has to reconcile against a
//! parallel simulated field. Calendar placement is a placeholder cadence (TASK-TUNE),
//! same status as `cup_dispatch::CUP_ROUND_SPACING`.

use goat_rng::{GoatRng, RngSource};
use goat_world::continental::{continental_slots_for_nation, ContinentalTier, GroupStanding};
use goat_world::world::{ClubId, DivLevel, NationId, WorldGenesis};
use goat_world::Table;

/// League-round index the PC's continental group stage kicks off on (TASK-TUNE —
/// chosen to not collide with the domestic-cup dispatcher's entry rounds, which land
/// on multiples of `cup_dispatch::CUP_ROUND_SPACING` starting at 0/4/8).
pub const GROUP_ENTRY_ROUND: usize = 3;
/// Spacing between the 3 group matchdays.
pub const GROUP_SPACING: usize = 4;
/// Spacing between a knockout tie's own two legs.
pub const KNOCKOUT_LEG_SPACING: usize = 1;
/// Gap between one knockout round's last leg and the next round's (or the group
/// stage's last matchday and the first knockout round's) first leg. Tight on purpose —
/// with up to 5 knockout rounds (Tier2/Tier3, §3.3's bye-round taper) and 9 total legs,
/// this must fit inside `ROUNDS_PER_SEASON` (38) alongside the group stage and the
/// domestic-cup dispatcher's own rounds.
pub const KNOCKOUT_ROUND_GAP: usize = 2;

/// A stable small salt per tier, mirroring `continental::ContinentalTier`'s own private
/// `domain()` (not `pub`, so this dispatcher salts independently rather than reusing it —
/// the two never need to coincide, they just both need to be *some* per-tier salt).
fn tier_salt(tier: ContinentalTier) -> u64 {
    match tier {
        ContinentalTier::Tier1 => 0x51,
        ContinentalTier::Tier2 => 0x52,
        ContinentalTier::Tier3 => 0x53,
    }
}

/// Rounds needed for `tier`'s knockout bracket to reach a single champion, played as a
/// clean single-elimination ladder over the PC's own path (`ceil(log2(knockout_size))`)
/// — this dispatcher never needs the real bracket's odd-pool bye handling since the PC
/// always faces exactly one salted-stream opponent per round.
pub fn knockout_rounds_for(tier: ContinentalTier) -> usize {
    let size = tier.knockout_size();
    let mut n = 1usize;
    let mut rounds = 0usize;
    while n < size {
        n *= 2;
        rounds += 1;
    }
    rounds
}

/// Which continental tier (if any) the PC's club qualifies for, purely from the
/// season that just ended: reuses `continental::continental_slots_for_nation` (the real
/// per-nation taper table) against the PC's own final Tier-1 table position. Never the
/// 2nd/3rd domestic tiers (bible §3.1) — a non-Tier-1 club never qualifies.
pub fn qualified_tier(
    world: &WorldGenesis,
    nation: NationId,
    pc_league_tier: DivLevel,
    final_table: &Table,
    pc_club: ClubId,
) -> Option<ContinentalTier> {
    if pc_league_tier != DivLevel::Top {
        return None;
    }
    let slots = continental_slots_for_nation(world, nation);
    let pos = final_table.position_of(pc_club); // 1-based
    if pos <= slots.tier1 as usize {
        Some(ContinentalTier::Tier1)
    } else if pos <= slots.tier2 as usize {
        Some(ContinentalTier::Tier2)
    } else if pos <= slots.tier3 as usize {
        Some(ContinentalTier::Tier3)
    } else {
        None
    }
}

/// Draw `n` distinct clubs (never `pc_club`) from the whole world on their own salted
/// stream — a Fisher-Yates partial shuffle over a snapshot of every club id.
fn draw_distinct_opponents(
    seed: u64,
    world: &WorldGenesis,
    pc_club: ClubId,
    n: usize,
) -> Vec<ClubId> {
    let mut rng = GoatRng::new(seed);
    let mut pool: Vec<ClubId> = world
        .clubs
        .iter()
        .map(|c| c.id)
        .filter(|&id| id != pc_club)
        .collect();
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

/// One phase of the PC's continental run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinentalPhase {
    /// 0, 1 or 2 — the group stage's 3 single round-robin matchdays.
    GroupMatchday(u8),
    /// Two-legged knockout round `round` (0-indexed), except the final round (see
    /// `knockout_rounds_for`), which is single-leg — `leg` is 0 or 1, never 1 on a
    /// final round.
    KnockoutLeg { round: usize, leg: u8 },
    /// The PC's run has ended this season (eliminated, or the tournament is over).
    Done,
}

/// Session-local (never persisted — see `cup_dispatch.rs`'s doc comment for the same
/// rationale) tracker for one season's continental campaign.
pub struct ContinentalRun {
    pub tier: ContinentalTier,
    /// The PC's group: opponents only (the PC's own club is implicit, standings index 0).
    pub opponents: [ClubId; 3],
    /// Group standings, index 0 = PC, 1..=3 = `opponents` in order.
    pub standings: [GroupStanding; 4],
    pub phase: ContinentalPhase,
    pub next_due_round: usize,
    /// Current knockout opponent (drawn fresh each round the PC survives to).
    pub knockout_opponent: Option<ClubId>,
    /// Two-legged aggregate so far this knockout round: (pc_goals, opponent_goals).
    pub knockout_agg: (u32, u32),
}

impl ContinentalRun {
    /// Start a fresh run for a club that just qualified for `tier`.
    pub fn start(
        world: &WorldGenesis,
        world_seed: u64,
        season: u32,
        pc_club: ClubId,
        tier: ContinentalTier,
    ) -> Self {
        let seed = world_seed
            ^ (season as u64).rotate_left(19)
            ^ tier_salt(tier).rotate_left(31)
            ^ 0x0000_0000_0000_C7A1;
        let drawn = draw_distinct_opponents(seed, world, pc_club, 3);
        let opponents = [drawn[0], drawn[1], drawn[2]];
        let standings = [
            GroupStanding {
                club: pc_club,
                ..Default::default()
            },
            GroupStanding {
                club: opponents[0],
                ..Default::default()
            },
            GroupStanding {
                club: opponents[1],
                ..Default::default()
            },
            GroupStanding {
                club: opponents[2],
                ..Default::default()
            },
        ];
        ContinentalRun {
            tier,
            opponents,
            standings,
            phase: ContinentalPhase::GroupMatchday(0),
            next_due_round: GROUP_ENTRY_ROUND,
            knockout_opponent: None,
            knockout_agg: (0, 0),
        }
    }

    /// True if a continental fixture is due at this league round.
    pub fn fixture_due(&self, league_round: usize) -> bool {
        !matches!(self.phase, ContinentalPhase::Done) && league_round == self.next_due_round
    }

    /// The "other pair" (both non-PC group members) playing each other on group
    /// matchday `md` (0, 1, 2) — completes the single round-robin of 4 alongside the
    /// PC's own match that matchday. Local indices into `opponents` (0-based).
    fn other_pair_for_matchday(md: u8) -> (usize, usize) {
        match md {
            0 => (1, 2), // PC plays opponents[0]; opponents[1] vs opponents[2]
            1 => (0, 2), // PC plays opponents[1]; opponents[0] vs opponents[2]
            _ => (0, 1), // PC plays opponents[2]; opponents[0] vs opponents[1]
        }
    }

    /// Record the "other pair"'s result (a background match not involving the PC) into
    /// standings, on its own salted stream — never shares a stream with the PC's own
    /// live match.
    pub fn sim_other_pair(&mut self, world: &WorldGenesis, world_seed: u64, season: u32, md: u8) {
        let (a, b) = Self::other_pair_for_matchday(md);
        let seed = world_seed
            ^ (season as u64).rotate_left(23)
            ^ tier_salt(self.tier).rotate_left(37)
            ^ ((md as u64) << 4)
            ^ 0x0000_0000_0000_C7A2;
        let mut rng = GoatRng::new(seed);
        let club_a = self.standings[a + 1].club;
        let club_b = self.standings[b + 1].club;
        let (ga, gb) = goat_world::sim_team_match(
            world.clubs[club_a].strength,
            world.clubs[club_b].strength,
            &mut rng,
        );
        record_group_result(&mut self.standings[a + 1], ga, gb);
        record_group_result(&mut self.standings[b + 1], gb, ga);
    }

    /// The opponent due this group matchday (`opponents[md]`).
    pub fn group_opponent(&self, md: u8) -> ClubId {
        self.opponents[md as usize]
    }

    /// Record the PC's own group-match result into standings.
    pub fn record_pc_group_result(&mut self, gf: u32, ga: u32) {
        record_group_result(&mut self.standings[0], gf, ga);
        let opp_idx = match self.phase {
            ContinentalPhase::GroupMatchday(md) => md as usize + 1,
            _ => return,
        };
        record_group_result(&mut self.standings[opp_idx], ga, gf);
    }

    /// Advance past the just-played group matchday (or knockout leg), scheduling the
    /// next fixture (or resolving elimination/advancement).
    pub fn advance(&mut self, world: &WorldGenesis, world_seed: u64, season: u32, pc_club: ClubId) {
        match self.phase {
            ContinentalPhase::GroupMatchday(md) => {
                if md < 2 {
                    self.phase = ContinentalPhase::GroupMatchday(md + 1);
                    self.next_due_round += GROUP_SPACING;
                } else {
                    // Group stage complete — top 2 of 4 advance (mirrors
                    // `continental::simulate_continental`'s group-stage cut).
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
                        self.begin_knockout_round(world, world_seed, season, pc_club, 0);
                    } else {
                        self.phase = ContinentalPhase::Done;
                    }
                }
            }
            ContinentalPhase::KnockoutLeg { round, leg } => {
                let is_final = round == knockout_rounds_for(self.tier) - 1;
                if leg == 0 && !is_final {
                    self.phase = ContinentalPhase::KnockoutLeg { round, leg: 1 };
                    self.next_due_round += KNOCKOUT_LEG_SPACING;
                }
                // Otherwise this was the tie's last leg — the caller resolves it via
                // `resolve_knockout_tie` instead of `advance` (that call sets `phase`
                // to the next round or `Done` directly); nothing to do here.
            }
            ContinentalPhase::Done => {}
        }
    }

    fn begin_knockout_round(
        &mut self,
        world: &WorldGenesis,
        world_seed: u64,
        season: u32,
        pc_club: ClubId,
        round: usize,
    ) {
        let seed = world_seed
            ^ (season as u64).rotate_left(29)
            ^ tier_salt(self.tier).rotate_left(41)
            ^ ((round as u64) << 8)
            ^ 0x0000_0000_0000_C7A3;
        let drawn = draw_distinct_opponents(seed, world, pc_club, 1);
        self.knockout_opponent = Some(drawn[0]);
        self.knockout_agg = (0, 0);
        self.phase = ContinentalPhase::KnockoutLeg { round, leg: 0 };
        self.next_due_round += KNOCKOUT_ROUND_GAP;
    }

    /// True when the current (or just-finished) knockout leg is single-leg (the final).
    pub fn is_final_round(&self) -> bool {
        matches!(self.phase, ContinentalPhase::KnockoutLeg { round, .. } if round == knockout_rounds_for(self.tier) - 1)
    }

    /// Fold this leg's scoreline into the aggregate.
    pub fn accumulate_knockout_leg(&mut self, pc_goals: u32, opp_goals: u32) {
        self.knockout_agg.0 += pc_goals;
        self.knockout_agg.1 += opp_goals;
    }

    /// Resolve the knockout tie just completed (both legs played, or the single-leg
    /// final): `true` if the PC's club advances. Advances `phase`/`next_due_round` to
    /// the next knockout round on a win, or `Done` on elimination/tournament win.
    pub fn resolve_knockout_tie(
        &mut self,
        world: &WorldGenesis,
        world_seed: u64,
        season: u32,
        pc_club: ClubId,
        tiebreak_rng: &mut GoatRng,
    ) -> bool {
        let round = match self.phase {
            ContinentalPhase::KnockoutLeg { round, .. } => round,
            _ => return false,
        };
        let opp = self
            .knockout_opponent
            .expect("knockout opponent must be set");
        let (agg_pc, agg_opp) = self.knockout_agg;
        let winner =
            goat_world::domestic_cup::break_tie(pc_club, opp, agg_pc, agg_opp, tiebreak_rng);
        let pc_wins = winner == pc_club;
        if !pc_wins {
            self.phase = ContinentalPhase::Done;
        } else if round == knockout_rounds_for(self.tier) - 1 {
            self.phase = ContinentalPhase::Done; // won the whole tier
        } else {
            self.begin_knockout_round(world, world_seed, season, pc_club, round + 1);
        }
        pc_wins
    }
}

fn record_group_result(standing: &mut GroupStanding, gf: u32, ga: u32) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualified_tier_only_from_top_flight() {
        let world = WorldGenesis::generate(11);
        let table = Table::new(&world.leagues[0].clubs);
        assert_eq!(
            qualified_tier(
                &world,
                0,
                DivLevel::Second,
                &table,
                world.leagues[0].clubs[0]
            ),
            None
        );
    }

    #[test]
    fn qualified_tier_respects_taper_table() {
        let world = WorldGenesis::generate(11);
        let league = world
            .leagues
            .iter()
            .find(|l| l.tier == DivLevel::Top)
            .unwrap();
        let mut table = Table::new(&league.clubs);
        // Give every club a distinct win count so `position_of` is well-defined.
        for (i, &club) in league.clubs.iter().enumerate() {
            for _ in 0..(league.clubs.len() - i) {
                table.apply_result(club, club, 0, 0); // no-op draws just to exercise position_of below
            }
        }
        let slots = continental_slots_for_nation(&world, league.nation);
        let sorted = table.sorted();
        if slots.tier1 > 0 {
            let top_club = sorted[0].club_id;
            assert_eq!(
                qualified_tier(&world, league.nation, DivLevel::Top, &table, top_club),
                Some(ContinentalTier::Tier1)
            );
        }
        let last_club = sorted[sorted.len() - 1].club_id;
        assert_eq!(
            qualified_tier(&world, league.nation, DivLevel::Top, &table, last_club),
            None,
            "the bottom club of a 20-team table always falls outside the (<=8) taper"
        );
    }

    #[test]
    fn knockout_rounds_match_expected_ladder_depth() {
        assert_eq!(knockout_rounds_for(ContinentalTier::Tier1), 4); // 16->8->4->2
        assert_eq!(knockout_rounds_for(ContinentalTier::Tier2), 5); // 24 -> ceil(log2)=5
        assert_eq!(knockout_rounds_for(ContinentalTier::Tier3), 5); // 32->16->8->4->2
    }

    #[test]
    fn run_start_draws_three_distinct_opponents_never_the_pc() {
        let world = WorldGenesis::generate(4);
        let pc_club = 0;
        let run = ContinentalRun::start(&world, 4, 1, pc_club, ContinentalTier::Tier1);
        assert!(!run.opponents.contains(&pc_club));
        let mut sorted = run.opponents.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 3, "opponents must be pairwise distinct");
    }

    #[test]
    fn group_stage_advances_matchday_by_matchday() {
        let world = WorldGenesis::generate(4);
        let pc_club = 0;
        let mut run = ContinentalRun::start(&world, 4, 1, pc_club, ContinentalTier::Tier1);
        assert_eq!(run.phase, ContinentalPhase::GroupMatchday(0));
        assert_eq!(run.next_due_round, GROUP_ENTRY_ROUND);
        run.sim_other_pair(&world, 4, 1, 0);
        run.record_pc_group_result(2, 0);
        run.advance(&world, 4, 1, pc_club);
        assert_eq!(run.phase, ContinentalPhase::GroupMatchday(1));
        assert_eq!(run.next_due_round, GROUP_ENTRY_ROUND + GROUP_SPACING);
    }
}
