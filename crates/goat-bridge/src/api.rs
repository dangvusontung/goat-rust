//! goat-bridge — FFI layer for Flutter.
//!
//! Holds a singleton WorldState inside a Mutex.
//! All Dart-callable functions take plain scalars/Strings and return flat DTOs.
//! No simulation logic lives here — this is a thin translation layer.

#![allow(clippy::too_many_arguments)]

use std::sync::{Arc, Mutex};

use goat_core::{
    attrs::{AttrId, ATTR_NAMES, NUM_ATTRS},
    calendar_loop::LEAGUE_COMPETITION_ID,
    derive::{derive_attrs, ovr, role_rating},
    generation::CreationChoices,
    positions::PrimaryPosition,
    roles::{RoleId, NUM_ROLES},
    state::{reduce, Intent, PeerState, WorldState},
    week::{Intensity, Routine},
};
use goat_fixed::Fixed;
use goat_match::{
    beats::ScoreEvent,
    discipline::RefPersonality,
    sim::{advance_beat, auto_play_match, start_match, ActiveMatchState, BeatLibrary, MatchSetup},
};
use goat_meta::{
    all_rankings, compute_axes, compute_golden_boot, compute_player_of_year, compute_reputation,
    LegacyEvidence, AXIS_NAMES, SCHOOLS,
};
use goat_rng::{GoatRng, RngSource};
use goat_traits::PlayerTraits;
use goat_world::{
    format_match_date, format_week_header, is_break_week,
    promotion::{apply_season_end_for_nation, effective_league_clubs, sim_league_season},
    round_to_week, week_to_rounds,
    world::WorldGenesis,
    Table, BASE_CAREER_YEAR, ROUNDS_PER_SEASON, SEASON_CALENDAR_WEEKS,
};

/// Current membership of `league_id` in the live pipeline (A3.3): the persisted
/// promotion-advanced membership for the PC's nation's leagues, genesis-static
/// otherwise. The bridge's shared `Arc<WorldGenesis>` is never mutated — drift
/// lives only in `WorldState::pc_nation_membership`.
fn live_league_clubs(world: &WorldGenesis, state: &WorldState, league_id: usize) -> Vec<usize> {
    let pc_nation = world.leagues[state.pc_div_idx as usize].nation;
    effective_league_clubs(world, pc_nation, &state.pc_nation_membership, league_id)
}

// ── Global singleton ──────────────────────────────────────────────────────────

static GAME: Mutex<Option<WorldState>> = Mutex::new(None);
static ACTIVE_MATCH: Mutex<Option<ActiveMatchSession>> = Mutex::new(None);
static BEAT_LIB: Mutex<Option<BeatLibrary>> = Mutex::new(None);
/// Cache of the last generated `WorldGenesis`, keyed by `world_seed`. Building the
/// ~1,200-club world is not free, and every session-scoped function below is keyed off
/// a single `world_seed` (the one on the active `WorldState`), so a one-entry cache
/// avoids regenerating it on every call while staying correct if the seed ever changes
/// (e.g. loading a different save) — no `WorldGenesis` is ever persisted, it is always
/// rebuilt from the seed, same "seed is the universe" pattern the crate uses elsewhere.
static WORLD_CACHE: Mutex<Option<(u64, Arc<WorldGenesis>)>> = Mutex::new(None);

/// Fixed seed used only for the pre-game club/nation/league picker (`list_clubs`), which
/// has no `WorldState` (and therefore no real `world_seed`) to key off yet — `new_game`'s
/// own `seed` argument is what actually seeds the played world. Flutter should treat the
/// picker's club identities as flavor for club/nation/league *shape* (names, tiers,
/// strengths) rather than as guaranteed-stable ids across into the real game world.
const PICKER_WORLD_SEED: u64 = 0;

/// Get (building + caching if needed) the `WorldGenesis` for `world_seed`.
fn get_world(world_seed: u64) -> Arc<WorldGenesis> {
    let mut lock = WORLD_CACHE.lock().expect("world cache lock poisoned");
    if let Some((seed, world)) = lock.as_ref() {
        if *seed == world_seed {
            return Arc::clone(world);
        }
    }
    let world = Arc::new(WorldGenesis::generate(world_seed));
    *lock = Some((world_seed, Arc::clone(&world)));
    world
}

/// Bundled beats.json compiled into the binary as a fallback.
const BUNDLED_BEATS_JSON: &str = include_str!("../../../beats.json");

fn with_beat_lib<F, T>(f: F) -> T
where
    F: FnOnce(&BeatLibrary) -> T,
{
    let mut lock = BEAT_LIB.lock().expect("beat_lib lock poisoned");
    if lock.is_none() {
        *lock =
            Some(BeatLibrary::load(BUNDLED_BEATS_JSON).expect("bundled beats.json must be valid"));
    }
    f(lock.as_ref().unwrap())
}

fn with_state<F, T>(f: F) -> T
where
    F: FnOnce(&WorldState) -> T,
{
    let lock = GAME.lock().expect("state lock poisoned");
    f(lock
        .as_ref()
        .expect("game not initialised — call new_game first"))
}

#[allow(dead_code)]
fn with_state_mut<F, T>(f: F) -> T
where
    F: FnOnce(&mut WorldState) -> T,
{
    let mut lock = GAME.lock().expect("state lock poisoned");
    f(lock
        .as_mut()
        .expect("game not initialised — call new_game first"))
}

fn set_state(s: WorldState) {
    *GAME.lock().expect("state lock poisoned") = Some(s);
}

fn take_state() -> WorldState {
    GAME.lock()
        .expect("state lock poisoned")
        .take()
        .expect("game not initialised")
}

struct ActiveMatchSession {
    state: ActiveMatchState,
    rng: GoatRng,
    round: usize,
    pc_club_id: usize,
    div_idx: usize,
    season: u32,
    world_seed: u64,
    opp_name: String,
}

// ── Data Transfer Objects ─────────────────────────────────────────────────────

/// Flat snapshot of game state returned to Dart after every mutation.
#[derive(Debug, Clone, PartialEq)]
pub struct GoatGameState {
    pub player_name: String,
    pub age_years: u32,
    pub age_weeks_in_year: u32,
    pub energy: i32, // 0–100
    pub ovr: i32,    // 0–100
    pub injury_weeks: u32,
    /// PrimaryPosition discriminant: 0=ST 1=W 2=WM 3=CAM 4=CM 5=DM 6=FB 7=CB.
    pub position: u8,
    pub club_name: String,
    pub div_name: String,
    pub nationality: String,

    // Season
    pub season_number: u32,
    pub season_round: u32,
    pub rounds_per_season: u32,
    pub form: i32, // 0–100
    pub season_goals: u32,
    pub season_matches: u32,
    pub is_suspended: bool,

    // Career finances & market
    pub contract_seasons_left: u32,
    pub wage_annual: i64,
    pub savings: i64,
    pub power_ladder: u8,

    // Discipline
    pub yellow_cards_season: u32,
    pub discipline_rep: i32,

    // Career stats
    pub career_goals: u32,
    pub career_matches: u32,
    pub career_seasons: u32,
    pub league_titles: u32,
    pub player_of_year_wins: u32,

    // Reputation
    pub sporting_rep: i32,
    pub club_fan_rep: i32,
    pub character_rep: i32, // inverse of discipline_rep

    // Lifestyle (0=Professional 1=Balanced 2=Flashy)
    pub lifestyle: u8,

    // Status flags
    pub is_retired: bool,
    pub has_rival: bool,
    pub rival_name: String,

    // Routine
    pub routine_intensity: u8,
    pub routine_focus_attr_names: Vec<String>,

    // Last week events
    pub last_events: Vec<String>,

    // Calendar
    /// 0-indexed calendar week within the season (derived from season_round).
    pub calendar_week: u32,
    /// Total calendar weeks in a season (always SEASON_CALENDAR_WEEKS = 38).
    pub calendar_weeks: u32,
    /// "Game Week 5 · Sep 2025"
    pub calendar_week_label: String,
    /// True if the current week is a break (no matches).
    pub is_break_week: bool,
    /// Matches scheduled for the current calendar week.
    pub week_fixtures: Vec<WeekFixtureDto>,
    /// How many of this week's matches have been played.
    pub week_fixtures_played: u32,
    /// True if the player has used their training session this calendar week.
    pub week_training_done: bool,
}

pub struct AttrDto {
    pub name: String,
    pub current: i32,
    pub potential: i32,
    pub family: String, // "pace","shooting","passing","dribbling","defending","physical"
}

pub struct FamilyDto {
    pub pace: i32,
    pub shooting: i32,
    pub passing: i32,
    pub dribbling: i32,
    pub defending: i32,
    pub physical: i32,
}

pub struct RoleDto {
    pub name: String,
    pub rating: i32,
    pub familiarity: String,
}

pub struct TableRowDto {
    pub position: u32,
    pub club_name: String,
    pub played: u32,
    pub won: u32,
    pub drawn: u32,
    pub lost: u32,
    pub goals_for: u32,
    pub goals_against: u32,
    pub points: u32,
    pub is_player_club: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchResultDto {
    pub player_output: i32,
    pub goals_for: u32,
    pub goals_against: u32,
    pub rating_label: String, // "★★★★☆" etc.
    pub moments: Vec<String>,
    pub yellow_cards: u32,
    pub red_card: bool,
}

pub struct LegacyAxisDto {
    pub name: String,
    pub value: i32,
}

pub struct SchoolRankingDto {
    pub school_name: String,
    pub school_tagline: String,
    pub score: i32,
    pub rank: u32,
    pub total: u32,
}

pub struct LegacyDto {
    pub axes: Vec<LegacyAxisDto>,
    pub rankings: Vec<SchoolRankingDto>,
    pub reputation_label: String,
    pub sporting_rep: i32,
    pub character_rep: i32,
    pub club_fan_rep: i32,
}

pub struct ClubDto {
    pub club_id: u32,
    pub name: String,
    pub strength: u8,
    pub div_name: String,
    pub div_idx: u32,
}

pub struct AwardDto {
    pub award_name: String,
    pub winner_name: String,
    pub runner_up: String,
    pub pc_won: bool,
    pub pc_score: i32,
    pub winner_score: i32,
}

pub struct TransferOfferDto {
    pub club_id: u32,
    pub club_name: String,
    pub div_name: String,
    pub wage: i64,
    pub length: u32,
    pub strength: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeekFixtureDto {
    /// 0-indexed round index.
    pub round: u32,
    /// 0-indexed calendar week this round falls in (see goat_world::calendar).
    pub calendar_week: u32,
    pub opponent: String,
    pub opp_strength: u8,
    pub is_home: bool,
    /// True if this round has already been played (round < season_round).
    pub played: bool,
    /// "Sat 16 Aug 2025"
    pub date: String,
}

pub struct PeerDto {
    pub name: String,
    pub nationality: String,
    pub career_goals: u32,
    pub career_matches: u32,
    pub avg_output: u8,
    pub titles: u32,
    pub is_rival: bool,
}

pub struct BeatChoiceDto {
    pub text: String,
    pub primary_attr: String,
    pub difficulty: u8,
}

pub struct ActiveBeatDto {
    pub beat_id: u32,
    pub setup: String,
    pub choices: Vec<BeatChoiceDto>,
    pub minute: u32,
    pub player_output: i32,
    pub goals_for: u32,
    pub goals_against: u32,
    pub stamina: i32,
    pub beat_number: u32,
    pub total_beats: u32,
    pub opp_name: String,
}

pub struct BeatOutcomeDto {
    pub success: bool,
    pub outcome_text: String,
    pub output_delta: i32,
    pub goal_for: bool,
    pub goal_against: bool,
    pub yellow_card: bool,
    pub red_card: bool,
    pub player_output: i32,
    pub goals_for: u32,
    pub goals_against: u32,
    pub is_complete: bool,
    pub next_beat: Option<ActiveBeatDto>,
    pub final_result: Option<MatchResultDto>,
    pub game_state: Option<GoatGameState>,
}

// ── Builder helpers ───────────────────────────────────────────────────────────

/// Family-representative match role for a specific position (0..7 PrimaryPosition
/// discriminant). Deploying the player in their exact role (Winger vs Poacher …)
/// is the TASK-CORE-creation-role-choice follow-up; until then the match engine
/// uses one anchor role per broad family, matching pre-revision behaviour.
fn match_role_for_position(pc_position: u8) -> RoleId {
    use goat_core::roles::PositionFamily;
    match PrimaryPosition::from_u8(pc_position)
        .unwrap_or(PrimaryPosition::ST)
        .family()
    {
        PositionFamily::Defender => RoleId::CentreBack,
        PositionFamily::Midfielder => RoleId::CentralMid,
        PositionFamily::Forward => RoleId::CompleteForward,
    }
}

fn build_game_state(s: &WorldState) -> GoatGameState {
    let pc_id = match s.pc_player_id {
        Some(id) => id,
        None => {
            return GoatGameState {
                player_name: String::new(),
                age_years: 0,
                age_weeks_in_year: 0,
                energy: 100,
                ovr: 0,
                injury_weeks: 0,
                position: s.pc_position,
                club_name: s.pc_club.to_string(),
                div_name: String::new(),
                nationality: s.pc_nationality.to_string(),
                season_number: s.season_number,
                season_round: s.season_round,
                rounds_per_season: ROUNDS_PER_SEASON as u32,
                form: s.pc_form.to_int(),
                season_goals: s.pc_season_goals,
                season_matches: s.pc_season_matches,
                is_suspended: s.pc_suspension_matches_remaining(LEAGUE_COMPETITION_ID) > 0,
                contract_seasons_left: s.pc_contract_seasons_left,
                wage_annual: s.pc_wage_annual,
                savings: s.pc_savings,
                power_ladder: s.pc_power_ladder,
                yellow_cards_season: s.pc_yellow_cards_season,
                discipline_rep: s.pc_discipline_rep,
                career_goals: s.pc_career_goals,
                career_matches: s.pc_career_matches,
                career_seasons: s.pc_seasons_played,
                league_titles: s.pc_league_titles,
                player_of_year_wins: s.pc_player_of_year_wins,
                sporting_rep: s.pc_sporting_rep,
                club_fan_rep: s.pc_club_fan_rep,
                character_rep: 100 - s.pc_discipline_rep,
                lifestyle: s.pc_lifestyle,
                is_retired: s.pc_retired,
                has_rival: s.pc_rival_idx.is_some(),
                rival_name: String::new(),
                routine_intensity: s.pc_routine.intensity as u8,
                routine_focus_attr_names: s
                    .pc_routine
                    .focus_attrs
                    .iter()
                    .map(|&a| ATTR_NAMES[a as usize].to_string())
                    .collect(),
                last_events: Vec::new(),
                calendar_week: 0,
                calendar_weeks: SEASON_CALENDAR_WEEKS as u32,
                calendar_week_label: String::new(),
                is_break_week: false,
                week_fixtures: Vec::new(),
                week_fixtures_played: 0,
                week_training_done: false,
            };
        }
    };

    let view = s.players.snapshot(pc_id);
    let _fam = derive_attrs(&view.current);
    let ovr = ovr(&view.current, view.primary_position).to_int();

    let rival_name = s
        .pc_rival_idx
        .and_then(|i| s.pc_peers.get(i))
        .map(|p| p.name.clone())
        .unwrap_or_default();

    let world = get_world(s.world_seed);
    let div_name = if s.season_number > 0 {
        world
            .leagues
            .get(s.pc_div_idx as usize)
            .map(|l| l.name.clone())
            .unwrap_or_default()
    } else {
        String::new()
    };

    let events: Vec<String> = s
        .last_week_events
        .iter()
        .map(|e| match e {
            goat_core::week::DevelopmentEvent::Injury { weeks } => {
                format!("INJURY — out for {weeks} week(s)")
            }
            goat_core::week::DevelopmentEvent::Illness { weeks } => {
                format!("ILLNESS — reduced capacity for {weeks} week(s)")
            }
            goat_core::week::DevelopmentEvent::Breakthrough { attr, .. } => {
                format!("BREAKTHROUGH! {}", ATTR_NAMES[*attr as usize])
            }
            goat_core::week::DevelopmentEvent::FamiliarityUpgrade { role, new_tier } => {
                format!("Role up: {} → {}", role.name(), new_tier.name())
            }
        })
        .collect();

    // ── Calendar ─────────────────────────────────────────────────────────────
    let cal_week = if s.season_number > 0 {
        round_to_week(s.season_round.min(ROUNDS_PER_SEASON as u32 - 1) as usize)
    } else {
        0
    };
    let season_year = BASE_CAREER_YEAR + s.season_number.saturating_sub(1);
    let cal_week_label = if s.season_number > 0 {
        format_week_header(season_year, cal_week)
    } else {
        String::new()
    };
    let break_week = s.season_number > 0 && is_break_week(cal_week);
    let week_rounds = week_to_rounds(cal_week);
    let week_fixtures_played = s.season_round.saturating_sub(week_rounds.start as u32);

    let mut week_fixtures: Vec<WeekFixtureDto> = Vec::new();
    if s.season_number > 0 {
        let div_idx = s.pc_div_idx as usize;
        let pc_club_id = s.pc_club_idx as usize;
        let div_clubs = &live_league_clubs(&world, s, div_idx);
        for (slot, round_idx) in week_rounds.clone().enumerate() {
            let fixture = goat_world::fixture_for_round(
                s.world_seed,
                s.season_number,
                div_idx,
                div_clubs,
                pc_club_id,
                round_idx,
            );
            if let Some(f) = fixture {
                let is_home = f.home == pc_club_id;
                let opp_id = if is_home { f.away } else { f.home };
                week_fixtures.push(WeekFixtureDto {
                    round: round_idx as u32,
                    calendar_week: cal_week as u32,
                    opponent: world.clubs[opp_id].name.to_string(),
                    opp_strength: world.clubs[opp_id].strength,
                    is_home,
                    played: (round_idx as u32) < s.season_round,
                    date: format_match_date(season_year, cal_week, slot),
                });
            }
        }
    }

    GoatGameState {
        player_name: view.name.clone(),
        age_years: view.age_weeks / 52,
        age_weeks_in_year: view.age_weeks % 52,
        energy: view.energy.to_int(),
        ovr,
        injury_weeks: view.injury_weeks,
        position: s.pc_position,
        club_name: s.pc_club.to_string(),
        div_name,
        nationality: s.pc_nationality.to_string(),
        season_number: s.season_number,
        season_round: s.season_round,
        rounds_per_season: ROUNDS_PER_SEASON as u32,
        form: s.pc_form.to_int(),
        season_goals: s.pc_season_goals,
        season_matches: s.pc_season_matches,
        is_suspended: s.pc_suspension_matches_remaining(LEAGUE_COMPETITION_ID) > 0,
        contract_seasons_left: s.pc_contract_seasons_left,
        wage_annual: s.pc_wage_annual,
        savings: s.pc_savings,
        power_ladder: s.pc_power_ladder,
        yellow_cards_season: s.pc_yellow_cards_season,
        discipline_rep: s.pc_discipline_rep,
        career_goals: s.pc_career_goals,
        career_matches: s.pc_career_matches,
        career_seasons: s.pc_seasons_played,
        league_titles: s.pc_league_titles,
        player_of_year_wins: s.pc_player_of_year_wins,
        sporting_rep: s.pc_sporting_rep,
        club_fan_rep: s.pc_club_fan_rep,
        character_rep: 100 - s.pc_discipline_rep,
        lifestyle: s.pc_lifestyle,
        is_retired: s.pc_retired,
        has_rival: s.pc_rival_idx.is_some(),
        rival_name,
        routine_intensity: s.pc_routine.intensity as u8,
        routine_focus_attr_names: s
            .pc_routine
            .focus_attrs
            .iter()
            .map(|&a| ATTR_NAMES[a as usize].to_string())
            .collect(),
        last_events: events,
        calendar_week: cal_week as u32,
        calendar_weeks: SEASON_CALENDAR_WEEKS as u32,
        calendar_week_label: cal_week_label,
        is_break_week: break_week,
        week_fixtures,
        week_fixtures_played,
        week_training_done: s.pc_week_training_done,
    }
}

fn beat_to_dto(
    beat: &goat_match::beats::GeneratedBeat,
    ms: &ActiveMatchState,
    opp_name: &str,
) -> ActiveBeatDto {
    ActiveBeatDto {
        beat_id: 0, // situation id is now a string; use 0 as placeholder
        setup: beat.setup.clone(),
        choices: beat
            .choices
            .iter()
            .map(|c| BeatChoiceDto {
                text: c.text.clone(),
                primary_attr: ATTR_NAMES[c.primary as usize].to_string(),
                difficulty: c.difficulty,
            })
            .collect(),
        minute: ms.current_minute(),
        player_output: ms.player_output,
        goals_for: ms.goals_for,
        goals_against: ms.goals_against,
        stamina: ms.stamina.to_int(),
        beat_number: ms.beat_idx as u32 + 1,
        total_beats: ms.beats.len() as u32,
        opp_name: opp_name.to_string(),
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// True if a game is currently loaded in memory.
pub fn has_active_game() -> bool {
    GAME.lock().map(|g| g.is_some()).unwrap_or(false)
}

/// Load the beat library from a JSON string (call once at app start with the bundled asset).
pub fn load_beat_library(json: String) -> bool {
    match BeatLibrary::load(&json) {
        Ok(lib) => {
            *BEAT_LIB.lock().expect("beat_lib lock poisoned") = Some(lib);
            true
        }
        Err(_) => false,
    }
}

/// List all clubs across all divisions for the new-game picker.
pub fn list_clubs() -> Vec<ClubDto> {
    let world = get_world(PICKER_WORLD_SEED);
    let mut result = Vec::new();
    for (div_idx, league) in world.leagues.iter().enumerate() {
        for &club_id in &league.clubs {
            let club = &world.clubs[club_id];
            result.push(ClubDto {
                club_id: club_id as u32,
                name: club.name.to_string(),
                strength: club.strength,
                div_name: league.name.clone(),
                div_idx: div_idx as u32,
            });
        }
    }
    result
}

/// Start a new game. Returns the initial game state snapshot.
///
/// `position` accepts the full 0..7 `PrimaryPosition` range (widened from the old 0..2
/// broad-family range — bible §4 revision, TASK-CORE-creation-role-choice): 0=ST, 1=W,
/// 2=WM, 3=CAM, 4=CM, 5=DM, 6=FB, 7=CB. Out-of-range values default to ST.
///
/// `lifestyle` is accepted only for FFI binary compatibility with the existing
/// `frb_generated.rs` signature — it is now ignored. Lifestyle is derived (bible
/// §8.5/§8.6) from training intensity, dev investment and sponsor choices over the
/// career; read it back via the `lifestyle` field on `GoatGameState`.
pub fn new_game(
    player_name: String,
    position: u8,
    club_id: u32,
    seed: u64,
    _lifestyle: u8,
) -> GoatGameState {
    let club_id = club_id as usize;
    // The actual played world is generated from `seed` (the real game seed), not
    // `PICKER_WORLD_SEED` — `club_id` is treated as a stable index into whichever world
    // is generated (see `PICKER_WORLD_SEED`'s doc comment above).
    let world = get_world(seed);
    let club = &world.clubs[club_id];

    // Find which division this club belongs to.
    let div_idx = world.club_league(club_id);
    let nationality = world.nation_name(club.nation).to_string();

    let primary_position = PrimaryPosition::from_u8(position).unwrap_or(PrimaryPosition::ST);

    let choices = CreationChoices {
        name: player_name,
        primary_position,
        nationality: nationality.clone(),
        club: club.name.clone(),
    };

    let mut state = WorldState::new();
    state = reduce(
        state,
        Intent::CreatePlayer { seed, choices },
        &mut GoatRng::new(0),
    );
    state = reduce(
        state,
        Intent::InitWorld {
            world_seed: seed,
            pc_club_idx: club_id as u16,
            pc_div_idx: div_idx as u8,
            facilities_mult: club.facilities_mult(),
            initial_table: Box::new([0u32; 100]),
        },
        &mut GoatRng::new(0),
    );

    // Seed peer cohort.
    let peers = build_peers(seed, &nationality);
    state = reduce(state, Intent::InitPeers { peers }, &mut GoatRng::new(0));
    state = reduce(
        state,
        Intent::StartSeason { fixtures: vec![] },
        &mut GoatRng::new(0),
    );

    let snapshot = build_game_state(&state);
    set_state(state);
    snapshot
}

fn build_peers(world_seed: u64, nationality: &str) -> Vec<PeerState> {
    let eng_names = [
        "J. Smith",
        "T. Williams",
        "O. Brown",
        "L. Taylor",
        "E. Jones",
        "C. Davis",
        "M. Wilson",
        "A. Moore",
    ];
    let bra_names = [
        "R. Silva",
        "G. Santos",
        "F. Oliveira",
        "M. Souza",
        "L. Costa",
        "P. Ferreira",
        "A. Alves",
        "D. Lima",
    ];
    let names = if nationality == "Brazil" {
        &bra_names
    } else {
        &eng_names
    };
    let mut rng = GoatRng::new(world_seed ^ 0x00C0_CAFE_BEEF_u64);
    (0..8)
        .map(|i| PeerState {
            seed: rng.next_u64(),
            name: names[i].to_string(),
            nationality: nationality.to_string(),
            career_goals: 0,
            career_matches: 0,
            avg_output: 0,
            titles: 0,
        })
        .collect()
}

/// Advance one training week. Returns updated state.
pub fn advance_week() -> GoatGameState {
    let state = take_state();
    let pc_id = state.pc_player_id.unwrap_or(0);
    let view = state.players.snapshot(pc_id);
    let seed = (view.age_weeks as u64).wrapping_mul(6364136223846793005);
    let state = reduce(state, Intent::AdvanceWeek, &mut GoatRng::new(seed));
    let snap = build_game_state(&state);
    set_state(state);
    snap
}

/// True if the club's current calendar week has a scheduled fixture that hasn't
/// been played yet. `AdvanceWeeks` never touches `season_round` (only playing a
/// match does), so this can only be true at call time — it can't newly become
/// true partway through the skipped weeks.
fn current_week_has_pending_match(state: &WorldState) -> bool {
    if state.season_number == 0 || state.season_round >= ROUNDS_PER_SEASON as u32 {
        return false;
    }
    let cal_week = round_to_week(state.season_round.min(ROUNDS_PER_SEASON as u32 - 1) as usize);
    if is_break_week(cal_week) {
        return false;
    }
    let week_rounds = week_to_rounds(cal_week);
    let played = state.season_round.saturating_sub(week_rounds.start as u32) as usize;
    played < week_rounds.len()
}

/// Advance up to `n` weeks, stopping at notable events.
///
/// Caps the actual number of weeks ticked to at most 1 while this week's
/// fixture(s) are unplayed (0 if this week's training is already done too) —
/// otherwise the player's age/development clock (ticked here) would drift
/// arbitrarily far ahead of the match calendar (only advanced by actually
/// playing), leaving fixtures stuck unplayed indefinitely with no way back.
/// A single extra tick is still allowed so this doesn't regress the ordinary
/// "train, then play this week's match" flow — only unbounded multi-week
/// skips past an unresolved fixture are blocked.
pub fn advance_weeks(n: u32) -> GoatGameState {
    let state = take_state();
    let pending = current_week_has_pending_match(&state);
    let effective_n = if pending {
        if state.pc_week_training_done {
            0
        } else {
            n.min(1)
        }
    } else {
        n
    };
    let capped = effective_n < n;

    let pc_id = state.pc_player_id.unwrap_or(0);
    let view = state.players.snapshot(pc_id);
    let seed = (view.age_weeks as u64).wrapping_mul(6364136223846793005);
    let state = reduce(
        state,
        Intent::AdvanceWeeks { n: effective_n },
        &mut GoatRng::new(seed),
    );
    let mut snap = build_game_state(&state);
    if capped {
        snap.last_events
            .push("Play this week's match(es) to keep fast-forwarding.".to_string());
    }
    set_state(state);
    snap
}

/// Set weekly training routine. `attr_ids` are AttrId discriminants (0..29).
pub fn set_routine(attr_ids: Vec<u8>, intensity: u8) -> GoatGameState {
    let focus_attrs: Vec<AttrId> = attr_ids
        .iter()
        .filter(|&&b| (b as usize) < NUM_ATTRS)
        .map(|&b| goat_core::attrs::AttrId::ALL[b as usize])
        .collect();
    let intensity = match intensity {
        0 => Intensity::Low,
        2 => Intensity::High,
        _ => Intensity::Medium,
    };
    let routine = Routine {
        focus_attrs,
        intensity,
    };
    let state = take_state();
    let state = reduce(state, Intent::SetRoutine { routine }, &mut GoatRng::new(0));
    let snap = build_game_state(&state);
    set_state(state);
    snap
}

/// Auto-play or skip the current season round.
pub fn play_round(interactive: bool) -> (GoatGameState, MatchResultDto) {
    let _ = interactive; // For now always auto-play (interactive beats need separate flow)
    let state = take_state();

    if state.season_round >= ROUNDS_PER_SEASON as u32 {
        let snap = build_game_state(&state);
        set_state(state);
        return (
            snap,
            MatchResultDto {
                player_output: 0,
                goals_for: 0,
                goals_against: 0,
                rating_label: String::new(),
                moments: Vec::new(),
                yellow_cards: 0,
                red_card: false,
            },
        );
    }

    let round = state.season_round as usize;
    let old_cal_week = round_to_week(round.min(ROUNDS_PER_SEASON - 1));
    let season = state.season_number;
    let div_idx = state.pc_div_idx as usize;
    let pc_club_id = state.pc_club_idx as usize;
    let world_seed = state.world_seed;
    let world = get_world(world_seed);
    let div_clubs = live_league_clubs(&world, &state, div_idx);

    let match_seed = world_seed ^ ((season as u64) << 32) ^ (round as u64) ^ 0xc0ffee;
    let mut match_rng = GoatRng::new(match_seed);

    // A suspended player's club still plays the fixture, but the PC doesn't
    // personally take part (bible AC-06: the ban is served by the round being
    // played, not by the PC's own participation).
    let is_suspended = state.pc_suspension_matches_remaining(LEAGUE_COMPETITION_ID) > 0;

    // Sim PC match.
    let pc_fixture =
        goat_world::fixture_for_round(world_seed, season, div_idx, &div_clubs, pc_club_id, round);
    let (pc_goals, pc_assists, pc_decisive, pc_clutch, pc_output, pc_result, match_dto) =
        if let Some(f) = pc_fixture {
            let is_home = f.home == pc_club_id;
            let opp_id = if is_home { f.away } else { f.home };
            let opp = &world.clubs[opp_id];
            let own_str = world.clubs[pc_club_id].strength;

            if is_suspended {
                let (gf, ga) = goat_world::sim_team_match(own_str, opp.strength, &mut match_rng);
                (
                    0,
                    0,
                    0,
                    0,
                    0,
                    0i8,
                    MatchResultDto {
                        player_output: 0,
                        goals_for: gf,
                        goals_against: ga,
                        rating_label: "SUSPENDED".to_string(),
                        moments: Vec::new(),
                        yellow_cards: 0,
                        red_card: false,
                    },
                )
            } else {
                let pc_id = state.pc_player_id.unwrap_or(0);
                let view = state.players.snapshot(pc_id);

                let ref_personality = {
                    let mut rp_rng = GoatRng::new(match_seed ^ 0xBADCAFE);
                    RefPersonality::from_rng(&mut rp_rng)
                };

                let setup = MatchSetup {
                    player_role: match_role_for_position(state.pc_position),
                    player_attrs: view.current,
                    player_familiarity: view.familiarity,
                    own_strength: own_str,
                    opp_strength: opp.strength,
                    opp_name: opp.name.clone(),
                    form: state.pc_form,
                    player_aggression: view.current[goat_core::attrs::AttrId::Aggression as usize]
                        .to_int()
                        .clamp(1, 99) as u8,
                    ref_personality,
                    dirty_rep: state.pc_discipline_rep,
                    player_traits: PlayerTraits::default(),
                };

                let result = with_beat_lib(|lib| auto_play_match(lib, setup, &mut match_rng));

                let stars = "★".repeat((result.player_output / 20 + 1).clamp(1, 5) as usize)
                    + &"☆".repeat(5 - (result.player_output / 20 + 1).clamp(1, 5) as usize);
                let moments: Vec<String> = result
                    .moments
                    .iter()
                    .filter(|m| m.goal_event.is_some() || m.success)
                    .take(5)
                    .map(|m| {
                        let icon = match m.goal_event {
                            Some(goat_match::beats::ScoreEvent::GoalFor) => "⚽",
                            Some(goat_match::beats::ScoreEvent::AssistFor) => "🅰",
                            Some(goat_match::beats::ScoreEvent::GoalAgainst) => "❌",
                            None => {
                                if m.success {
                                    "✓"
                                } else {
                                    "✗"
                                }
                            }
                        };
                        format!("{icon} {}'  {}", m.minute, m.outcome_text)
                    })
                    .collect();

                let pc_goals = result
                    .moments
                    .iter()
                    .filter(|m| {
                        matches!(m.goal_event, Some(goat_match::beats::ScoreEvent::GoalFor))
                    })
                    .count() as u32;
                let pc_assists = result
                    .moments
                    .iter()
                    .filter(|m| {
                        matches!(m.goal_event, Some(goat_match::beats::ScoreEvent::AssistFor))
                    })
                    .count() as u32;
                let pc_decisive = result
                    .moments
                    .iter()
                    .filter(|m| goat_match::sim::is_decisive(m))
                    .count() as u32;
                let pc_clutch = result
                    .moments
                    .iter()
                    .filter(|m| goat_match::sim::is_clutch(m))
                    .count() as u32;
                let pc_result: i8 = if result.goals_for > result.goals_against {
                    1
                } else if result.goals_for < result.goals_against {
                    -1
                } else {
                    0
                };

                (
                    pc_goals,
                    pc_assists,
                    pc_decisive,
                    pc_clutch,
                    result.player_output,
                    pc_result,
                    MatchResultDto {
                        player_output: result.player_output,
                        goals_for: result.goals_for,
                        goals_against: result.goals_against,
                        rating_label: stars,
                        moments,
                        yellow_cards: result.yellow_cards as u32,
                        red_card: result.red_card,
                    },
                )
            }
        } else {
            (
                0,
                0,
                0,
                0,
                0,
                0i8,
                MatchResultDto {
                    player_output: 0,
                    goals_for: 0,
                    goals_against: 0,
                    rating_label: String::new(),
                    moments: Vec::new(),
                    yellow_cards: 0,
                    red_card: false,
                },
            )
        };

    // Sim all round fixtures.
    let all_fixtures = goat_world::round_fixtures(world_seed, season, div_idx, &div_clubs, round);
    let sim_seed = world_seed ^ ((season as u64) << 32) ^ (round as u64) ^ 0xfeed;
    let mut sim_rng = GoatRng::new(sim_seed);
    let pc_fixture =
        goat_world::fixture_for_round(world_seed, season, div_idx, &div_clubs, pc_club_id, round);

    let mut round_results: Vec<(u8, u8, u32, u32)> = Vec::new();
    for f in &all_fixtures {
        let is_pc = f.home == pc_club_id || f.away == pc_club_id;
        let (gf, ga) = if is_pc {
            if let Some(ref pf) = pc_fixture {
                if pf.home == pc_club_id {
                    (match_dto.goals_for, match_dto.goals_against)
                } else {
                    (match_dto.goals_against, match_dto.goals_for)
                }
            } else {
                (0, 0)
            }
        } else {
            goat_world::sim_team_match(
                world.clubs[f.home].strength,
                world.clubs[f.away].strength,
                &mut sim_rng,
            )
        };
        let h_pos = div_clubs.iter().position(|&c| c == f.home).unwrap_or(0) as u8;
        let a_pos = div_clubs.iter().position(|&c| c == f.away).unwrap_or(0) as u8;
        round_results.push((h_pos, a_pos, gf, ga));
    }

    let mut state = state;
    if match_dto.yellow_cards > 0 || match_dto.red_card {
        state = reduce(
            state,
            Intent::ApplyMatchResult {
                familiarity_xp: [Fixed::ZERO; NUM_ROLES],
                energy_cost: Fixed::from_int(25),
                injury_weeks: None,
            },
            &mut GoatRng::new(0),
        );
        state = reduce(
            state,
            Intent::ApplyCardResult {
                competition_id: LEAGUE_COMPETITION_ID,
                yellow_cards: match_dto.yellow_cards,
                red_card: match_dto.red_card,
            },
            &mut GoatRng::new(0),
        );
    }
    state = reduce(
        state,
        Intent::ApplyRoundResult {
            competition_id: LEAGUE_COMPETITION_ID,
            pc_goals,
            pc_assists,
            pc_decisive_count: pc_decisive,
            pc_clutch_count: pc_clutch,
            fixture_importance: goat_core::state::FixtureImportance::League,
            pc_output,
            pc_result,
            round_results,
            rest_weeks: goat_world::rest_weeks_after_round(round),
            week_ends: goat_world::week_ends_after_round(round),
        },
        &mut GoatRng::new(0),
    );

    // Training flag is reset by the ApplyRoundResult reducer (the player can
    // train before each round). Update the stored calendar week if it changed.
    let new_cal_week = round_to_week((state.season_round as usize).min(ROUNDS_PER_SEASON - 1));
    if new_cal_week != old_cal_week {
        state.pc_current_calendar_week = new_cal_week as u32;
    }

    let snap = build_game_state(&state);
    set_state(state);
    (snap, match_dto)
}

/// Get every fixture for the player's club across the full season, not just
/// the current calendar week. Fixture generation is deterministic and pure
/// (see `goat_world::fixtures` — "never stored, always recomputed"), so the
/// entire season's schedule already exists the moment a season starts; this
/// just exposes it instead of recomputing only the current week's slice.
pub fn get_full_season_fixtures() -> Vec<WeekFixtureDto> {
    with_state(|s| {
        if s.season_number == 0 {
            return Vec::new();
        }
        let div_idx = s.pc_div_idx as usize;
        let pc_club_id = s.pc_club_idx as usize;
        let season_year = BASE_CAREER_YEAR + s.season_number.saturating_sub(1);
        let world = get_world(s.world_seed);
        let div_clubs = &live_league_clubs(&world, s, div_idx);

        goat_world::fixtures_for_club(
            s.world_seed,
            s.season_number,
            div_idx,
            div_clubs,
            pc_club_id,
        )
        .into_iter()
        .map(|f| {
            let cal_week = round_to_week(f.round);
            let slot = week_to_rounds(cal_week)
                .position(|r| r == f.round)
                .unwrap_or(0);
            let is_home = f.home == pc_club_id;
            let opp_id = if is_home { f.away } else { f.home };
            WeekFixtureDto {
                round: f.round as u32,
                calendar_week: cal_week as u32,
                opponent: world.clubs[opp_id].name.to_string(),
                opp_strength: world.clubs[opp_id].strength,
                is_home,
                played: (f.round as u32) < s.season_round,
                date: format_match_date(season_year, cal_week, slot),
            }
        })
        .collect()
    })
}

/// Get the current league table.
pub fn get_table() -> Vec<TableRowDto> {
    with_state(|s| {
        if s.season_number == 0 {
            return Vec::new();
        }
        let div_idx = s.pc_div_idx as usize;
        let world = get_world(s.world_seed);
        let div_clubs = &live_league_clubs(&world, s, div_idx);
        let table = Table::from_raw(&s.table_raw, div_clubs);
        let sorted = table.sorted();
        sorted
            .iter()
            .enumerate()
            .map(|(rank, e)| TableRowDto {
                position: rank as u32 + 1,
                club_name: world.clubs[e.club_id].name.to_string(),
                played: e.played(),
                won: e.w,
                drawn: e.d,
                lost: e.l,
                goals_for: e.gf,
                goals_against: e.ga,
                points: e.points(),
                is_player_club: e.club_id == s.pc_club_idx as usize,
            })
            .collect()
    })
}

/// Get all 30 attributes as DTOs.
pub fn get_attributes() -> Vec<AttrDto> {
    with_state(|s| {
        let pc_id = match s.pc_player_id {
            Some(id) => id,
            None => return Vec::new(),
        };
        let view = s.players.snapshot(pc_id);
        let family_names = [
            "pace",
            "pace",
            "shooting",
            "shooting",
            "shooting",
            "shooting",
            "shooting",
            "passing",
            "passing",
            "passing",
            "passing",
            "passing",
            "dribbling",
            "dribbling",
            "dribbling",
            "dribbling",
            "dribbling",
            "defending",
            "defending",
            "defending",
            "defending",
            "defending",
            "defending",
            "physical",
            "physical",
            "physical",
            "physical",
            "physical",
            "physical",
            "physical",
        ];
        (0..NUM_ATTRS)
            .map(|i| AttrDto {
                name: ATTR_NAMES[i].to_string(),
                current: view.current[i].to_int(),
                potential: view.potential[i].to_int(),
                family: family_names.get(i).copied().unwrap_or("other").to_string(),
            })
            .collect()
    })
}

/// Get family averages.
pub fn get_families() -> FamilyDto {
    with_state(|s| {
        let pc_id = match s.pc_player_id {
            Some(id) => id,
            None => {
                return FamilyDto {
                    pace: 0,
                    shooting: 0,
                    passing: 0,
                    dribbling: 0,
                    defending: 0,
                    physical: 0,
                }
            }
        };
        let view = s.players.snapshot(pc_id);
        let f = derive_attrs(&view.current);
        FamilyDto {
            pace: f.pace.to_int(),
            shooting: f.shooting.to_int(),
            passing: f.passing.to_int(),
            dribbling: f.dribbling.to_int(),
            defending: f.defending.to_int(),
            physical: f.physical.to_int(),
        }
    })
}

/// Get all 14 roles sorted by rating.
pub fn get_roles() -> Vec<RoleDto> {
    with_state(|s| {
        let pc_id = match s.pc_player_id {
            Some(id) => id,
            None => return Vec::new(),
        };
        let view = s.players.snapshot(pc_id);
        let mut roles: Vec<RoleDto> = RoleId::ALL
            .iter()
            .map(|&r| {
                let tier = view.familiarity[r as usize];
                RoleDto {
                    name: r.name().to_string(),
                    rating: role_rating(&view.current, r, tier).to_int(),
                    familiarity: tier.name().to_string(),
                }
            })
            .collect();
        roles.sort_by_key(|r| std::cmp::Reverse(r.rating));
        roles
    })
}

/// Get legacy axes and school rankings.
pub fn get_legacy() -> LegacyDto {
    with_state(|s| {
        let ev = LegacyEvidence {
            career_goals: s.pc_career_goals,
            career_matches: s.pc_career_matches,
            career_output_sum: s.pc_career_output_sum,
            best_season_avg_output: s.pc_best_season_avg_output,
            seasons_played: s.pc_seasons_played,
            decisive_moments: s.pc_decisive_moments,
            player_of_year_wins: s.pc_player_of_year_wins,
            league_titles: s.pc_league_titles,
            clubs_served: s.pc_clubs_served,
            longest_club_tenure: s.pc_longest_club_tenure,
            career_standout_matches: s.pc_career_standout_matches,
            career_best_ovr: s.pc_career_best_ovr,
            career_transfer_requests: s.pc_career_transfer_requests,
            career_caps: s.pc_career_caps,
            career_international_goals: s.pc_career_international_goals,
            career_world_cups_won: s.pc_career_world_cups_won,
            career_continental_championships_won: s.pc_career_continental_championships_won,
        };
        let axes = compute_axes(&ev);
        let rankings = all_rankings(&ev, &axes);
        let rep = compute_reputation(s.pc_sporting_rep, s.pc_discipline_rep, s.pc_club_fan_rep);

        LegacyDto {
            axes: AXIS_NAMES
                .iter()
                .zip(axes.as_array().iter())
                .map(|(&name, val)| LegacyAxisDto {
                    name: name.to_string(),
                    value: val.to_int(),
                })
                .collect(),
            rankings: rankings
                .iter()
                .enumerate()
                .map(|(i, &(score, rank, total))| SchoolRankingDto {
                    school_name: SCHOOLS[i].name.to_string(),
                    school_tagline: SCHOOLS[i].tagline.to_string(),
                    score: score.to_int(),
                    rank: rank as u32,
                    total: total as u32,
                })
                .collect(),
            reputation_label: rep.label().to_string(),
            sporting_rep: rep.sporting.to_int(),
            character_rep: rep.character.to_int(),
            club_fan_rep: rep.club_fan.to_int(),
        }
    })
}

/// Compute season awards (call after season ends).
pub fn get_season_awards() -> Vec<AwardDto> {
    with_state(|s| {
        let pc_id = match s.pc_player_id {
            Some(id) => id,
            None => return Vec::new(),
        };
        let view = s.players.snapshot(pc_id);
        let season_avg = if s.pc_season_matches > 0 {
            s.pc_season_output / s.pc_season_matches as i32
        } else {
            0
        };

        let poty = compute_player_of_year(
            &view.name,
            season_avg,
            s.pc_season_goals,
            s.season_number,
            s.world_seed,
        );
        let boot =
            compute_golden_boot(&view.name, s.pc_season_goals, s.season_number, s.world_seed);

        vec![poty, boot]
            .into_iter()
            .map(|a| AwardDto {
                award_name: a.award_name.to_string(),
                winner_name: a.winner_name.clone(),
                runner_up: a.runner_up_name.clone().unwrap_or_default(),
                pc_won: a.pc_won,
                pc_score: a.pc_score,
                winner_score: a.winner_score,
            })
            .collect()
    })
}

/// Apply end-of-season legacy update (call after rendering awards).
pub fn apply_season_end() -> GoatGameState {
    let state = take_state();
    let season_goals = state.pc_season_goals;
    let season_assists = state.pc_season_assists;
    let season_decisive = state.pc_season_decisive_moments;
    let season_clutch = state.pc_season_clutch_index;
    let season_matches = state.pc_season_matches;
    let season_output = state.pc_season_output;
    let season_standout_matches = state.pc_season_standout_matches;
    let season_transfer_requests = state.pc_season_transfer_requests;

    let div_idx = state.pc_div_idx as usize;
    let world = get_world(state.world_seed);
    let div_clubs = &live_league_clubs(&world, &state, div_idx);
    let table = Table::from_raw(&state.table_raw, div_clubs);
    let finish_pos = table.position_of(state.pc_club_idx as usize) as u32;
    let won_title = finish_pos == 1;

    let season_avg = if season_matches > 0 {
        season_output / season_matches as i32
    } else {
        0
    };
    let new_sporting =
        goat_meta::update_sporting_rep(state.pc_sporting_rep, season_avg, finish_pos);
    let new_club_fan = goat_meta::update_club_fan_rep(
        state.pc_club_fan_rep,
        state.pc_longest_club_tenure,
        season_avg,
        state.pc_clubs_served,
    );

    let pc_id = state.pc_player_id.unwrap_or(0);
    let view = state.players.snapshot(pc_id);
    let player_of_year = compute_player_of_year(
        &view.name,
        season_avg,
        season_goals,
        state.season_number,
        state.world_seed,
    )
    .pc_won;

    let mut state = state;
    state = reduce(state, Intent::CollectWage, &mut GoatRng::new(0));
    let season = state.season_number;
    state = reduce(
        state,
        Intent::BatchTickPeers { season },
        &mut GoatRng::new(0),
    );

    // Rival crystallisation
    if season >= 5 && state.pc_rival_idx.is_none() {
        let pc_avg = if state.pc_career_matches > 0 {
            (state.pc_career_output_sum / state.pc_career_matches as i64) as u8
        } else {
            0
        };
        let rival = state
            .pc_peers
            .iter()
            .enumerate()
            .find(|(_, p)| {
                p.career_matches >= 80 && (p.avg_output as i32 - pc_avg as i32).abs() <= 8
            })
            .map(|(i, _)| i);
        if let Some(rival_idx) = rival {
            state = reduce(
                state,
                Intent::DeclareRival {
                    peer_idx: rival_idx,
                    season,
                },
                &mut GoatRng::new(0),
            );
        }
    }

    state = reduce(
        state,
        Intent::ApplySeasonEndLegacy {
            season_goals,
            season_assists,
            season_matches,
            season_output_sum: season_output,
            won_title,
            player_of_year,
            finish_position: finish_pos,
            decisive_moments: season_decisive,
            season_clutch_index: season_clutch,
            new_sporting_rep: new_sporting,
            new_club_fan_rep: new_club_fan,
            season_standout_matches,
            season_transfer_requests,
            // National-team call-ups (Design round 2, Doc B §B.3) aren't wired into the
            // bridge layer yet — no international-break call-up flow exists on the
            // Flutter side, so nothing accrues caps this season.
            season_caps: 0,
            season_international_goals: 0,
            // National-team tournaments (Design round 4, Slice 4) aren't dispatched from
            // the bridge layer yet either (same deferred-to-Slice-5 gap).
            season_world_cups_won: 0,
            season_continental_championships_won: 0,
        },
        &mut GoatRng::new(0),
    );

    let snap = build_game_state(&state);
    set_state(state);
    snap
}

/// Start the next season.
pub fn start_next_season() -> GoatGameState {
    let mut state = take_state();

    // A3.3: promotion/relegation for the PC's nation at the season boundary —
    // the PC's league from the REAL played table (table_raw), the two sibling
    // leagues batch-simmed from static strengths. The bridge's shared Arc world
    // is never mutated: drift lives only in the persisted pc_nation_membership,
    // read back via `live_league_clubs` at every league-clubs read site (the
    // same rule the TUI applies to its session-owned world). Surfacing the
    // event list to the Flutter client is a DTO follow-up — the client can
    // already observe the new league composition directly.
    {
        let world = get_world(state.world_seed);
        let pc_league_id = state.pc_div_idx as usize;
        let nation = world.leagues[pc_league_id].nation;
        let mut nation_leagues: Vec<usize> = world
            .leagues
            .iter()
            .filter(|l| l.nation == nation)
            .map(|l| l.id)
            .collect();
        nation_leagues.sort_by_key(|&id| world.leagues[id].tier as usize);

        let mut membership: Vec<Vec<usize>> = world
            .leagues
            .iter()
            .map(|l| live_league_clubs(&world, &state, l.id))
            .collect();
        let mut tables: Vec<Table> = world
            .leagues
            .iter()
            .map(|l| Table::new(&membership[l.id]))
            .collect();
        for &id in &nation_leagues {
            tables[id] = if id == pc_league_id {
                Table::from_raw(&state.table_raw, &membership[id])
            } else {
                sim_league_season(
                    &world,
                    id,
                    &membership[id],
                    state.world_seed,
                    state.season_number,
                )
            };
        }
        let _events = apply_season_end_for_nation(
            &world,
            &mut membership,
            nation,
            state.season_number,
            &tables,
        );

        state.pc_nation_membership = nation_leagues
            .iter()
            .flat_map(|&id| membership[id].iter().map(|&c| c as u32))
            .collect();
        // The PC's club may have moved tiers — re-resolve its league.
        let pc_club = state.pc_club_idx as usize;
        if let Some(&new_league) = nation_leagues
            .iter()
            .find(|&&id| membership[id].contains(&pc_club))
        {
            state.pc_div_idx = new_league as u8;
        }
    }

    let state = reduce(
        state,
        Intent::StartSeason { fixtures: vec![] },
        &mut GoatRng::new(0),
    );
    let snap = build_game_state(&state);
    set_state(state);
    snap
}

/// Accept a transfer offer identified by club_id.
pub fn accept_transfer(club_id: u32, wage: i64, length: u32) -> GoatGameState {
    let club_id = club_id as usize;
    let world_seed = with_state(|s| s.world_seed);
    let world = get_world(world_seed);
    let club = &world.clubs[club_id];
    let div_idx = world.club_league(club_id);
    let fee_bonus = (world.clubs[with_state(|s| s.pc_club_idx) as usize].strength as i64) * 3;
    let state = take_state();
    let state = reduce(
        state,
        Intent::ExecuteTransfer {
            to_club_idx: club_id as u16,
            to_div_idx: div_idx as u8,
            new_wage: wage,
            new_length: length,
            new_club_name: club.name.clone(),
            facilities_mult: club.facilities_mult(),
            fee_bonus,
        },
        &mut GoatRng::new(0),
    );
    let snap = build_game_state(&state);
    set_state(state);
    snap
}

/// Agitate for a transfer (advance power ladder, burn rep).
pub fn agitate_for_transfer() -> GoatGameState {
    let state = take_state();
    let state = reduce(state, Intent::AgitateForTransfer, &mut GoatRng::new(0));
    let snap = build_game_state(&state);
    set_state(state);
    snap
}

/// Generate transfer offers for the current window.
pub fn get_transfer_offers() -> Vec<TransferOfferDto> {
    with_state(|s| {
        let pc_id = match s.pc_player_id {
            Some(id) => id,
            None => return Vec::new(),
        };
        let view = s.players.snapshot(pc_id);
        let form = s.pc_form.to_int();
        let age = view.age_weeks / 52;
        if form < 55 || age >= 34 {
            return Vec::new();
        }

        let world = get_world(s.world_seed);
        let mut rng = GoatRng::new(s.world_seed ^ ((s.season_number as u64) << 32) ^ 0xA11BEEF);
        let n = rng.next_range_u64(0, 2) as usize;
        let num_leagues = world.leagues.len() as u64;
        let mut offers = Vec::new();
        for _ in 0..n {
            let target_div = ((s.pc_div_idx as u64 + 1 + rng.next_range_u64(0, num_leagues - 2))
                % num_leagues) as usize;
            let league_clubs = &live_league_clubs(&world, s, target_div);
            let club_pos = rng.next_range_u64(0, league_clubs.len() as u64 - 1) as usize;
            let club_id = league_clubs[club_pos];
            if club_id == s.pc_club_idx as usize {
                continue;
            }
            let wage = s.pc_wage_annual
                + (world.clubs[club_id].strength as i64 * 2)
                + rng.next_range_u64(0, 50) as i64;
            let length = 2 + rng.next_range_u64(0, 2) as u32;
            offers.push(TransferOfferDto {
                club_id: club_id as u32,
                club_name: world.clubs[club_id].name.to_string(),
                div_name: world.leagues[target_div].name.clone(),
                wage,
                length,
                strength: world.clubs[club_id].strength,
            });
        }
        offers
    })
}

/// Get all peers in the cohort.
pub fn get_peers() -> Vec<PeerDto> {
    with_state(|s| {
        s.pc_peers
            .iter()
            .enumerate()
            .map(|(i, p)| PeerDto {
                name: p.name.clone(),
                nationality: p.nationality.to_string(),
                career_goals: p.career_goals,
                career_matches: p.career_matches,
                avg_output: p.avg_output,
                titles: p.titles,
                is_rival: s.pc_rival_idx == Some(i),
            })
            .collect()
    })
}

/// Retire the player.
pub fn retire() -> GoatGameState {
    let state = take_state();
    let state = reduce(state, Intent::Retire, &mut GoatRng::new(0));
    let snap = build_game_state(&state);
    set_state(state);
    snap
}

/// Save game to file at `path`.
pub fn save_game(path: String) -> bool {
    with_state(|s| {
        if let Some(pc_id) = s.pc_player_id {
            let view = s.players.snapshot(pc_id);
            let data = goat_save::save::from_world_state(s, &view);
            goat_save::save::save_to_file(&data, &path).is_ok()
        } else {
            false
        }
    })
}

/// Load game from file at `path`.
pub fn load_game(path: String) -> Option<GoatGameState> {
    match goat_save::save::load_from_file(&path) {
        Ok(data) => {
            let world = get_world(data.world_seed);
            let state = goat_save::save::to_world_state(&data, &world);
            let snap = build_game_state(&state);
            set_state(state);
            Some(snap)
        }
        Err(_) => None,
    }
}

/// Get a snapshot without mutating state.
pub fn get_state() -> Option<GoatGameState> {
    let lock = GAME.lock().ok()?;
    lock.as_ref().map(build_game_state)
}

/// Start an interactive match for the current season round.
/// Returns `None` if there is no fixture this round, or if the PC is
/// suspended — a suspended player doesn't personally take part (bible
/// AC-06); callers should fall back to `play_round`, which auto-sims the
/// club's match without the PC and serves one match of the ban.
pub fn start_interactive_match() -> Option<ActiveBeatDto> {
    let session = with_state(|s| -> Option<ActiveMatchSession> {
        if s.season_number == 0
            || s.season_round >= ROUNDS_PER_SEASON as u32
            || s.pc_suspension_matches_remaining(LEAGUE_COMPETITION_ID) > 0
        {
            return None;
        }
        let round = s.season_round as usize;
        let div_idx = s.pc_div_idx as usize;
        let pc_club_id = s.pc_club_idx as usize;
        let world_seed = s.world_seed;
        let season = s.season_number;
        let world = get_world(world_seed);
        let div_clubs = &live_league_clubs(&world, s, div_idx);

        let pc_fixture = goat_world::fixture_for_round(
            world_seed, season, div_idx, div_clubs, pc_club_id, round,
        )?;
        let is_home = pc_fixture.home == pc_club_id;
        let opp_id = if is_home {
            pc_fixture.away
        } else {
            pc_fixture.home
        };
        let opp = &world.clubs[opp_id];
        let own_str = world.clubs[pc_club_id].strength;

        let pc_id = s.pc_player_id?;
        let view = s.players.snapshot(pc_id);

        let match_seed = world_seed ^ ((season as u64) << 32) ^ (round as u64) ^ 0xc0ffee;
        let mut rng = GoatRng::new(match_seed);
        let ref_personality = {
            let mut rp_rng = GoatRng::new(match_seed ^ 0xBADCAFE);
            RefPersonality::from_rng(&mut rp_rng)
        };

        let setup = MatchSetup {
            player_role: match_role_for_position(s.pc_position),
            player_attrs: view.current,
            player_familiarity: view.familiarity,
            own_strength: own_str,
            opp_strength: opp.strength,
            opp_name: opp.name.clone(),
            form: s.pc_form,
            player_aggression: view.current[goat_core::attrs::AttrId::Aggression as usize]
                .to_int()
                .clamp(1, 99) as u8,
            ref_personality,
            dirty_rep: s.pc_discipline_rep,
            player_traits: PlayerTraits::default(),
        };

        let ms = with_beat_lib(|lib| start_match(lib, setup, &mut rng));
        Some(ActiveMatchSession {
            state: ms,
            rng,
            round,
            pc_club_id,
            div_idx,
            season,
            world_seed,
            opp_name: opp.name.clone(),
        })
    })?;

    // GAME lock dropped. Now store in ACTIVE_MATCH and return first beat.
    let beat = session.state.current_beat()?;
    let opp = session.opp_name.clone();
    let dto = beat_to_dto(beat, &session.state, &opp);
    *ACTIVE_MATCH.lock().unwrap() = Some(session);
    Some(dto)
}

/// Resolve a player choice for the current interactive match beat.
/// Returns `None` if no match is active.
pub fn make_beat_choice(choice_idx: u8) -> Option<BeatOutcomeDto> {
    let mut session = ACTIVE_MATCH.lock().unwrap().take()?;

    let prev_goals_for = session.state.goals_for;
    let prev_goals_against = session.state.goals_against;
    let prev_yellow_cards = session.state.yellow_cards;
    let prev_red_card = session.state.red_card;
    let prev_output = session.state.player_output;

    session.state = with_beat_lib(|lib| {
        advance_beat(
            session.state.clone(),
            choice_idx as usize,
            lib,
            &mut session.rng,
        )
    });

    let last = session.state.moments.last()?;
    let goal_for = session.state.goals_for > prev_goals_for;
    let goal_against = session.state.goals_against > prev_goals_against;
    let yellow_card = session.state.yellow_cards > prev_yellow_cards;
    let red_card = !prev_red_card && session.state.red_card;
    let output_delta = session.state.player_output - prev_output;
    let success = last.success;
    let outcome_text = last.outcome_text.to_string();
    let opp_name = session.opp_name.clone();

    if session.state.is_complete {
        let pc_goals = session
            .state
            .moments
            .iter()
            .filter(|m| matches!(m.goal_event, Some(ScoreEvent::GoalFor)))
            .count() as u32;
        let pc_assists = session
            .state
            .moments
            .iter()
            .filter(|m| matches!(m.goal_event, Some(ScoreEvent::AssistFor)))
            .count() as u32;
        let pc_decisive = session
            .state
            .moments
            .iter()
            .filter(|m| goat_match::sim::is_decisive(m))
            .count() as u32;
        let pc_clutch = session
            .state
            .moments
            .iter()
            .filter(|m| goat_match::sim::is_clutch(m))
            .count() as u32;
        let pc_output = session.state.player_output;
        let goals_for = session.state.goals_for;
        let goals_against = session.state.goals_against;
        let pc_result: i8 = if goals_for > goals_against {
            1
        } else if goals_for < goals_against {
            -1
        } else {
            0
        };
        let yc = session.state.yellow_cards;
        let rc = session.state.red_card;

        let stars = "★".repeat((pc_output / 20 + 1).clamp(1, 5) as usize)
            + &"☆".repeat(5 - (pc_output / 20 + 1).clamp(1, 5) as usize);
        let moment_texts: Vec<String> = session
            .state
            .moments
            .iter()
            .filter(|m| m.goal_event.is_some() || m.success)
            .take(5)
            .map(|m| {
                let icon = match m.goal_event {
                    Some(ScoreEvent::GoalFor) => "⚽",
                    Some(ScoreEvent::AssistFor) => "🅰",
                    Some(ScoreEvent::GoalAgainst) => "❌",
                    None => {
                        if m.success {
                            "✓"
                        } else {
                            "✗"
                        }
                    }
                };
                format!("{icon} {}'  {}", m.minute, m.outcome_text)
            })
            .collect();
        let match_dto = MatchResultDto {
            player_output: pc_output,
            goals_for,
            goals_against,
            rating_label: stars,
            moments: moment_texts,
            yellow_cards: yc as u32,
            red_card: rc,
        };

        let round = session.round;
        let pc_club_id = session.pc_club_id;
        let div_idx = session.div_idx;
        let season = session.season;
        let world_seed = session.world_seed;
        let world = get_world(world_seed);
        // Apply results to world state.
        let gs = {
            let state = take_state();
            let div_clubs = live_league_clubs(&world, &state, div_idx);
            let old_cal_week = round_to_week(round.min(ROUNDS_PER_SEASON - 1));
            let all_fixtures =
                goat_world::round_fixtures(world_seed, season, div_idx, &div_clubs, round);
            let sim_seed = world_seed ^ ((season as u64) << 32) ^ (round as u64) ^ 0xfeed;
            let mut sim_rng = GoatRng::new(sim_seed);
            let pc_fixture = goat_world::fixture_for_round(
                world_seed, season, div_idx, &div_clubs, pc_club_id, round,
            );

            let mut round_results: Vec<(u8, u8, u32, u32)> = Vec::new();
            for f in &all_fixtures {
                let is_pc = f.home == pc_club_id || f.away == pc_club_id;
                let (gf, ga) = if is_pc {
                    if let Some(ref pf) = pc_fixture {
                        if pf.home == pc_club_id {
                            (goals_for, goals_against)
                        } else {
                            (goals_against, goals_for)
                        }
                    } else {
                        (0, 0)
                    }
                } else {
                    goat_world::sim_team_match(
                        world.clubs[f.home].strength,
                        world.clubs[f.away].strength,
                        &mut sim_rng,
                    )
                };
                let h_pos = div_clubs.iter().position(|&c| c == f.home).unwrap_or(0) as u8;
                let a_pos = div_clubs.iter().position(|&c| c == f.away).unwrap_or(0) as u8;
                round_results.push((h_pos, a_pos, gf, ga));
            }

            let mut state = state;
            if yc > 0 || rc {
                state = reduce(
                    state,
                    Intent::ApplyMatchResult {
                        familiarity_xp: [Fixed::ZERO; NUM_ROLES],
                        energy_cost: Fixed::from_int(25),
                        injury_weeks: None,
                    },
                    &mut GoatRng::new(0),
                );
                state = reduce(
                    state,
                    Intent::ApplyCardResult {
                        competition_id: LEAGUE_COMPETITION_ID,
                        yellow_cards: yc as u32,
                        red_card: rc,
                    },
                    &mut GoatRng::new(0),
                );
            }
            state = reduce(
                state,
                Intent::ApplyRoundResult {
                    competition_id: LEAGUE_COMPETITION_ID,
                    pc_goals,
                    pc_assists,
                    pc_decisive_count: pc_decisive,
                    pc_clutch_count: pc_clutch,
                    fixture_importance: goat_core::state::FixtureImportance::League,
                    pc_output,
                    pc_result,
                    round_results,
                    rest_weeks: goat_world::rest_weeks_after_round(round),
                    week_ends: goat_world::week_ends_after_round(round),
                },
                &mut GoatRng::new(0),
            );

            // Training flag is reset by the ApplyRoundResult reducer.
            let new_cal_week =
                round_to_week((state.season_round as usize).min(ROUNDS_PER_SEASON - 1));
            if new_cal_week != old_cal_week {
                state.pc_current_calendar_week = new_cal_week as u32;
            }

            let snap = build_game_state(&state);
            set_state(state);
            snap
        };

        Some(BeatOutcomeDto {
            success,
            outcome_text,
            output_delta,
            goal_for,
            goal_against,
            yellow_card,
            red_card,
            player_output: pc_output,
            goals_for,
            goals_against,
            is_complete: true,
            next_beat: None,
            final_result: Some(match_dto),
            game_state: Some(gs),
        })
    } else {
        let player_output = session.state.player_output;
        let goals_for = session.state.goals_for;
        let goals_against = session.state.goals_against;
        let next_beat = session
            .state
            .current_beat()
            .map(|b| beat_to_dto(b, &session.state, &opp_name));
        *ACTIVE_MATCH.lock().unwrap() = Some(session);

        Some(BeatOutcomeDto {
            success,
            outcome_text,
            output_delta,
            goal_for,
            goal_against,
            yellow_card,
            red_card,
            player_output,
            goals_for,
            goals_against,
            is_complete: false,
            next_beat,
            final_result: None,
            game_state: None,
        })
    }
}
