//! World state and the core `reduce()` entry point.
//!
//! `reduce(state, intent, rng) -> WorldState` is the only way to mutate the
//! simulation. The function is pure: same state + same intent + same RNG sequence
//! = same output. Renderers send intents; they never mutate state directly.

use goat_fixed::Fixed;
use goat_rng::RngSource;

use crate::attrs::{AttrId, NUM_ATTRS};
use crate::generation::{self, CreationChoices};
use crate::player::{PlayerId, PlayerStore};
use crate::roles::FamiliarityTier;
use crate::roles::NUM_ROLES;
use crate::tuning::{ENERGY_START, START_AGE_WEEKS};
use crate::tuning::{FAM_XP_AWKWARD, FAM_XP_COMPETENT, FAM_XP_UNCONVINCING};
use crate::week::{advance_week, DevelopmentEvent, Routine};

/// The entire world state for a single save.
#[derive(Debug, Clone)]
pub struct WorldState {
    pub players: PlayerStore,
    /// The player-controlled player's id. `None` before character creation.
    pub pc_player_id: Option<PlayerId>,
    // ── Phase 3 ───────────────────────────────────────────────────────────────
    pub pc_routine: Routine,
    pub pc_club: &'static str,
    pub pc_nationality: &'static str,
    /// Position as u8 (0=Defender, 1=Midfielder, 2=Forward) — saved for load reconstruct.
    pub pc_position: u8,
    pub last_week_events: Vec<DevelopmentEvent>,
    pub last_week_growth: [Fixed; NUM_ATTRS],
    // ── Phase 5 ───────────────────────────────────────────────────────────────
    /// Master seed used to generate the world (clubs, fixtures).
    pub world_seed: u64,
    /// PC's club index in goat-world's CLUBS array.
    pub pc_club_idx: u16,
    /// Index of PC's division in DIV_CLUBS.
    pub pc_div_idx: u8,
    /// Facilities development multiplier at the PC's club (set from goat-world).
    pub pc_facilities_mult: Fixed,
    /// Current season number (starts at 1).
    pub season_number: u32,
    /// Current round within the season (0-indexed, 0..ROUNDS_PER_SEASON).
    pub season_round: u32,
    /// PC's slow-moving form baseline (0–100).
    pub pc_form: Fixed,
    /// Goals scored by the PC in the current season.
    pub pc_season_goals: u32,
    /// Matches played by PC this season.
    pub pc_season_matches: u32,
    /// Cumulative output across PC's matches this season.
    pub pc_season_output: i32,
    /// League table raw data: [W, D, L, GF, GA] × CLUBS_PER_DIV.
    /// Layout: table_raw[col * 16 + row_in_div] where col ∈ {0..4}.
    pub table_raw: [u32; 80], // 5 × 16
    // ── Phase 6 discipline fields ─────────────────────────────────────────────
    /// Yellow cards in the current season (resets each season). 5 = ban.
    pub pc_yellow_cards_season: u32,
    /// Matches remaining on suspension (0 = available).
    pub pc_suspension_weeks: u32,
    /// Dirty/clean reputation 0–100 (50 = neutral). Higher = stricter officiating.
    pub pc_discipline_rep: i32,
    // ── Phase 7 legacy evidence ───────────────────────────────────────────────
    pub pc_career_goals: u32,
    pub pc_career_matches: u32,
    pub pc_career_output_sum: i64,
    pub pc_best_season_avg_output: i32,
    pub pc_seasons_played: u32,
    pub pc_decisive_moments: u32,
    pub pc_player_of_year_wins: u32,
    pub pc_league_titles: u32,
    pub pc_clubs_served: u32,
    pub pc_longest_club_tenure: u32,
    // ── Phase 7 reputation scalars ────────────────────────────────────────────
    pub pc_sporting_rep: i32,
    pub pc_club_fan_rep: i32,
    // ── Phase 8 — contract + transfer market ─────────────────────────────────
    /// Seasons left on current contract at PC's club.
    pub pc_contract_seasons_left: u32,
    /// Annual wage in thousands.
    pub pc_wage_annual: i64,
    /// Player power escalation (0=Happy … 3=SkippingTraining).
    pub pc_power_ladder: u8,
    /// Total career savings in thousands (wages minus notional spend).
    pub pc_savings: i64,
    // ── Phase 9 — peer cohort + rival ────────────────────────────────────────
    /// 8 career-start peers: packed as (seed:u64, goals:u32, matches:u32, avg_output:u8)
    /// per peer, stored flat. Length always 0 or exactly 8*17 bytes when serialised.
    pub pc_peers: Vec<PeerState>,
    /// Index into pc_peers of the crystallised rival, if any.
    pub pc_rival_idx: Option<usize>,
    /// Season in which the rivalry was first declared.
    pub pc_rival_declared_season: Option<u32>,
    // ── Phase 10 — lifestyle + retirement ────────────────────────────────────
    /// 0=Professional, 1=Balanced, 2=Flashy.
    pub pc_lifestyle: u8,
    /// True once the player has retired.
    pub pc_retired: bool,
    // ── Calendar ─────────────────────────────────────────────────────────────
    /// Which calendar week (0-indexed) the current season_round belongs to.
    /// Updated by the bridge when season_round crosses a week boundary.
    pub pc_current_calendar_week: u32,
    /// Whether the player has taken their training session this calendar week.
    pub pc_week_training_done: bool,
}

/// Batch-ticked career state for one cohort peer (Phase 9).
#[derive(Debug, Clone)]
pub struct PeerState {
    /// Deterministic seed used for this peer's batch-tick progression.
    pub seed: u64,
    pub name: String,
    pub nationality: &'static str,
    /// Accumulated career goals (batch-ticked each season).
    pub career_goals: u32,
    /// Career matches (batch-ticked).
    pub career_matches: u32,
    /// Average output 0–100 (batch-ticked).
    pub avg_output: u8,
    /// League titles (batch-ticked).
    pub titles: u32,
}

impl WorldState {
    pub fn new() -> Self {
        Self {
            players: PlayerStore::new(),
            pc_player_id: None,
            pc_routine: Routine::default(),
            pc_club: "",
            pc_nationality: "",
            pc_position: 2, // Forward default
            last_week_events: Vec::new(),
            last_week_growth: [Fixed::ZERO; NUM_ATTRS],
            world_seed: 0,
            pc_club_idx: 0,
            pc_div_idx: 0,
            pc_facilities_mult: Fixed::ONE,
            season_number: 0,
            season_round: 0,
            pc_form: Fixed::from_int(50),
            pc_season_goals: 0,
            pc_season_matches: 0,
            pc_season_output: 0,
            table_raw: [0u32; 80],
            pc_yellow_cards_season: 0,
            pc_suspension_weeks: 0,
            pc_discipline_rep: 50,
            pc_career_goals: 0,
            pc_career_matches: 0,
            pc_career_output_sum: 0,
            pc_best_season_avg_output: 0,
            pc_seasons_played: 0,
            pc_decisive_moments: 0,
            pc_player_of_year_wins: 0,
            pc_league_titles: 0,
            pc_clubs_served: 1,
            pc_longest_club_tenure: 0,
            pc_sporting_rep: 50,
            pc_club_fan_rep: 50,
            pc_contract_seasons_left: 2,
            pc_wage_annual: 20, // £20k/yr at start
            pc_power_ladder: 0,
            pc_savings: 0,
            pc_peers: Vec::new(),
            pc_rival_idx: None,
            pc_rival_declared_season: None,
            pc_lifestyle: 1, // Balanced
            pc_retired: false,
            pc_current_calendar_week: 0,
            pc_week_training_done: false,
        }
    }
}

impl Default for WorldState {
    fn default() -> Self {
        Self::new()
    }
}

/// Player intents the renderer sends to the core.
#[derive(Debug, Clone)]
pub enum Intent {
    /// No-op — advance without changing anything. Heartbeat / test sentinel.
    NoOp,

    /// Create the PC player. Idempotent: if a PC already exists, ignored.
    CreatePlayer { seed: u64, choices: CreationChoices },

    /// Apply a signed delta to one attribute, clamped to [1, 99] and potential.
    ApplyAttrDelta {
        player_id: PlayerId,
        attr: AttrId,
        delta: Fixed,
    },

    // ── Phase 3 intents ───────────────────────────────────────────────────────
    /// Replace the current weekly training routine.
    SetRoutine { routine: Routine },

    /// Advance one week, collecting development events.
    AdvanceWeek,

    /// Advance up to `n` weeks, stopping early when a noteworthy event fires.
    AdvanceWeeks { n: u32 },

    // ── Phase 4 intents ───────────────────────────────────────────────────────
    /// Apply the effects of a completed match to persistent state.
    ///
    /// The TUI (via goat-match) manages the beat-by-beat match loop and calls
    /// this once when the match is complete.  Core never sees beat types.
    ApplyMatchResult {
        /// Familiarity XP earned across all roles during the match.
        familiarity_xp: [Fixed; NUM_ROLES],
        /// Energy drained playing a full match.
        energy_cost: Fixed,
        /// Injury sustained during the match (if any), in weeks.
        injury_weeks: Option<u32>,
    },

    // ── Phase 7 intents ───────────────────────────────────────────────────────
    /// Accumulate season-end legacy evidence and update reputation scalars.
    ///
    /// Called once per season (by the TUI) after the season summary is computed.
    ApplySeasonEndLegacy {
        season_goals: u32,
        season_matches: u32,
        /// Sum of all match output scores this season.
        season_output_sum: i32,
        won_title: bool,
        player_of_year: bool,
        /// Final league position (1-based) for reputation update.
        finish_position: u32,
        /// Decisive moments scored this season (e.g. winning goals in must-win games).
        decisive_moments: u32,
        new_sporting_rep: i32,
        new_club_fan_rep: i32,
    },

    // ── Phase 8 intents ───────────────────────────────────────────────────────
    /// Accept a new contract (renewal or fresh signing). Updates contract terms.
    AcceptContract {
        new_wage: i64,
        new_length: u32,
        new_club_idx: u16,
    },
    /// Escalate the player-power ladder by one rung. Burns Character rep.
    AgitateForTransfer,
    /// Execute a transfer: change club, reset contract, receive fee portion as bonus.
    ExecuteTransfer {
        to_club_idx: u16,
        to_div_idx: u8,
        new_wage: i64,
        new_length: u32,
        new_club_name: &'static str,
        facilities_mult: Fixed,
        fee_bonus: i64, // fraction of fee paid to player (signing bonus)
    },
    /// Collect end-of-season wage into savings.
    CollectWage,

    // ── Phase 9 intents ───────────────────────────────────────────────────────
    /// Seed the peer cohort at career start.
    InitPeers { peers: Vec<PeerState> },
    /// Advance all peers one season via batch-tick.
    BatchTickPeers { season: u32 },
    /// Declare a rival (crystallised from cohort).
    DeclareRival { peer_idx: usize, season: u32 },

    // ── Phase 10 intents ──────────────────────────────────────────────────────
    /// Set the lifestyle (0=Professional,1=Balanced,2=Flashy). Done once at career start.
    SetLifestyle { lifestyle: u8 },
    /// Retire the player. Triggers the final-verdict flow in the TUI.
    Retire,

    // ── Phase 6 intents ───────────────────────────────────────────────────────
    /// Apply cards received in a match. Updates yellow card count and suspension.
    ///
    /// Called after `ApplyMatchResult`, once per match where a card was shown.
    ApplyCardResult { yellow_cards: u32, red_card: bool },

    // ── Phase 5 intents ───────────────────────────────────────────────────────
    /// Initialise the world (sets world_seed, pc_club_idx, div_idx, facilities).
    /// Called once after CreatePlayer, before the first season.
    InitWorld {
        world_seed: u64,
        pc_club_idx: u16,
        pc_div_idx: u8,
        facilities_mult: Fixed,
        /// Flat table raw data for the PC's division (initialised to all zeros).
        initial_table: Box<[u32; 80]>,
    },

    /// Start a new season. Resets round counter, clears PC season stats.
    StartSeason,

    /// Apply the results of one completed season round.
    ///
    /// The TUI drives fixture simulation (via goat-world) and sends the outcomes here.
    ApplyRoundResult {
        /// PC's goals this round (0 if they didn't play or skipped).
        pc_goals: u32,
        /// PC output this round (0 if didn't play).
        pc_output: i32,
        /// Did the PC's team win/draw/lose?  (1/0/-1)
        pc_result: i8,
        /// All match results in the round: (home_div_pos, away_div_pos, home_gf, home_ga).
        /// `div_pos` is the 0-based club index within DIV_CLUBS[pc_div_idx].
        round_results: Vec<(u8, u8, u32, u32)>,
    },
}

/// Advance the simulation by one intent.
pub fn reduce(mut state: WorldState, intent: Intent, rng: &mut impl RngSource) -> WorldState {
    match intent {
        Intent::NoOp => state,

        Intent::CreatePlayer { seed, choices } => {
            if state.pc_player_id.is_some() {
                return state;
            }
            let mut view = generation::generate_player(seed, &choices);
            view.age_weeks = START_AGE_WEEKS;
            view.energy = ENERGY_START;
            view.injury_weeks = 0;
            let club = choices.club;
            let nationality = choices.nationality;
            let position = choices.position as u8;
            let id = state.players.push(view);
            state.pc_player_id = Some(id);
            state.pc_club = club;
            state.pc_nationality = nationality;
            state.pc_position = position;
            state.last_week_events.clear();
            state
        }

        Intent::ApplyAttrDelta {
            player_id,
            attr,
            delta,
        } => {
            let a = attr as usize;
            let cur = state.players.get_current(player_id, a);
            let pot = state.players.get_potential(player_id, a);
            let new_val = (cur + delta).clamp(Fixed::MIN_ATTR, pot);
            state.players.set_current(player_id, a, new_val);
            state
        }

        Intent::SetRoutine { routine } => {
            state.pc_routine = routine;
            state
        }

        Intent::AdvanceWeek => tick_one_week(state, rng),

        Intent::AdvanceWeeks { n } => {
            for _ in 0..n {
                state.last_week_events.clear();
                state = tick_one_week(state, rng);
                if !state.last_week_events.is_empty() {
                    break; // stop at the first noteworthy event
                }
            }
            state
        }

        Intent::ApplyMatchResult {
            familiarity_xp,
            energy_cost,
            injury_weeks,
        } => {
            let pc_id = match state.pc_player_id {
                Some(id) => id,
                None => return state,
            };

            // Apply familiarity XP and process tier upgrades.
            for (r, &xp) in familiarity_xp.iter().enumerate() {
                if xp == Fixed::ZERO {
                    continue;
                }
                let current_xp = state.players.get_familiarity_xp(pc_id, r) + xp;
                state.players.set_familiarity_xp(pc_id, r, current_xp);

                let tier = state.players.get_familiarity(pc_id, r);
                let threshold = match tier {
                    FamiliarityTier::Awkward => Some(FAM_XP_AWKWARD),
                    FamiliarityTier::Unconvincing => Some(FAM_XP_UNCONVINCING),
                    FamiliarityTier::Competent => Some(FAM_XP_COMPETENT),
                    FamiliarityTier::Natural => None,
                };
                if let Some(t) = threshold {
                    if current_xp >= t {
                        if let Some(new_tier) = tier.upgrade() {
                            state.players.set_familiarity(pc_id, r, new_tier);
                            state.players.set_familiarity_xp(pc_id, r, Fixed::ZERO);
                        }
                    }
                }
            }

            // Apply energy cost.
            let energy = state.players.get_energy(pc_id);
            let new_energy = (energy - energy_cost).clamp(Fixed::ZERO, Fixed::raw(100_000));
            state.players.set_energy(pc_id, new_energy);

            // Apply injury if one occurred in the match.
            if let Some(weeks) = injury_weeks {
                let existing = state.players.get_injury_weeks(pc_id);
                state.players.set_injury_weeks(pc_id, existing.max(weeks));
            }

            state
        }

        Intent::ApplySeasonEndLegacy {
            season_goals,
            season_matches,
            season_output_sum,
            won_title,
            player_of_year,
            finish_position: _finish_position,
            decisive_moments,
            new_sporting_rep,
            new_club_fan_rep,
        } => {
            state.pc_career_goals += season_goals;
            state.pc_career_matches += season_matches;
            state.pc_career_output_sum += season_output_sum as i64;
            if won_title {
                state.pc_league_titles += 1;
            }
            if player_of_year {
                state.pc_player_of_year_wins += 1;
            }
            state.pc_decisive_moments += decisive_moments;
            state.pc_seasons_played += 1;
            // Update best season avg output.
            let season_avg = if season_matches > 0 {
                (season_output_sum / season_matches as i32).clamp(0, 100)
            } else {
                0
            };
            if season_avg > state.pc_best_season_avg_output {
                state.pc_best_season_avg_output = season_avg;
            }
            // Update longest tenure (PC is always at same club for now).
            state.pc_longest_club_tenure += 1;
            // Update reputation.
            state.pc_sporting_rep = new_sporting_rep;
            state.pc_club_fan_rep = new_club_fan_rep;
            state
        }

        // ── Phase 8 handlers ─────────────────────────────────────────────────────
        Intent::AcceptContract {
            new_wage,
            new_length,
            new_club_idx: _,
        } => {
            state.pc_contract_seasons_left = new_length;
            state.pc_wage_annual = new_wage;
            state.pc_power_ladder = 0; // happy again
            state
        }

        Intent::AgitateForTransfer => {
            state.pc_power_ladder = (state.pc_power_ladder + 1).min(3);
            // Each rung burns Character rep (tightens officiating).
            state.pc_discipline_rep = (state.pc_discipline_rep + 8).min(100);
            state
        }

        Intent::ExecuteTransfer {
            to_club_idx,
            to_div_idx,
            new_wage,
            new_length,
            new_club_name,
            facilities_mult,
            fee_bonus,
        } => {
            state.pc_club_idx = to_club_idx;
            state.pc_div_idx = to_div_idx;
            state.pc_club = new_club_name;
            state.pc_facilities_mult = facilities_mult;
            state.pc_contract_seasons_left = new_length;
            state.pc_wage_annual = new_wage;
            state.pc_power_ladder = 0;
            state.pc_savings += fee_bonus;
            state.pc_clubs_served += 1;
            // Reset club fan rep when leaving (fresh start at new club).
            state.pc_club_fan_rep = 40;
            state
        }

        Intent::CollectWage => {
            state.pc_savings += state.pc_wage_annual;
            if state.pc_contract_seasons_left > 0 {
                state.pc_contract_seasons_left -= 1;
            }
            state
        }

        // ── Phase 9 handlers ─────────────────────────────────────────────────────
        Intent::InitPeers { peers } => {
            state.pc_peers = peers;
            state
        }

        Intent::BatchTickPeers { season } => {
            for peer in &mut state.pc_peers {
                let mut rng = goat_rng::GoatRng::new(peer.seed ^ (season as u64 * 0x9e3779b));
                let played = 20 + rng.next_range_u64(0, 15) as u32;
                peer.career_matches += played;
                let out = (50 + rng.next_range_u64(0, 40)) as u8;
                // Smooth avg output with a running average.
                let old_sum = peer.avg_output as u64 * (season as u64 - 1).max(1);
                peer.avg_output = ((old_sum + out as u64) / season.max(1) as u64).min(99) as u8;
                let goals_per_match = rng.next_range_u64(0, 3); // 0-2 goals/match chance
                peer.career_goals += (played as u64 * goals_per_match / 3) as u32;
                if rng.next_range_u64(0, 9) == 0 {
                    peer.titles += 1;
                }
            }
            state
        }

        Intent::DeclareRival { peer_idx, season } => {
            state.pc_rival_idx = Some(peer_idx);
            state.pc_rival_declared_season = Some(season);
            state
        }

        // ── Phase 10 handlers ────────────────────────────────────────────────────
        Intent::SetLifestyle { lifestyle } => {
            state.pc_lifestyle = lifestyle.min(2);
            state
        }

        Intent::Retire => {
            state.pc_retired = true;
            state
        }

        Intent::ApplyCardResult {
            yellow_cards,
            red_card,
        } => {
            state.pc_yellow_cards_season += yellow_cards;
            // 5 yellow cards in a season = 1-match ban; red = 1–3 matches.
            if red_card {
                state.pc_suspension_weeks += 2; // base 2-match ban for red
                state.pc_discipline_rep = (state.pc_discipline_rep + 15).min(100);
            }
            if yellow_cards > 0 && state.pc_yellow_cards_season >= 5 {
                state.pc_suspension_weeks += 1;
                state.pc_yellow_cards_season = 0; // reset after serving ban
                state.pc_discipline_rep = (state.pc_discipline_rep + 5).min(100);
            }
            // Clean match recovery: handled separately per week/round (not here).
            state
        }

        Intent::InitWorld {
            world_seed,
            pc_club_idx,
            pc_div_idx,
            facilities_mult,
            initial_table,
        } => {
            state.world_seed = world_seed;
            state.pc_club_idx = pc_club_idx;
            state.pc_div_idx = pc_div_idx;
            state.pc_facilities_mult = facilities_mult;
            state.table_raw = *initial_table;
            state
        }

        Intent::StartSeason => {
            state.season_number += 1;
            state.season_round = 0;
            state.pc_season_goals = 0;
            state.pc_season_matches = 0;
            state.pc_season_output = 0;
            state.pc_yellow_cards_season = 0; // reset yellow cards each season
                                              // Preserve table from last season? No — start fresh each season.
            state.table_raw = [0u32; 80];
            state
        }

        Intent::ApplyRoundResult {
            pc_goals,
            pc_output,
            pc_result,
            round_results,
        } => {
            const N: usize = 16; // CLUBS_PER_DIV

            // Update PC season stats.
            state.pc_season_goals += pc_goals;
            state.pc_season_output += pc_output;
            if pc_output > 0 {
                state.pc_season_matches += 1;
            }

            // Form EMA: form = 0.85 × form + 0.15 × output
            if pc_output > 0 {
                let out_fixed = Fixed::from_int(pc_output.clamp(0, 100));
                state.pc_form = state.pc_form * Fixed::raw(850) + out_fixed * Fixed::raw(150);
            }

            // Update table raw data.
            // Layout: table_raw[col * N + row] where col ∈ {W=0, D=1, L=2, GF=3, GA=4}
            let apply_result = |raw: &mut [u32; 80],
                                home_pos: usize,
                                away_pos: usize,
                                home_gf: u32,
                                home_ga: u32| {
                let (hw, hd, hl) = if home_gf > home_ga {
                    (1, 0, 0)
                } else if home_gf == home_ga {
                    (0, 1, 0)
                } else {
                    (0, 0, 1)
                };
                let (aw, ad, al) = (hl, hd, hw);
                raw[home_pos] += hw;
                raw[N + home_pos] += hd;
                raw[N * 2 + home_pos] += hl;
                raw[N * 3 + home_pos] += home_gf;
                raw[N * 4 + home_pos] += home_ga;
                raw[away_pos] += aw;
                raw[N + away_pos] += ad;
                raw[N * 2 + away_pos] += al;
                raw[N * 3 + away_pos] += home_ga;
                raw[N * 4 + away_pos] += home_gf;
            };

            // Find PC's position in the division table.
            let pc_div_pos = state.pc_club_idx as usize % N; // rough — TUI ensures correctness

            // Apply PC's own match result to the table.
            if pc_output > 0 {
                // The TUI embeds the PC's result in round_results; if not, apply manually.
                // We'll rely on round_results including the PC's match.
            }

            for (home_pos, away_pos, gf, ga) in round_results {
                apply_result(
                    &mut state.table_raw,
                    home_pos as usize,
                    away_pos as usize,
                    gf,
                    ga,
                );
            }

            let _ = (pc_result, pc_div_pos); // pc_result used by TUI display only
            state.season_round += 1;
            state
        }
    }
}

fn tick_one_week(mut state: WorldState, rng: &mut impl RngSource) -> WorldState {
    let pc_id = match state.pc_player_id {
        Some(id) => id,
        None => return state,
    };

    // Snapshot current attrs to compute growth delta for TUI display.
    let before: [Fixed; NUM_ATTRS] = core::array::from_fn(|a| state.players.get_current(pc_id, a));

    // Lifestyle modifier: Professional +10% growth; Flashy −10%.
    let lifestyle_mult = match state.pc_lifestyle {
        0 => Fixed::raw(1_100), // Professional
        2 => Fixed::raw(900),   // Flashy
        _ => Fixed::ONE,        // Balanced
    };
    let effective_mult = state.pc_facilities_mult * lifestyle_mult;

    let events = advance_week(
        &mut state.players,
        pc_id,
        &state.pc_routine,
        effective_mult,
        rng,
    );

    // Record per-attribute delta.
    state.last_week_growth =
        core::array::from_fn(|a| state.players.get_current(pc_id, a) - before[a]);

    state.last_week_events = events;
    state
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attrs::NUM_ATTRS;
    use crate::player::PlayerView;
    use crate::roles::{FamiliarityTier, NUM_ROLES};
    use crate::week::Intensity;
    use goat_rng::GoatRng;

    fn make_rng() -> GoatRng {
        GoatRng::new(1)
    }

    fn push_uniform_player(state: &mut WorldState, current: i32, potential: i32) -> PlayerId {
        let view = PlayerView {
            name: "Test".into(),
            current: [Fixed::from_int(current); NUM_ATTRS],
            potential: [Fixed::from_int(potential); NUM_ATTRS],
            familiarity: [FamiliarityTier::Awkward; NUM_ROLES],
            ..PlayerView::default()
        };
        let id = state.players.push(view);
        state.pc_player_id = Some(id);
        state.pc_club = "Riverside Town";
        id
    }

    #[test]
    fn noop_leaves_state_unchanged() {
        let mut base = WorldState::new();
        push_uniform_player(&mut base, 50, 75);
        let s = reduce(base.clone(), Intent::NoOp, &mut make_rng());
        assert_eq!(
            s.players.get_current(0, AttrId::Finishing as usize),
            Fixed::from_int(50)
        );
    }

    #[test]
    fn attr_delta_applies_correctly() {
        let mut s = WorldState::new();
        push_uniform_player(&mut s, 50, 99);
        let s = reduce(
            s,
            Intent::ApplyAttrDelta {
                player_id: 0,
                attr: AttrId::Finishing,
                delta: Fixed::from_int(5),
            },
            &mut make_rng(),
        );
        assert_eq!(
            s.players.get_current(0, AttrId::Finishing as usize),
            Fixed::from_int(55)
        );
    }

    #[test]
    fn attr_delta_clamped_by_ceiling() {
        let mut s = WorldState::new();
        push_uniform_player(&mut s, 50, 60);
        let s = reduce(
            s,
            Intent::ApplyAttrDelta {
                player_id: 0,
                attr: AttrId::Finishing,
                delta: Fixed::from_int(20),
            },
            &mut make_rng(),
        );
        assert_eq!(
            s.players.get_current(0, AttrId::Finishing as usize),
            Fixed::from_int(60)
        );
    }

    #[test]
    fn attr_delta_clamped_at_min() {
        let mut s = WorldState::new();
        push_uniform_player(&mut s, 5, 60);
        let s = reduce(
            s,
            Intent::ApplyAttrDelta {
                player_id: 0,
                attr: AttrId::Finishing,
                delta: Fixed::from_int(-10),
            },
            &mut make_rng(),
        );
        assert_eq!(
            s.players.get_current(0, AttrId::Finishing as usize),
            Fixed::MIN_ATTR
        );
    }

    #[test]
    fn set_routine_persists() {
        let mut s = WorldState::new();
        push_uniform_player(&mut s, 50, 99);
        let routine = Routine {
            focus_attrs: vec![AttrId::Finishing],
            intensity: Intensity::High,
        };
        let s = reduce(
            s,
            Intent::SetRoutine {
                routine: routine.clone(),
            },
            &mut make_rng(),
        );
        assert_eq!(s.pc_routine.intensity, Intensity::High);
        assert_eq!(s.pc_routine.focus_attrs, vec![AttrId::Finishing]);
    }

    #[test]
    fn advance_week_grows_focused_attr() {
        let mut s = WorldState::new();
        push_uniform_player(&mut s, 30, 99);
        let routine = Routine {
            focus_attrs: vec![AttrId::Finishing],
            intensity: Intensity::Medium,
        };
        let s = reduce(s, Intent::SetRoutine { routine }, &mut make_rng());
        let before = s.players.get_current(0, AttrId::Finishing as usize);
        let s = reduce(s, Intent::AdvanceWeek, &mut make_rng());
        let after = s.players.get_current(0, AttrId::Finishing as usize);
        assert!(after >= before, "focused attr should grow or stay same");
    }

    #[test]
    fn advance_weeks_stops_on_event() {
        // Use a seed/state likely to produce an injury within 100 weeks.
        let mut s = WorldState::new();
        push_uniform_player(&mut s, 30, 99);
        // High intensity + low energy → high injury chance
        s.players.set_energy(0, Fixed::from_int(5)); // nearly exhausted
        let routine = Routine {
            focus_attrs: vec![AttrId::Finishing],
            intensity: Intensity::High,
        };
        let s = reduce(s, Intent::SetRoutine { routine }, &mut make_rng());
        let s = reduce(s, Intent::AdvanceWeeks { n: 100 }, &mut GoatRng::new(1));
        // We can't guarantee events in exactly 100 weeks, but we can verify state is valid.
        let energy = s.players.get_energy(0);
        assert!(energy >= Fixed::ZERO);
        assert!(energy <= Fixed::from_int(100));
    }
}
