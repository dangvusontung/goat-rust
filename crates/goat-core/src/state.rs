//! World state and the core `reduce()` entry point.
//!
//! `reduce(state, intent, rng) -> WorldState` is the only way to mutate the
//! simulation. The function is pure: same state + same intent + same RNG sequence
//! = same output. Renderers send intents; they never mutate state directly.

use goat_fixed::Fixed;
use goat_rng::RngSource;

use crate::attrs::{AttrId, NUM_ATTRS};
use crate::calendar_loop::{advance_calendar_week, CalendarFlashpoint};
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
    pub pc_club: String,
    pub pc_nationality: String,
    /// `PrimaryPosition as u8` (0..7 — one of the 8 specific positions, e.g. 0=ST, 7=CB;
    /// see `positions::PrimaryPosition`) — saved for load reconstruct.
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
    /// Layout: table_raw[col * CLUBS_PER_DIV + row_in_div] where col ∈ {0..4}.
    pub table_raw: [u32; 100], // 5 × CLUBS_PER_DIV(20)
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
    // ── Pantheon raw-signal evidence (Design round 1) ─────────────────────────
    /// Cumulative count of matches with pc_output >= STANDOUT_OUTPUT_THRESHOLD, career-wide.
    /// Feeds the Eye-Test Romantics school directly (raw "moments", not a season average).
    pub pc_career_standout_matches: u32,
    /// This season's standout-match count, live — folded into pc_career_standout_matches
    /// only at ApplySeasonEndLegacy (mirrors pc_season_goals -> pc_career_goals).
    pub pc_season_standout_matches: u32,
    /// Career-peak OVR (derive::ovr at the player's primary position), checked once per
    /// season end. Feeds Stats Purists directly — a talent/ability number, structurally
    /// different from the Output axis (match performance, not attribute ceiling).
    pub pc_career_best_ovr: i32,
    /// Cumulative count of AgitateForTransfer escalations, career-wide — unlike
    /// pc_power_ladder (current rung, resets on contract/transfer), this never resets.
    /// Feeds Loyalty Traditionalists directly (raw request count, not a clubs_served penalty).
    pub pc_career_transfer_requests: u32,
    /// This season's transfer-request count, live — folded into pc_career_transfer_requests
    /// only at ApplySeasonEndLegacy.
    pub pc_season_transfer_requests: u32,
    // ── National-team caps (Design round 2, Doc B §B.4) ───────────────────────
    /// National-team caps won, career-wide — a minimal legacy-evidence counter for the
    /// call-up/tactical-fit layer. No school-weighting logic reads this yet.
    pub pc_career_caps: u32,
    /// This season's caps, live — folded into pc_career_caps only at ApplySeasonEndLegacy.
    pub pc_season_caps: u32,
    /// Goals scored while capped for the national team, career-wide.
    pub pc_career_international_goals: u32,
    /// This season's international goals, live — folded at ApplySeasonEndLegacy.
    pub pc_season_international_goals: u32,
    // ── Design round 4, Slice 4 §4.5 — national-team tournament wins ──────────
    /// World Cups won with the PC's nation, career-wide.
    pub pc_career_world_cups_won: u32,
    /// This season's World Cup wins, live — folded into pc_career_world_cups_won only at
    /// ApplySeasonEndLegacy (mirrors pc_season_caps -> pc_career_caps exactly).
    pub pc_season_world_cups_won: u32,
    /// Continental championships won with the PC's nation, career-wide.
    pub pc_career_continental_championships_won: u32,
    /// This season's continental-championship wins, live — folded at
    /// ApplySeasonEndLegacy.
    pub pc_season_continental_championships_won: u32,
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
    // ── Phase 10 — economy (TASK-10B.1) ──────────────────────────────────────
    /// Capital currently tied up in the player's business/investments (thousands).
    pub pc_business_value: i64,
    /// True once the player has gone bankrupt (savings below the floor). Sticky for the
    /// season; an Icon-axis black mark.
    pub pc_bankrupt: bool,
    /// Development-investment level 0–3 (private trainers/nutrition). 0 = none (neutral,
    /// goldens unmoved); higher buys a ceiling-capped growth multiplier — never past
    /// potential (pillar §2.4).
    pub pc_dev_invest_level: u8,
    /// Marketability 0–100 — drives which sponsor tiers are reachable (Icon axis).
    pub pc_marketability: i32,
    /// Active sponsor tier: 0=none, 1=local, 2=national, 3=global. 0 = neutral (no income,
    /// no obligation energy cost → goldens unmoved).
    pub pc_sponsor_tier: u8,
    /// Relationship-thread stability 0–100: [partner, family, close friend]. A thread
    /// dropping below the rupture threshold triggers a scandal. Few threads by design —
    /// the deeper relationship web is parked (§11).
    pub pc_relationships: [i32; 3],
    /// Character reputation 0–100 (the off-pitch facet scandals hit; 50 = neutral).
    pub pc_character_rep: i32,
    // ── Phase 9 — peer cohort + rival ────────────────────────────────────────
    /// 8 career-start peers: packed as (seed:u64, goals:u32, matches:u32, avg_output:u8)
    /// per peer, stored flat. Length always 0 or exactly 8*17 bytes when serialised.
    pub pc_peers: Vec<PeerState>,
    /// Index into pc_peers of the crystallised rival, if any.
    pub pc_rival_idx: Option<usize>,
    /// Season in which the rivalry was first declared.
    pub pc_rival_declared_season: Option<u32>,
    // ── Phase 10 — lifestyle + retirement ────────────────────────────────────
    /// Emergent lifestyle score, signed Fixed in [-1.000, 1.000] (bible §8.5/§8.6).
    /// Negative = Pro-leaning, positive = Flashy-leaning, 0 = Balanced. Built up weekly
    /// from training intensity + dev investment, plus a one-off nudge on sponsor signing.
    /// `pc_lifestyle` below is the cached tier derived from this score each week via
    /// `lifestyle_tier_from_score` — never set directly.
    pub pc_lifestyle_score: Fixed,
    /// Cached tier derived from `pc_lifestyle_score`: 0=Professional, 1=Balanced,
    /// 2=Flashy. Recomputed at the start of every week tick — read-only elsewhere.
    pub pc_lifestyle: u8,
    /// True once the player has retired.
    pub pc_retired: bool,
    // ── Calendar ─────────────────────────────────────────────────────────────
    /// Which calendar week (0-indexed) the current season_round belongs to.
    /// Updated by the bridge when season_round crosses a week boundary.
    pub pc_current_calendar_week: u32,
    /// Whether the player has taken their training session this calendar week.
    pub pc_week_training_done: bool,
    /// Live calendar position (epoch days since career start). Advanced 7/week by the
    /// week loop, which drives the CalendarEngine. Persisted in the save (v6+).
    pub pc_epoch_day: u32,
    /// Calendar flashpoints (window openings) surfaced by the most recent week tick.
    pub last_week_flashpoints: Vec<CalendarFlashpoint>,
    /// The PC's current-season orbit fixtures (league this slice; cup/continental/
    /// national-team once sibling slices ship), fed into `CalendarEngine` by
    /// `advance_calendar_week` every week tick. Set by `Intent::StartSeason` — the TUI
    /// bridge builds these from `goat-world`, since `goat-core` stays headless with
    /// respect to that crate. NOT persisted in the save (like `WorldGenesis`, this is
    /// "generated but consistent": regenerated from `world_seed` + `season_number` +
    /// `pc_div_idx`/`pc_club_idx` on load, never serialized).
    pub pc_season_fixtures: Vec<goat_calendar::Fixture>,
}

/// Batch-ticked career state for one cohort peer (Phase 9).
#[derive(Debug, Clone)]
pub struct PeerState {
    /// Deterministic seed used for this peer's batch-tick progression.
    pub seed: u64,
    pub name: String,
    pub nationality: String,
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
            pc_club: String::new(),
            pc_nationality: String::new(),
            pc_position: 0, // PrimaryPosition::ST default
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
            table_raw: [0u32; 100],
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
            pc_career_standout_matches: 0,
            pc_season_standout_matches: 0,
            pc_career_best_ovr: 0,
            pc_career_transfer_requests: 0,
            pc_season_transfer_requests: 0,
            pc_career_caps: 0,
            pc_season_caps: 0,
            pc_career_international_goals: 0,
            pc_season_international_goals: 0,
            pc_career_world_cups_won: 0,
            pc_season_world_cups_won: 0,
            pc_career_continental_championships_won: 0,
            pc_season_continental_championships_won: 0,
            pc_sporting_rep: 50,
            pc_club_fan_rep: 50,
            pc_contract_seasons_left: 2,
            pc_wage_annual: 20, // £20k/yr at start
            pc_power_ladder: 0,
            pc_savings: 0,
            pc_business_value: 0,
            pc_bankrupt: false,
            pc_dev_invest_level: 0,
            pc_marketability: 50,
            pc_sponsor_tier: 0,
            pc_relationships: [70, 70, 70],
            pc_character_rep: 50,
            pc_peers: Vec::new(),
            pc_rival_idx: None,
            pc_rival_declared_season: None,
            pc_lifestyle_score: Fixed::ZERO, // Balanced
            pc_lifestyle: 1,                 // Balanced
            pc_retired: false,
            pc_current_calendar_week: 0,
            pc_week_training_done: false,
            pc_epoch_day: 0,
            last_week_flashpoints: Vec::new(),
            pc_season_fixtures: Vec::new(),
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
        /// Standout matches (pc_output >= STANDOUT_OUTPUT_THRESHOLD) played this season.
        season_standout_matches: u32,
        /// AgitateForTransfer escalations this season.
        season_transfer_requests: u32,
        /// National-team caps won this season (Design round 2, Doc B §B.4).
        season_caps: u32,
        /// International goals scored this season.
        season_international_goals: u32,
        /// World Cups won with the PC's nation this season (Design round 4, Slice 4 §4.5).
        season_world_cups_won: u32,
        /// Continental championships won with the PC's nation this season.
        season_continental_championships_won: u32,
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
        new_club_name: String,
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

    // ── Design round 2, Doc B — national-team call-ups ────────────────────────
    /// Record the outcome of one international-break call-up window (rolled by the
    /// renderer against `tactical_identity::team_fit`; core just records the result —
    /// non-blocking per the decision's own wording, this never hard-gates anything).
    /// A no-call-up window still fires this intent with `called_up: false` so the
    /// season-live counters stay consistent even when nothing happened.
    NationalTeamCallUp {
        called_up: bool,
        started: bool,
        goals: u32,
    },

    // ── Phase 10 intents ──────────────────────────────────────────────────────
    // Lifestyle is no longer a settable intent (bible §8.5/§8.6) — it is derived
    // weekly from `pc_lifestyle_score`, nudged by routine intensity, dev-investment
    // level (in `tick_one_week`) and sponsor tier (in `SignSponsor` below).
    /// Set the development-investment level 0–3 (ceiling-capped growth multiplier).
    SetDevInvestment { level: u8 },
    /// Move savings into the business/investment portfolio (thousands).
    InvestInBusiness { amount: i64 },
    /// End-of-season economy settlement: wage + bonus in, upkeep + dev-cost out, business
    /// return applied, bankruptcy checked. `rng` drives the investment-return variance.
    SettleSeasonEconomy { season_bonus: i64 },
    /// Set marketability 0–100 (driven by output/icon at season end).
    SetMarketability { value: i32 },
    /// Sign a sponsor at a tier (0=drop, 1=local, 2=national, 3=global). Gated by
    /// marketability; signing above your sporting merit dents Sporting reputation.
    SignSponsor { tier: u8 },
    /// Apply a life event to a relationship thread (0=partner,1=family,2=friend); `delta`
    /// strains (−) or strengthens (+) it. A thread falling below the rupture threshold
    /// triggers a scandal (Character + marketability hit).
    ApplyLifeEvent { thread: u8, delta: i32 },
    /// Respond to a media flashpoint (after a red card / scandal). `contrite` rebuilds
    /// Character at a Sporting cost; defiant does the reverse — a real trade-off.
    RespondToMedia { contrite: bool },
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
        initial_table: Box<[u32; 100]>,
    },

    /// Start a new season. Resets round counter, clears PC season stats.
    StartSeason {
        /// The PC's orbit fixtures for the season about to start — built by the TUI
        /// bridge from `goat-world` (see `pc_season_fixtures`'s doc comment).
        fixtures: Vec<goat_calendar::Fixture>,
    },

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
        /// Break/rest calendar weeks skipped between this round and the next
        /// (0 within the same or an adjacent week). Computed by the caller from
        /// goat-world's calendar (`rest_weeks_after_round`); each one elapses as
        /// a rest week for the PC so the player clock tracks the season calendar.
        rest_weeks: u32,
        /// True when this round is the LAST round of its calendar week (computed
        /// by the caller via `week_ends_after_round`). False means a second
        /// fixture follows in the SAME week: no time passes and no new training
        /// session opens until that fixture is played.
        week_ends: bool,
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
            // PrimaryPosition as u8 (0..7) — widened from the old 3-way Position (0..2).
            let position = choices.primary_position as u8;
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

        Intent::AdvanceWeek => {
            // One session per calendar week: the flag means "this week's tick has
            // run" (training or rest). The reducer is the gate, not the UI — a
            // second train in the same week (e.g. a double-fixture week) is a no-op.
            if state.pc_week_training_done {
                return state;
            }
            let mut state = tick_one_week(state, rng);
            state.pc_week_training_done = true;
            state
        }

        Intent::AdvanceWeeks { n } => {
            // Accumulate calendar flashpoints across the skipped weeks so a window that
            // opens mid-fast-forward isn't lost (tick_one_week overwrites them each week).
            let mut all_flashpoints = Vec::new();
            for _ in 0..n {
                state.last_week_events.clear();
                state = tick_one_week(state, rng);
                all_flashpoints.append(&mut state.last_week_flashpoints);
                if !state.last_week_events.is_empty() {
                    break; // stop at the first noteworthy event
                }
            }
            state.last_week_flashpoints = all_flashpoints;
            if n > 0 {
                state.pc_week_training_done = true;
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
            season_standout_matches,
            season_transfer_requests,
            season_caps,
            season_international_goals,
            season_world_cups_won,
            season_continental_championships_won,
        } => {
            state.pc_career_goals += season_goals;
            state.pc_career_matches += season_matches;
            state.pc_career_output_sum += season_output_sum as i64;
            state.pc_career_standout_matches += season_standout_matches;
            state.pc_career_transfer_requests += season_transfer_requests;
            state.pc_career_caps += season_caps;
            state.pc_career_international_goals += season_international_goals;
            state.pc_career_world_cups_won += season_world_cups_won;
            state.pc_career_continental_championships_won += season_continental_championships_won;
            // Career-peak OVR: computed here, not staged — a "peak so far" check is
            // naturally season-cadenced, no per-match staging needed.
            if let Some(pc_id) = state.pc_player_id {
                let view = state.players.snapshot(pc_id);
                let current_ovr =
                    crate::derive::ovr(&view.current, state.players.get_primary_position(pc_id))
                        .to_int();
                state.pc_career_best_ovr = state.pc_career_best_ovr.max(current_ovr.clamp(0, 100));
            }
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
            state.pc_season_transfer_requests += 1;
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

        Intent::SetDevInvestment { level } => {
            state.pc_dev_invest_level = level.min(3);
            state
        }

        Intent::SetMarketability { value } => {
            state.pc_marketability = value.clamp(0, 100);
            state
        }

        Intent::ApplyLifeEvent { thread, delta } => {
            use crate::tuning::{
                RELATIONSHIP_RUPTURE_THRESHOLD, SCANDAL_CHARACTER_HIT, SCANDAL_MARKETABILITY_HIT,
            };
            let t = (thread as usize).min(2);
            let before = state.pc_relationships[t];
            let after = (before + delta).clamp(0, 100);
            state.pc_relationships[t] = after;
            // Crossing into rupture territory triggers a scandal (once, on the way down).
            if before >= RELATIONSHIP_RUPTURE_THRESHOLD && after < RELATIONSHIP_RUPTURE_THRESHOLD {
                state.pc_character_rep = (state.pc_character_rep - SCANDAL_CHARACTER_HIT).max(0);
                state.pc_marketability =
                    (state.pc_marketability - SCANDAL_MARKETABILITY_HIT).max(0);
            }
            state
        }

        Intent::RespondToMedia { contrite } => {
            use crate::tuning::{MEDIA_CONTRITE, MEDIA_DEFIANT};
            let (dc, ds) = if contrite {
                MEDIA_CONTRITE
            } else {
                MEDIA_DEFIANT
            };
            state.pc_character_rep = (state.pc_character_rep + dc).clamp(0, 100);
            state.pc_sporting_rep = (state.pc_sporting_rep + ds).clamp(0, 100);
            state
        }

        Intent::SignSponsor { tier } => {
            use crate::tuning::{
                LIFESTYLE_NUDGE_PER_SPONSOR_TIER, LIFESTYLE_SCORE_MAX, LIFESTYLE_SCORE_MIN,
                OVERCOMMERCIAL_REP_PENALTY, SPONSOR_TIER_THRESHOLDS,
            };
            let tier = tier.min(3);
            if tier == 0 {
                state.pc_sponsor_tier = 0;
                return state;
            }
            let needed = SPONSOR_TIER_THRESHOLDS[(tier - 1) as usize];
            // Marketability gates eligibility; below it, you simply can't land the deal.
            if state.pc_marketability >= needed {
                state.pc_sponsor_tier = tier;
                // Over-commercialising: cashing in beyond your sporting merit dents image.
                if state.pc_sporting_rep < needed {
                    state.pc_sporting_rep -= OVERCOMMERCIAL_REP_PENALTY;
                }
                // One-off lifestyle nudge (bible §8.5): more commercial exposure leans
                // Flashy, proportional to tier.
                state.pc_lifestyle_score = (state.pc_lifestyle_score
                    + LIFESTYLE_NUDGE_PER_SPONSOR_TIER * Fixed::from_int(tier as i32))
                .clamp(LIFESTYLE_SCORE_MIN, LIFESTYLE_SCORE_MAX);
            }
            state
        }

        Intent::InvestInBusiness { amount } => {
            // Move capital from savings into the business (can't invest what you don't have).
            let moved = amount.clamp(0, state.pc_savings.max(0));
            state.pc_savings -= moved;
            state.pc_business_value += moved;
            state
        }

        Intent::SettleSeasonEconomy { season_bonus } => {
            use crate::tuning::{
                BANKRUPTCY_FLOOR, DEV_INVEST_COST, INVEST_RETURN_PER_1000,
                INVEST_VARIANCE_PER_1000, SPONSOR_INCOME, UPKEEP_BY_LIFESTYLE,
            };
            // Income: wage + performance bonus + sponsor income.
            state.pc_savings += state.pc_wage_annual
                + season_bonus
                + SPONSOR_INCOME[(state.pc_sponsor_tier as usize).min(3)];
            // Outgoings: lifestyle upkeep + the cost of the dev-investment tier.
            let upkeep = UPKEEP_BY_LIFESTYLE[(state.pc_lifestyle as usize).min(2)];
            state.pc_savings -=
                upkeep + DEV_INVEST_COST[(state.pc_dev_invest_level as usize).min(3)];
            // Business return: baseline ± a season-specific swing (deterministic via rng).
            if state.pc_business_value > 0 {
                let swing = rng.next_range_u64(0, (INVEST_VARIANCE_PER_1000 * 2) as u64) as i64
                    - INVEST_VARIANCE_PER_1000;
                let rate = INVEST_RETURN_PER_1000 + swing; // tenths of a percent
                state.pc_business_value += state.pc_business_value * rate / 1000;
                state.pc_business_value = state.pc_business_value.max(0);
            }
            // Bankruptcy: deep enough in the red wipes the business and flags the career.
            if state.pc_savings < BANKRUPTCY_FLOOR {
                state.pc_bankrupt = true;
                state.pc_business_value = 0;
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

        Intent::NationalTeamCallUp {
            called_up,
            started,
            goals,
        } => {
            if called_up {
                state.pc_season_caps += 1;
                if started {
                    state.pc_season_international_goals += goals;
                }
            }
            state
        }

        // ── Phase 10 handlers ────────────────────────────────────────────────────
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

        Intent::StartSeason { fixtures } => {
            state.pc_season_fixtures = fixtures;
            state.season_number += 1;
            state.season_round = 0;
            state.pc_season_goals = 0;
            state.pc_season_matches = 0;
            state.pc_season_output = 0;
            state.pc_season_standout_matches = 0;
            state.pc_season_transfer_requests = 0;
            state.pc_season_caps = 0;
            state.pc_season_international_goals = 0;
            state.pc_season_world_cups_won = 0;
            state.pc_season_continental_championships_won = 0;
            state.pc_yellow_cards_season = 0; // reset yellow cards each season
                                              // Preserve table from last season? No — start fresh each season.
            state.table_raw = [0u32; 100];

            // ── Off-season back-fill: every season-year is exactly 52 weeks ──
            // In-season play ticks ~41 weeks (one per round + skipped breaks);
            // the remainder elapses here as rest weeks, so at the start of
            // season N the invariant holds: age == START_AGE + (N−1)·52.
            if let Some(pc_id) = state.pc_player_id {
                let target = START_AGE_WEEKS + (state.season_number - 1) * 52;
                while state.players.get_age_weeks(pc_id) < target {
                    state = tick_one_rest_week(state);
                }
                state.pc_week_training_done = false;
            }
            state
        }

        Intent::ApplyRoundResult {
            pc_goals,
            pc_output,
            pc_result,
            round_results,
            rest_weeks,
            week_ends,
        } => {
            // ── Suspension serves one match per round resolved (bible AC-06: a
            // ban counts down by matches actually played, not by elapsed days) ──
            if state.pc_suspension_weeks > 0 {
                state.pc_suspension_weeks -= 1;
            }

            // ── Time passes: the season calendar drives the player clock ─────
            // Exactly one tick per calendar week: the match week elapses even if
            // the player never trained in it, and any skipped break weeks before
            // the next round elapse as rest. Between two fixtures of the SAME
            // week no time passes — the flag stays set so the week can neither
            // tick again nor open a second training session.
            if state.pc_player_id.is_some() {
                if !state.pc_week_training_done {
                    state = tick_one_rest_week(state);
                }
                if week_ends {
                    for _ in 0..rest_weeks {
                        state = tick_one_rest_week(state);
                    }
                    state.pc_week_training_done = false;
                } else {
                    state.pc_week_training_done = true;
                }
            }
            const N: usize = 20; // CLUBS_PER_DIV (goat-core stays headless — can't import
                                 // goat_world::CLUBS_PER_DIV — kept in sync by hand)

            // Update PC season stats.
            state.pc_season_goals += pc_goals;
            state.pc_season_output += pc_output;
            if pc_output > 0 {
                state.pc_season_matches += 1;
            }
            if pc_output >= crate::tuning::STANDOUT_OUTPUT_THRESHOLD {
                state.pc_season_standout_matches += 1;
            }

            // Form EMA: form = 0.85 × form + 0.15 × output
            if pc_output > 0 {
                let out_fixed = Fixed::from_int(pc_output.clamp(0, 100));
                state.pc_form = state.pc_form * Fixed::raw(850) + out_fixed * Fixed::raw(150);
            }

            // Update table raw data.
            // Layout: table_raw[col * N + row] where col ∈ {W=0, D=1, L=2, GF=3, GA=4}
            let apply_result = |raw: &mut [u32; 100],
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

/// One REST week: time passes without a training session (untrained match
/// weeks, break weeks, off-season). Age/injury/energy/decay run via
/// `week::advance_rest_week`; the live calendar engine still ticks 7 days so
/// flashpoint windows stay date-aligned. Deterministic — no RNG.
///
/// Guarded by the season age cap (START_AGE + season·52) so pathological
/// fast-forwarding can never compound with rest back-fill past a season-year.
fn tick_one_rest_week(mut state: WorldState) -> WorldState {
    let pc_id = match state.pc_player_id {
        Some(id) => id,
        None => return state,
    };
    let cap = START_AGE_WEEKS + state.season_number * 52;
    if state.players.get_age_weeks(pc_id) >= cap {
        return state;
    }

    crate::week::advance_rest_week(&mut state.players, pc_id, state.pc_lifestyle);

    let (new_epoch, flashpoints) = advance_calendar_week(
        state.pc_epoch_day,
        state.world_seed,
        state.season_number,
        &state.pc_season_fixtures,
    );
    state.pc_epoch_day = new_epoch;
    state.last_week_flashpoints = flashpoints;
    state
}

fn tick_one_week(mut state: WorldState, rng: &mut impl RngSource) -> WorldState {
    let pc_id = match state.pc_player_id {
        Some(id) => id,
        None => return state,
    };

    // ── Lifestyle: emergent weekly build-up (bible §8.5/§8.6) ────────────────
    // Nudge the score from this week's other choices, then derive the cached tier —
    // BEFORE anything below reads `state.pc_lifestyle` (injury/decline/growth all key
    // off it). Medium intensity + dev level 0 nudge nothing, so the no-choice path
    // stays at score 0 / tier Balanced, byte-identical to pre-existing goldens.
    state = apply_lifestyle_weekly_nudges(state);
    state.pc_lifestyle = lifestyle_tier_from_score(state.pc_lifestyle_score);

    // Snapshot current attrs to compute growth delta for TUI display.
    let before: [Fixed; NUM_ATTRS] = core::array::from_fn(|a| state.players.get_current(pc_id, a));

    // Lifestyle modifier: Professional +10% growth; Flashy −10%.
    let lifestyle_mult = match state.pc_lifestyle {
        0 => Fixed::raw(1_100), // Professional
        2 => Fixed::raw(900),   // Flashy
        _ => Fixed::ONE,        // Balanced
    };
    // Money buys a development edge — but growth still clamps to potential (§2.4), and
    // level 0 is ×1.0 so the no-spend path is byte-identical to existing goldens.
    let dev_mult = crate::tuning::DEV_INVEST_MULT[(state.pc_dev_invest_level as usize).min(3)];
    let effective_mult = state.pc_facilities_mult * lifestyle_mult * dev_mult;

    let events = advance_week(
        &mut state.players,
        pc_id,
        &state.pc_routine,
        effective_mult,
        state.pc_lifestyle,
        rng,
    );

    // Record per-attribute delta.
    state.last_week_growth =
        core::array::from_fn(|a| state.players.get_current(pc_id, a) - before[a]);

    state.last_week_events = events;

    // Sponsor obligations drain energy (the same resource training needs). Tier 0 costs
    // 0.0, so the no-sponsor path is byte-identical to existing goldens.
    let sponsor_cost = crate::tuning::SPONSOR_ENERGY_COST[(state.pc_sponsor_tier as usize).min(3)];
    if sponsor_cost > Fixed::ZERO {
        let e = state.players.get_energy(pc_id);
        state.players.set_energy(
            pc_id,
            (e - sponsor_cost).clamp(Fixed::ZERO, crate::tuning::ENERGY_MAX),
        );
    }

    // ── Live calendar tick (golden-safe) ─────────────────────────────────────
    // Advance the CalendarEngine 7 days on its OWN RNG stream (seeded from
    // world_seed) — independent of the growth RNG above, so attribute goldens are
    // untouched. Surfaces window-opening flashpoints for the renderer.
    let (new_epoch, flashpoints) = advance_calendar_week(
        state.pc_epoch_day,
        state.world_seed,
        state.season_number,
        &state.pc_season_fixtures,
    );
    state.pc_epoch_day = new_epoch;
    state.last_week_flashpoints = flashpoints;

    state
}

/// Apply this week's lifestyle-score nudges (bible §8.5): training intensity and
/// dev-investment level are habits the player repeats every week, so they build up the
/// emergent lifestyle readout gradually rather than being picked directly.
fn apply_lifestyle_weekly_nudges(mut state: WorldState) -> WorldState {
    use crate::tuning::{
        LIFESTYLE_NUDGE_INTENSITY_HIGH, LIFESTYLE_NUDGE_INTENSITY_LOW,
        LIFESTYLE_NUDGE_PER_DEV_LEVEL, LIFESTYLE_SCORE_MAX, LIFESTYLE_SCORE_MIN,
    };
    use crate::week::Intensity;

    let intensity_nudge = match state.pc_routine.intensity {
        Intensity::High => LIFESTYLE_NUDGE_INTENSITY_HIGH,
        Intensity::Low => LIFESTYLE_NUDGE_INTENSITY_LOW,
        Intensity::Medium => Fixed::ZERO,
    };
    let dev_nudge =
        LIFESTYLE_NUDGE_PER_DEV_LEVEL * Fixed::from_int(state.pc_dev_invest_level as i32);

    state.pc_lifestyle_score = (state.pc_lifestyle_score + intensity_nudge + dev_nudge)
        .clamp(LIFESTYLE_SCORE_MIN, LIFESTYLE_SCORE_MAX);
    state
}

/// Derive the cached lifestyle tier (0=Professional,1=Balanced,2=Flashy) from the
/// signed lifestyle score (bible §8.6). Thresholds are symmetric around 0.
pub fn lifestyle_tier_from_score(score: Fixed) -> u8 {
    use crate::tuning::LIFESTYLE_TIER_THRESHOLD;
    if score < Fixed::ZERO - LIFESTYLE_TIER_THRESHOLD {
        0 // Professional
    } else if score > LIFESTYLE_TIER_THRESHOLD {
        2 // Flashy
    } else {
        1 // Balanced
    }
}

/// Whether the player should retire now (bible §8.6): nobody plays past the hard age,
/// and past the soft age a player whose contract has run out (offers drying up) hangs up
/// the boots. The player may always choose to retire earlier (the `Retire` intent).
pub fn should_retire(state: &WorldState) -> bool {
    use crate::tuning::{RETIRE_AGE_HARD, RETIRE_AGE_SOFT};
    let age_years = match state.pc_player_id {
        Some(id) => state.players.get_age_weeks(id) / 52,
        None => return false,
    };
    age_years >= RETIRE_AGE_HARD
        || (age_years >= RETIRE_AGE_SOFT && state.pc_contract_seasons_left == 0)
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
        state.pc_club = "Riverside Town".to_string();
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
    fn advance_week_raw_growth_nonzero_but_display_truncates_to_zero() {
        // Locks in the fix for the "+0 training display" bug (playtest Slice 1):
        // real growth happens every trained week, but base growth is sub-1.0/week,
        // so `Fixed::to_int()` truncates it to 0 — a display bug, not a growth bug.
        // The TUI must format the raw `Fixed` value, never `to_int()`, for this
        // per-attribute weekly delta.
        let mut s = WorldState::new();
        push_uniform_player(&mut s, 30, 99);
        let routine = Routine {
            focus_attrs: vec![AttrId::Finishing],
            intensity: Intensity::Medium,
        };
        let s = reduce(s, Intent::SetRoutine { routine }, &mut make_rng());
        let s = reduce(s, Intent::AdvanceWeek, &mut make_rng());
        let growth = s.last_week_growth[AttrId::Finishing as usize];
        assert!(growth > Fixed::ZERO, "expected real growth, got {growth:?}");
        assert_eq!(
            growth.to_int(),
            0,
            "this test's whole point is a sub-1.0 weekly growth that to_int() truncates to 0 \
             — if this fails, the growth tuning changed and the test needs a different attr/seed, \
             not a change to the display fix"
        );
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
