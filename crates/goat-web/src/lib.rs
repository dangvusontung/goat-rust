//! goat-web — wasm-bindgen web API for the GOAT life-sim demo.
//!
//! Mirrors the session flow of `crates/goat-bridge/src/api.rs` (without the
//! flutter_rust_bridge dependency): a singleton `WorldState` behind a `Mutex`,
//! a one-entry `WorldGenesis` cache keyed by `world_seed`, and an interactive
//! match session for beat-by-beat play. All exported functions take/return
//! plain scalars, byte buffers, or JSON `String`s (no serde-wasm-bindgen).
//!
//! No simulation logic lives here — this is a thin translation layer over the
//! same core crates the bridge and TUI use.

#![forbid(unsafe_code)]

use std::sync::{Arc, Mutex};

use goat_core::{
    attrs::AttrId,
    calendar_loop::LEAGUE_COMPETITION_ID,
    generation::CreationChoices,
    positions::PrimaryPosition,
    roles::NUM_ROLES,
    state::{reduce, FixtureImportance, Intent, PeerState, WorldState},
};
use goat_fixed::Fixed;
use goat_match::{
    beats::ScoreEvent,
    discipline::RefPersonality,
    sim::{advance_beat, auto_play_match, start_match, ActiveMatchState, BeatLibrary, MatchSetup},
};
use goat_rng::{GoatRng, RngSource};
use goat_traits::PlayerTraits;
use goat_world::{
    format_week_header,
    promotion::{apply_season_end_for_nation, effective_league_clubs, sim_league_season},
    round_to_week,
    world::WorldGenesis,
    Table, PRE_SEASON_WEEKS, ROUNDS_PER_SEASON,
};
use serde::Serialize;
use wasm_bindgen::prelude::*;

// ── Global singletons ─────────────────────────────────────────────────────────

static GAME: Mutex<Option<WorldState>> = Mutex::new(None);
static ACTIVE_MATCH: Mutex<Option<ActiveMatchSession>> = Mutex::new(None);
static BEAT_LIB: Mutex<Option<BeatLibrary>> = Mutex::new(None);
/// PC's hidden trait ceilings, rolled from the creation seed (§A.3 aptitude) and
/// fed into every `MatchSetup`. Session-scoped, exactly like the TUI's
/// `pc_traits` — traits are never persisted into `WorldState`.
static PC_TRAITS: Mutex<Option<PlayerTraits>> = Mutex::new(None);
/// Cache of the last generated `WorldGenesis`, keyed by `world_seed` (see the
/// bridge's `get_world` — same "seed is the universe" pattern).
static WORLD_CACHE: Mutex<Option<(u64, Arc<WorldGenesis>)>> = Mutex::new(None);

/// Fixed seed used only for the pre-game nation picker (`get_nations`), which
/// has no seed argument of its own. Nation identities (names, order) are
/// seed-independent anyway — only stature rolls vary. `get_leagues`/`get_clubs`
/// take the real game seed, and `new_game`'s `seed` seeds the played world.
const PICKER_WORLD_SEED: u64 = 0;

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

/// Current membership of `league_id` in the live pipeline (A3.3): the persisted
/// promotion-advanced membership for the PC's nation's leagues, genesis-static
/// otherwise. Copied from the bridge's helper — the cached world is never
/// mutated; drift lives only in `WorldState::pc_nation_membership`.
fn live_league_clubs(world: &WorldGenesis, state: &WorldState, league_id: usize) -> Vec<usize> {
    let pc_nation = world.leagues[state.pc_div_idx as usize].nation;
    effective_league_clubs(world, pc_nation, &state.pc_nation_membership, league_id)
}

/// Bundled beats.json compiled into the binary.
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

struct ActiveMatchSession {
    state: ActiveMatchState,
    rng: GoatRng,
    round: usize,
    pc_club_id: usize,
    div_idx: usize,
    season: u32,
    world_seed: u64,
    opp_name: String,
    /// True for pre-season friendlies: on completion they apply ONLY
    /// `ApplyMatchResult` (development) — never `ApplyRoundResult`/cards, so no
    /// league table, season stats, or suspensions (mirrors the TUI's
    /// `run_friendly`; not a calendar fixture, nothing persisted).
    friendly: bool,
}

// ── JSON DTOs ─────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct NationDto {
    id: usize,
    name: String,
    stature: u8,
}

#[derive(Serialize)]
struct LeagueDto {
    id: usize,
    name: String,
    tier: usize,
}

#[derive(Serialize)]
struct ClubDto {
    id: usize,
    name: String,
    strength: u8,
}

#[derive(Serialize)]
struct TableRowDto {
    position: usize,
    club: String,
    played: u32,
    won: u32,
    drawn: u32,
    lost: u32,
    goals_for: u32,
    goals_against: u32,
    points: u32,
    is_player_club: bool,
}

#[derive(Serialize)]
struct StateSnapshot {
    player_name: String,
    club_name: String,
    league_name: String,
    nation_name: String,
    age_years: u32,
    energy: i32,
    form: i32,
    season_number: u32,
    season_round: u32,
    rounds_per_season: u32,
    week_label: String,
    season_goals: u32,
    season_assists: u32,
    season_decisive: u32,
    season_clutch: u32,
    trained_this_week: bool,
    season_over: bool,
    /// Pre-season (Jul-1 anchor, 7-week lead): true while the season's first
    /// league round hasn't come due yet. Friendlies only in this window.
    pre_season: bool,
    /// Current pre-season week (0..6); meaningless when `pre_season` is false.
    pre_season_week: u32,
    /// Current standing training routine, display-ready ("Finishing, Vision
    /// [Medium]" / "No focus [Medium]").
    routine_text: String,
    table: Vec<TableRowDto>,
}

#[derive(Serialize)]
struct BeatChoiceDto {
    index: usize,
    text: String,
}

#[derive(Serialize)]
struct BeatDto {
    setup: String,
    minute: u32,
    beat_number: usize,
    total_beats: usize,
    opp_name: String,
    player_output: i32,
    goals_for: u32,
    goals_against: u32,
    stamina: i32,
    choices: Vec<BeatChoiceDto>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn to_json<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).expect("DTO serialization is infallible")
}

/// The current grid week of the season, derived from the PC's age (mirrors the
/// TUI's `season_week`): the StartSeason back-fill pins age to
/// START_AGE + (N−1)·52 at every season start, so the difference IS the grid
/// week — pre-season weeks included.
fn season_week(s: &WorldState) -> u32 {
    match s.pc_player_id {
        Some(id) => {
            let start = goat_core::tuning::START_AGE_WEEKS + (s.season_number - 1) * 52;
            s.players.get_age_weeks(id).saturating_sub(start)
        }
        None => 0,
    }
}

/// True during the 7-week pre-season lead (before round 1's grid week).
fn in_pre_season(s: &WorldState) -> bool {
    s.season_number > 0 && s.season_round == 0 && season_week(s) < PRE_SEASON_WEEKS as u32
}

fn err_json(msg: impl std::fmt::Display) -> String {
    serde_json::json!({ "error": msg.to_string() }).to_string()
}

fn with_state<F, T>(f: F) -> Result<T, String>
where
    F: FnOnce(&WorldState) -> T,
{
    let lock = GAME.lock().expect("state lock poisoned");
    lock.as_ref()
        .map(f)
        .ok_or_else(|| "no active game — call new_game or load_game first".to_string())
}

fn take_state() -> Result<WorldState, String> {
    GAME.lock()
        .expect("state lock poisoned")
        .take()
        .ok_or_else(|| "no active game — call new_game or load_game first".to_string())
}

fn set_state(s: WorldState) {
    *GAME.lock().expect("state lock poisoned") = Some(s);
}

fn get_traits() -> PlayerTraits {
    PC_TRAITS
        .lock()
        .expect("traits lock poisoned")
        .unwrap_or_default()
}

fn set_traits(t: PlayerTraits) {
    *PC_TRAITS.lock().expect("traits lock poisoned") = Some(t);
}

/// Roll hidden trait ceilings from the creation seed, exactly like the TUI's
/// new-game flow (XOR domain tag so trait rolls don't alias attribute rolls).
fn roll_traits(seed: u64) -> PlayerTraits {
    PlayerTraits::roll_from_seed(seed, &mut GoatRng::new(seed ^ 0x7261_6974_0000_0001))
}

fn star_rating(player_output: i32) -> String {
    let n = (player_output / 20 + 1).clamp(1, 5) as usize;
    "★".repeat(n) + &"☆".repeat(5 - n)
}

fn moment_line(m: &goat_match::sim::MomentSummary) -> String {
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
}

fn build_snapshot(s: &WorldState) -> StateSnapshot {
    let world = get_world(s.world_seed);
    let (player_name, age_years, energy) = match s.pc_player_id {
        Some(id) => {
            let view = s.players.snapshot(id);
            (view.name.clone(), view.age_weeks / 52, view.energy.to_int())
        }
        None => (String::new(), 0, 100),
    };

    let (league_name, nation_name) = if s.season_number > 0 {
        match world.leagues.get(s.pc_div_idx as usize) {
            Some(l) => (l.name.clone(), world.nation_name(l.nation).to_string()),
            None => (String::new(), String::new()),
        }
    } else {
        (String::new(), String::new())
    };

    let season_year = s.career_base_year + s.season_number.saturating_sub(1);
    let cal_week = if s.season_number > 0 {
        round_to_week(s.season_round.min(ROUNDS_PER_SEASON as u32 - 1) as usize)
    } else {
        0
    };
    let week_label = if s.season_number > 0 {
        format_week_header(season_year, cal_week)
    } else {
        String::new()
    };

    let table = if s.season_number > 0 {
        let div_idx = s.pc_div_idx as usize;
        let div_clubs = live_league_clubs(&world, s, div_idx);
        let table = Table::from_raw(&s.table_raw, &div_clubs);
        table
            .sorted()
            .iter()
            .enumerate()
            .map(|(rank, e)| TableRowDto {
                position: rank + 1,
                club: world.clubs[e.club_id].name.to_string(),
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
    } else {
        Vec::new()
    };

    StateSnapshot {
        player_name,
        club_name: s.pc_club.to_string(),
        league_name,
        nation_name,
        age_years,
        energy,
        form: s.pc_form.to_int(),
        season_number: s.season_number,
        season_round: s.season_round,
        rounds_per_season: ROUNDS_PER_SEASON as u32,
        week_label,
        season_goals: s.pc_season_goals,
        season_assists: s.pc_season_assists,
        season_decisive: s.pc_season_decisive_moments,
        season_clutch: s.pc_season_clutch_index,
        trained_this_week: s.pc_week_training_done,
        season_over: s.season_round >= ROUNDS_PER_SEASON as u32,
        pre_season: in_pre_season(s),
        pre_season_week: season_week(s),
        routine_text: {
            use goat_core::attrs::ATTR_NAMES;
            let names: Vec<&str> = s
                .pc_routine
                .focus_attrs
                .iter()
                .map(|a| ATTR_NAMES[*a as usize])
                .collect();
            let focus = if names.is_empty() {
                "No focus".to_string()
            } else {
                names.join(", ")
            };
            format!("{} [{}]", focus, s.pc_routine.intensity.name())
        },
        table,
    }
}

fn week_events_text(s: &WorldState) -> Vec<String> {
    use goat_core::attrs::ATTR_NAMES;
    s.last_week_events
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
        .collect()
}

fn beat_to_dto(
    beat: &goat_match::beats::GeneratedBeat,
    ms: &ActiveMatchState,
    opp_name: &str,
) -> BeatDto {
    BeatDto {
        setup: beat.setup.clone(),
        minute: ms.current_minute(),
        beat_number: ms.beat_idx + 1,
        total_beats: ms.beats.len(),
        opp_name: opp_name.to_string(),
        player_output: ms.player_output,
        goals_for: ms.goals_for,
        goals_against: ms.goals_against,
        stamina: ms.stamina.to_int(),
        choices: beat
            .choices
            .iter()
            .enumerate()
            .map(|(i, c)| BeatChoiceDto {
                index: i,
                text: c.text.clone(),
            })
            .collect(),
    }
}

/// Family-representative match role for a position (copied from the bridge).
fn match_role_for_position(pc_position: u8) -> goat_core::roles::RoleId {
    use goat_core::roles::PositionFamily;
    match PrimaryPosition::from_u8(pc_position)
        .unwrap_or(PrimaryPosition::ST)
        .family()
    {
        PositionFamily::Defender => goat_core::roles::RoleId::CentreBack,
        PositionFamily::Midfielder => goat_core::roles::RoleId::CentralMid,
        PositionFamily::Forward => goat_core::roles::RoleId::CompleteForward,
    }
}

fn build_match_setup(
    s: &WorldState,
    own_strength: u8,
    opp_strength: u8,
    opp_name: &str,
    match_seed: u64,
) -> MatchSetup {
    let pc_id = s.pc_player_id.unwrap_or(0);
    let view = s.players.snapshot(pc_id);
    let ref_personality = {
        let mut rp_rng = GoatRng::new(match_seed ^ 0xBADCAFE);
        RefPersonality::from_rng(&mut rp_rng)
    };
    MatchSetup {
        player_role: match_role_for_position(s.pc_position),
        player_attrs: view.current,
        player_familiarity: view.familiarity,
        own_strength,
        opp_strength,
        opp_name: opp_name.to_string(),
        form: s.pc_form,
        player_aggression: view.current[AttrId::Aggression as usize]
            .to_int()
            .clamp(1, 99) as u8,
        ref_personality,
        dirty_rep: s.pc_discipline_rep,
        player_traits: get_traits(),
    }
}

/// Apply a completed round to the world state — shared by `skip_match` (auto
/// sim) and `play_match_choice` (interactive completion). Mirrors the tail of
/// the bridge's `play_round` / `make_beat_choice`: card intents, then
/// `ApplyRoundResult` with goals/assists/decisive/clutch counted from moments.
#[allow(clippy::too_many_arguments)]
fn apply_round(
    mut state: WorldState,
    round: usize,
    season: u32,
    div_idx: usize,
    pc_club_id: usize,
    world_seed: u64,
    pc_goals: u32,
    pc_assists: u32,
    pc_decisive: u32,
    pc_clutch: u32,
    pc_output: i32,
    goals_for: u32,
    goals_against: u32,
    yellow_cards: u32,
    red_card: bool,
) -> WorldState {
    let world = get_world(world_seed);
    let div_clubs = live_league_clubs(&world, &state, div_idx);
    let old_cal_week = round_to_week(round.min(ROUNDS_PER_SEASON - 1));

    let all_fixtures = goat_world::round_fixtures(world_seed, season, div_idx, &div_clubs, round);
    let sim_seed = world_seed ^ ((season as u64) << 32) ^ (round as u64) ^ 0xfeed;
    let mut sim_rng = GoatRng::new(sim_seed);
    let pc_fixture =
        goat_world::fixture_for_round(world_seed, season, div_idx, &div_clubs, pc_club_id, round);

    let pc_result: i8 = if goals_for > goals_against {
        1
    } else if goals_for < goals_against {
        -1
    } else {
        0
    };

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

    if yellow_cards > 0 || red_card {
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
                yellow_cards,
                red_card,
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
            fixture_importance: FixtureImportance::League,
            pc_output,
            pc_result,
            round_results,
            rest_weeks: goat_world::rest_weeks_after_round(round),
            week_ends: goat_world::week_ends_after_round(round),
        },
        &mut GoatRng::new(0),
    );

    // Training flag is reset by the ApplyRoundResult reducer. Keep the stored
    // calendar week in sync when the round crossed a week boundary.
    let new_cal_week = round_to_week((state.season_round as usize).min(ROUNDS_PER_SEASON - 1));
    if new_cal_week != old_cal_week {
        state.pc_current_calendar_week = new_cal_week as u32;
    }
    state
}

// ── Exported API ──────────────────────────────────────────────────────────────

/// Picker data: all 20 nations (id, name, stature). Nation identity is
/// seed-independent, so this uses the fixed picker world.
#[wasm_bindgen]
pub fn get_nations() -> String {
    let world = get_world(PICKER_WORLD_SEED);
    let nations: Vec<NationDto> = world
        .nations
        .iter()
        .map(|n| NationDto {
            id: n.id,
            name: n.name.clone(),
            stature: n.stature,
        })
        .collect();
    to_json(&nations)
}

/// The 3 leagues of one nation (tier-ordered) for the given world seed.
#[wasm_bindgen]
pub fn get_leagues(seed: u64, nation_idx: usize) -> String {
    let world = get_world(seed);
    if nation_idx >= world.nations.len() {
        return err_json(format!("nation_idx {nation_idx} out of range"));
    }
    let mut leagues: Vec<LeagueDto> = world
        .leagues
        .iter()
        .filter(|l| l.nation == nation_idx)
        .map(|l| LeagueDto {
            id: l.id,
            name: l.name.clone(),
            tier: l.tier as usize,
        })
        .collect();
    leagues.sort_by_key(|l| l.tier);
    to_json(&leagues)
}

/// The 20 clubs of one league (by global league id) for the given world seed.
#[wasm_bindgen]
pub fn get_clubs(seed: u64, league_id: usize) -> String {
    let world = get_world(seed);
    let Some(league) = world.leagues.get(league_id) else {
        return err_json(format!("league_id {league_id} out of range"));
    };
    let clubs: Vec<ClubDto> = league
        .clubs
        .iter()
        .map(|&cid| ClubDto {
            id: cid,
            name: world.clubs[cid].name.clone(),
            strength: world.clubs[cid].strength,
        })
        .collect();
    to_json(&clubs)
}

/// Start a new game. Mirrors the bridge's `new_game`: CreatePlayer → InitWorld
/// → InitPeers → StartSeason, plus the TUI-style trait roll from the creation
/// seed. Returns the state snapshot JSON.
#[wasm_bindgen]
pub fn new_game(
    seed: u64,
    name: &str,
    position: u8,
    nation_idx: usize,
    league_idx: usize,
    club_idx: usize,
    base_year: u32,
) -> String {
    let world = get_world(seed);
    if nation_idx >= world.nations.len() {
        return err_json(format!("nation_idx {nation_idx} out of range"));
    }
    let mut nation_leagues: Vec<&goat_world::world::League> = world
        .leagues
        .iter()
        .filter(|l| l.nation == nation_idx)
        .collect();
    nation_leagues.sort_by_key(|l| l.tier as usize);
    let Some(league) = nation_leagues.get(league_idx) else {
        return err_json(format!("league_idx {league_idx} out of range"));
    };
    let Some(&club_id) = league.clubs.get(club_idx) else {
        return err_json(format!("club_idx {club_idx} out of range"));
    };

    let club = &world.clubs[club_id];
    let div_idx = world.club_league(club_id);
    let nationality = world.nation_name(club.nation).to_string();
    let primary_position = PrimaryPosition::from_u8(position).unwrap_or(PrimaryPosition::ST);

    let choices = CreationChoices {
        name: name.to_string(),
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
    state = reduce(
        state,
        Intent::InitPeers {
            peers: build_peers(seed, &nationality),
        },
        &mut GoatRng::new(0),
    );
    state = reduce(
        state,
        Intent::StartSeason { fixtures: vec![] },
        &mut GoatRng::new(0),
    );
    // Career epoch: the real-world year, supplied by the page (new Date().getFullYear())
    // — wall-clock lives in JS, never in the core (§9).
    state.career_base_year = base_year;

    set_traits(roll_traits(seed));
    *ACTIVE_MATCH.lock().expect("match lock poisoned") = None;
    let snap = build_snapshot(&state);
    set_state(state);
    to_json(&snap)
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

/// Current state snapshot JSON.
#[wasm_bindgen]
pub fn state() -> String {
    match with_state(build_snapshot) {
        Ok(snap) => to_json(&snap),
        Err(e) => err_json(e),
    }
}

/// Advance one training week. Guards the already-trained case like the TUI:
/// reports instead of double-training. Returns events text + fresh state.
///
/// Pre-season weeks (Jul-1 anchor) tick via `AdvanceWeeks{1}` — each pre-season
/// week is fresh, and the per-week training flag only resets on round
/// boundaries (mirrors the TUI's pre-season [C]/[W] branch exactly).
#[wasm_bindgen]
pub fn train() -> String {
    let state = match take_state() {
        Ok(s) => s,
        Err(e) => return err_json(e),
    };
    let pc_id = state.pc_player_id.unwrap_or(0);
    let view = state.players.snapshot(pc_id);
    let seed = (view.age_weeks as u64).wrapping_mul(6364136223846793005);

    if in_pre_season(&state) {
        let mut state = reduce(
            state,
            Intent::AdvanceWeeks { n: 1 },
            &mut GoatRng::new(seed),
        );
        let events = week_events_text(&state);
        let mut text = if events.is_empty() {
            format!(
                "Trained. Pre-season week {}/{} done.",
                season_week(&state),
                PRE_SEASON_WEEKS
            )
        } else {
            format!("Trained.\n{}", events.join("\n"))
        };
        if !in_pre_season(&state) {
            // The first competition week opens fresh (round 1's week is still
            // trainable) — same boundary rule as the TUI.
            state.pc_week_training_done = false;
            text.push_str("\nPre-season complete — the league campaign opens this week.");
        }
        let snap = build_snapshot(&state);
        set_state(state);
        return serde_json::json!({ "text": text, "state": snap }).to_string();
    }

    let already_trained = state.pc_week_training_done;
    let state = reduce(state, Intent::AdvanceWeek, &mut GoatRng::new(seed));

    let text = if already_trained {
        "You've already trained this week — Play or Skip this round's match to continue."
            .to_string()
    } else {
        let events = week_events_text(&state);
        if events.is_empty() {
            "Trained. Quiet week.".to_string()
        } else {
            format!("Trained.\n{}", events.join("\n"))
        }
    };

    let snap = build_snapshot(&state);
    set_state(state);
    serde_json::json!({ "text": text, "state": snap }).to_string()
}

/// Rest one pre-season week: tick the week with a no-focus routine (no
/// attribute growth) using existing intents only — the standing routine is
/// restored afterwards. Pre-season only; the competition weeks already rest
/// automatically via the round bookkeeping. Mirrors the TUI, where "rest" is
/// simply not having a focus for the week's tick.
#[wasm_bindgen]
pub fn rest_week() -> String {
    let state = match take_state() {
        Ok(s) => s,
        Err(e) => return err_json(e),
    };
    if !in_pre_season(&state) {
        let snap = build_snapshot(&state);
        set_state(state);
        return serde_json::json!({
            "text": "Rest weeks are a pre-season option — during the season the calendar handles rest automatically.",
            "state": snap,
        })
        .to_string();
    }
    let pc_id = state.pc_player_id.unwrap_or(0);
    let seed = {
        let view = state.players.snapshot(pc_id);
        (view.age_weeks as u64).wrapping_mul(6364136223846793005)
    };
    let saved_routine = state.pc_routine.clone();
    let mut state = reduce(
        state,
        Intent::SetRoutine {
            routine: goat_core::week::Routine {
                focus_attrs: vec![],
                intensity: saved_routine.intensity,
            },
        },
        &mut GoatRng::new(0),
    );
    state = reduce(
        state,
        Intent::AdvanceWeeks { n: 1 },
        &mut GoatRng::new(seed),
    );
    state = reduce(
        state,
        Intent::SetRoutine {
            routine: saved_routine,
        },
        &mut GoatRng::new(0),
    );
    let mut text = format!(
        "Rested. Pre-season week {}/{} done.",
        season_week(&state),
        PRE_SEASON_WEEKS
    );
    if !in_pre_season(&state) {
        state.pc_week_training_done = false;
        text.push_str("\nPre-season complete — the league campaign opens this week.");
    }
    let snap = build_snapshot(&state);
    set_state(state);
    serde_json::json!({ "text": text, "state": snap }).to_string()
}

/// Begin an interactive pre-season FRIENDLY (beat-by-beat, like
/// `play_match_start`). Ad-hoc match only — no calendar fixture, and on
/// completion only `ApplyMatchResult` applies (development), never the league
/// table/season stats/discipline. Pre-season only.
#[wasm_bindgen]
pub fn play_friendly_start() -> String {
    let result = with_state(|s| -> Result<ActiveMatchSession, String> {
        if !in_pre_season(s) {
            return Err(
                "Friendlies are a pre-season option — the league season is on.".to_string(),
            );
        }
        let pc_club_id = s.pc_club_idx as usize;
        let week = season_week(s);
        let pc_league_id = s.pc_div_idx as usize;
        let world = get_world(s.world_seed);
        let nation = world.leagues[pc_league_id].nation;
        let mut tiers: Vec<usize> = world
            .leagues
            .iter()
            .filter(|l| l.nation == nation)
            .map(|l| l.id)
            .collect();
        tiers.sort_by_key(|&id| world.leagues[id].tier as usize);
        let pos = tiers.iter().position(|&id| id == pc_league_id).unwrap_or(0);
        let sibling_tiers: Vec<usize> = (0..tiers.len()).filter(|&i| i != pos).collect();
        let opp_league = tiers[sibling_tiers[week as usize % sibling_tiers.len()]];
        let slot = world.club_league_pos(pc_club_id);
        let opp_id = world.leagues[opp_league].clubs[slot % goat_world::CLUBS_PER_DIV];
        let opp = &world.clubs[opp_id];

        let match_seed =
            s.world_seed ^ ((s.season_number as u64) << 32) ^ (week as u64) ^ 0xF21E_5A17;
        let mut rng = GoatRng::new(match_seed);
        let own_str = world.clubs[pc_club_id].strength;
        let setup = build_match_setup(s, own_str, opp.strength, &opp.name, match_seed);
        let ms = with_beat_lib(|lib| start_match(lib, setup, &mut rng));
        Ok(ActiveMatchSession {
            state: ms,
            rng,
            round: 0, // unused for friendlies — no round is resolved on completion
            pc_club_id,
            div_idx: pc_league_id,
            season: s.season_number,
            world_seed: s.world_seed,
            opp_name: opp.name.clone(),
            friendly: true,
        })
    });

    let session = match result {
        Ok(Ok(session)) => session,
        Ok(Err(e)) => return err_json(e),
        Err(e) => return err_json(e),
    };
    let Some(beat) = session.state.current_beat() else {
        return err_json("match produced no beats");
    };
    let mut dto = beat_to_dto(beat, &session.state, &session.opp_name.clone());
    dto.setup = format!("[FRIENDLY] {}", dto.setup);
    *ACTIVE_MATCH.lock().expect("match lock poisoned") = Some(session);
    to_json(&dto)
}

/// All 30 attribute ids + names, for the routine picker's UI.
#[wasm_bindgen]
pub fn get_attrs() -> String {
    use goat_core::attrs::{AttrId, ATTR_NAMES};
    let attrs: Vec<serde_json::Value> = AttrId::ALL
        .iter()
        .map(|a| serde_json::json!({ "id": *a as u8, "name": ATTR_NAMES[*a as usize] }))
        .collect();
    serde_json::to_string(&attrs).expect("DTO serialization is infallible")
}

/// Set the standing training routine (ports the TUI's [S]): up to 4 focus
/// attributes by AttrId discriminant + intensity (0=Low, 1=Medium, 2=High).
/// An empty `attr_ids` clears the focus ("No focus"). Persists until changed.
#[wasm_bindgen]
pub fn set_routine(attr_ids: Vec<u8>, intensity: u8) -> String {
    let state = match take_state() {
        Ok(s) => s,
        Err(e) => return err_json(e),
    };
    let valid: Vec<u8> = attr_ids
        .into_iter()
        .filter(|&a| (a as usize) < goat_core::attrs::NUM_ATTRS)
        .take(4)
        .collect();
    let focus_attrs: Vec<goat_core::attrs::AttrId> = valid
        .iter()
        .map(|&a| goat_core::attrs::AttrId::ALL[a as usize])
        .collect();
    let intensity = match intensity {
        0 => goat_core::week::Intensity::Low,
        2 => goat_core::week::Intensity::High,
        _ => goat_core::week::Intensity::Medium,
    };
    let state = reduce(
        state,
        Intent::SetRoutine {
            routine: goat_core::week::Routine {
                focus_attrs,
                intensity,
            },
        },
        &mut GoatRng::new(0),
    );
    let snap = build_snapshot(&state);
    let text = format!("Routine set: {}", snap.routine_text);
    set_state(state);
    serde_json::json!({ "text": text, "state": snap }).to_string()
}

/// Fast-forward N weeks (ports the TUI's [F]): each week auto-applies the
/// standing routine; the loop stops early at the first noteworthy development
/// event; all events/flashpoints across the skipped weeks are accumulated and
/// returned at the end. Competition-season only — pre-season weeks are driven
/// one at a time via train()/rest_week() so the friendly offer isn't skipped.
#[wasm_bindgen]
pub fn advance_weeks(n: u32) -> String {
    let state = match take_state() {
        Ok(s) => s,
        Err(e) => return err_json(e),
    };
    if in_pre_season(&state) {
        let snap = build_snapshot(&state);
        set_state(state);
        return serde_json::json!({
            "text": "Pre-season weeks tick one at a time — use Train / Rest / Play Friendly.",
            "state": snap,
        })
        .to_string();
    }
    let pc_id = state.pc_player_id.unwrap_or(0);
    let seed = {
        let view = state.players.snapshot(pc_id);
        (view.age_weeks as u64).wrapping_mul(6364136223846793005)
    };
    let state = reduce(state, Intent::AdvanceWeeks { n }, &mut GoatRng::new(seed));

    let mut lines = week_events_text(&state);
    {
        use goat_calendar::WindowKind;
        for f in &state.last_week_flashpoints {
            let (icon, label) = match f.window {
                WindowKind::TransferSummer => ("⇄", "The summer transfer window is open."),
                WindowKind::TransferWinter => ("⇄", "The winter transfer window is open."),
                WindowKind::InternationalBreak => {
                    ("✈", "International break — call-ups announced.")
                }
                WindowKind::OffSeason => ("☼", "The off-season has begun."),
            };
            lines.push(format!("{icon}  CALENDAR: {label}"));
        }
    }
    let text = if lines.is_empty() {
        format!("Advanced {n} week(s). Quiet stretch.")
    } else {
        format!("Advanced {n} week(s).\n{}", lines.join("\n"))
    };
    let snap = build_snapshot(&state);
    set_state(state);
    serde_json::json!({ "text": text, "state": snap }).to_string()
}

/// Auto-play the current round and apply the result, exactly like the bridge's
/// `play_round`. Returns result summary text + state.
#[wasm_bindgen]
pub fn skip_match() -> String {
    let state = match take_state() {
        Ok(s) => s,
        Err(e) => return err_json(e),
    };
    if state.season_number == 0 || state.season_round >= ROUNDS_PER_SEASON as u32 {
        let snap = build_snapshot(&state);
        set_state(state);
        return serde_json::json!({
            "text": "No match left to play — the season is over.",
            "state": snap,
        })
        .to_string();
    }
    if in_pre_season(&state) {
        let snap = build_snapshot(&state);
        set_state(state);
        return serde_json::json!({
            "text": "No league fixture yet — pre-season friendlies are optional; train or rest through the week.",
            "state": snap,
        })
        .to_string();
    }

    let round = state.season_round as usize;
    let season = state.season_number;
    let div_idx = state.pc_div_idx as usize;
    let pc_club_id = state.pc_club_idx as usize;
    let world_seed = state.world_seed;
    let world = get_world(world_seed);
    let div_clubs = live_league_clubs(&world, &state, div_idx);

    let match_seed = world_seed ^ ((season as u64) << 32) ^ (round as u64) ^ 0xc0ffee;
    let mut match_rng = GoatRng::new(match_seed);
    let is_suspended = state.pc_suspension_matches_remaining(LEAGUE_COMPETITION_ID) > 0;

    let pc_fixture =
        goat_world::fixture_for_round(world_seed, season, div_idx, &div_clubs, pc_club_id, round);

    let Some(f) = pc_fixture else {
        let snap = build_snapshot(&state);
        set_state(state);
        return serde_json::json!({
            "text": "No fixture for your club this round.",
            "state": snap,
        })
        .to_string();
    };

    let is_home = f.home == pc_club_id;
    let opp_id = if is_home { f.away } else { f.home };
    let opp = &world.clubs[opp_id];
    let own_str = world.clubs[pc_club_id].strength;
    let own_name = world.clubs[pc_club_id].name.clone();

    let (
        pc_goals,
        pc_assists,
        pc_decisive,
        pc_clutch,
        pc_output,
        goals_for,
        goals_against,
        yc,
        rc,
        summary,
    ) = if is_suspended {
        // A suspended player's club still plays the fixture, but the PC
        // doesn't personally take part (bible AC-06).
        let (gf, ga) = goat_world::sim_team_match(own_str, opp.strength, &mut match_rng);
        (
            0,
            0,
            0,
            0,
            0,
            gf,
            ga,
            0u32,
            false,
            format!("SUSPENDED — {own_name} {gf}–{ga} {}", opp.name),
        )
    } else {
        let setup = build_match_setup(&state, own_str, opp.strength, &opp.name, match_seed);
        let result = with_beat_lib(|lib| auto_play_match(lib, setup, &mut match_rng));
        let pc_goals = result
            .moments
            .iter()
            .filter(|m| matches!(m.goal_event, Some(ScoreEvent::GoalFor)))
            .count() as u32;
        let pc_assists = result
            .moments
            .iter()
            .filter(|m| matches!(m.goal_event, Some(ScoreEvent::AssistFor)))
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
        let moments: Vec<String> = result
            .moments
            .iter()
            .filter(|m| m.goal_event.is_some() || m.success)
            .take(5)
            .map(moment_line)
            .collect();
        let summary = format!(
            "FT: {own_name} {}–{} {}  ({} · output {})\n{}",
            result.goals_for,
            result.goals_against,
            opp.name,
            star_rating(result.player_output),
            result.player_output,
            moments.join("\n")
        );
        (
            pc_goals,
            pc_assists,
            pc_decisive,
            pc_clutch,
            result.player_output,
            result.goals_for,
            result.goals_against,
            result.yellow_cards as u32,
            result.red_card,
            summary,
        )
    };

    let state = apply_round(
        state,
        round,
        season,
        div_idx,
        pc_club_id,
        world_seed,
        pc_goals,
        pc_assists,
        pc_decisive,
        pc_clutch,
        pc_output,
        goals_for,
        goals_against,
        yc,
        rc,
    );
    let snap = build_snapshot(&state);
    set_state(state);
    serde_json::json!({ "text": summary, "state": snap }).to_string()
}

/// Begin an interactive match for the current round. Returns the first beat
/// JSON (setup text + choices). Errors when there is no fixture, the PC is
/// suspended, or the season is over.
#[wasm_bindgen]
pub fn play_match_start() -> String {
    let result = with_state(|s| -> Result<ActiveMatchSession, String> {
        if s.season_number == 0 || s.season_round >= ROUNDS_PER_SEASON as u32 {
            return Err("No match to play — the season is over.".to_string());
        }
        if in_pre_season(s) {
            return Err(
                "No league fixture yet — this is a pre-season week (play a friendly instead)."
                    .to_string(),
            );
        }
        if s.pc_suspension_matches_remaining(LEAGUE_COMPETITION_ID) > 0 {
            return Err(
                "You are suspended — skip the match to serve the ban (bible AC-06).".to_string(),
            );
        }
        let round = s.season_round as usize;
        let div_idx = s.pc_div_idx as usize;
        let pc_club_id = s.pc_club_idx as usize;
        let world_seed = s.world_seed;
        let season = s.season_number;
        let world = get_world(world_seed);
        let div_clubs = live_league_clubs(&world, s, div_idx);

        let pc_fixture = goat_world::fixture_for_round(
            world_seed, season, div_idx, &div_clubs, pc_club_id, round,
        )
        .ok_or_else(|| "No fixture for your club this round.".to_string())?;
        let is_home = pc_fixture.home == pc_club_id;
        let opp_id = if is_home {
            pc_fixture.away
        } else {
            pc_fixture.home
        };
        let opp = &world.clubs[opp_id];
        let own_str = world.clubs[pc_club_id].strength;

        let match_seed = world_seed ^ ((season as u64) << 32) ^ (round as u64) ^ 0xc0ffee;
        let mut rng = GoatRng::new(match_seed);
        let setup = build_match_setup(s, own_str, opp.strength, &opp.name, match_seed);
        let ms = with_beat_lib(|lib| start_match(lib, setup, &mut rng));
        Ok(ActiveMatchSession {
            state: ms,
            rng,
            round,
            pc_club_id,
            div_idx,
            season,
            world_seed,
            opp_name: opp.name.clone(),
            friendly: false,
        })
    });

    let session = match result {
        Ok(Ok(session)) => session,
        Ok(Err(e)) => return err_json(e),
        Err(e) => return err_json(e),
    };
    let Some(beat) = session.state.current_beat() else {
        return err_json("match produced no beats");
    };
    let dto = beat_to_dto(beat, &session.state, &session.opp_name.clone());
    *ACTIVE_MATCH.lock().expect("match lock poisoned") = Some(session);
    to_json(&dto)
}

/// Resolve a player choice for the current interactive beat. Returns the
/// outcome text + next beat JSON, or — when the match completes — the
/// full-time summary and applies the round to state exactly like `skip_match`.
#[wasm_bindgen]
pub fn play_match_choice(idx: usize) -> String {
    let Some(mut session) = ACTIVE_MATCH.lock().expect("match lock poisoned").take() else {
        return err_json("no interactive match in progress — call play_match_start first");
    };

    session.state =
        with_beat_lib(|lib| advance_beat(session.state.clone(), idx, lib, &mut session.rng));

    let Some(last) = session.state.moments.last() else {
        return err_json("beat produced no moment");
    };
    let success = last.success;
    let outcome_text = last.outcome_text.to_string();

    if !session.state.is_complete {
        let next_beat = session
            .state
            .current_beat()
            .map(|b| beat_to_dto(b, &session.state, &session.opp_name));
        let goals_for = session.state.goals_for;
        let goals_against = session.state.goals_against;
        let player_output = session.state.player_output;
        *ACTIVE_MATCH.lock().expect("match lock poisoned") = Some(session);
        return serde_json::json!({
            "success": success,
            "outcome_text": outcome_text,
            "goals_for": goals_for,
            "goals_against": goals_against,
            "player_output": player_output,
            "is_complete": false,
            "next_beat": next_beat,
        })
        .to_string();
    }

    // Match complete — count PC contributions from moments, then apply the
    // round exactly like skip_match does.
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
    let yc = session.state.yellow_cards as u32;
    let rc = session.state.red_card;
    let moments: Vec<String> = session
        .state
        .moments
        .iter()
        .filter(|m| m.goal_event.is_some() || m.success)
        .take(5)
        .map(moment_line)
        .collect();

    let own_name = {
        let world = get_world(session.world_seed);
        world.clubs[session.pc_club_id].name.clone()
    };
    let scoreline = format!(
        "{own_name} {goals_for}–{goals_against} {}",
        session.opp_name
    );

    let state = match take_state() {
        Ok(s) => s,
        Err(e) => return err_json(e),
    };

    // Friendly (pre-season): development only — no round resolution, no table,
    // no season stats, no discipline (mirrors the TUI's run_friendly).
    if session.friendly {
        let state = reduce(
            state,
            Intent::ApplyMatchResult {
                familiarity_xp: session.state.familiarity_xp,
                energy_cost: Fixed::from_int(25),
                injury_weeks: None,
            },
            &mut GoatRng::new(0),
        );
        let snap = build_snapshot(&state);
        set_state(state);
        return serde_json::json!({
            "success": success,
            "outcome_text": outcome_text,
            "goals_for": goals_for,
            "goals_against": goals_against,
            "player_output": pc_output,
            "is_complete": true,
            "friendly": true,
            "final": {
                "scoreline": format!("FRIENDLY — {scoreline}"),
                "rating": star_rating(pc_output),
                "output": pc_output,
                "goals": pc_goals,
                "assists": pc_assists,
                "decisive": 0,
                "clutch": 0,
                "moments": moments,
            },
            "state": snap,
        })
        .to_string();
    }

    let state = apply_round(
        state,
        session.round,
        session.season,
        session.div_idx,
        session.pc_club_id,
        session.world_seed,
        pc_goals,
        pc_assists,
        pc_decisive,
        pc_clutch,
        pc_output,
        goals_for,
        goals_against,
        yc,
        rc,
    );
    let snap = build_snapshot(&state);
    set_state(state);

    serde_json::json!({
        "success": success,
        "outcome_text": outcome_text,
        "goals_for": goals_for,
        "goals_against": goals_against,
        "player_output": pc_output,
        "is_complete": true,
        "final": {
            "scoreline": scoreline,
            "rating": star_rating(pc_output),
            "output": pc_output,
            "goals": pc_goals,
            "assists": pc_assists,
            "decisive": pc_decisive,
            "clutch": pc_clutch,
            "moments": moments,
        },
        "state": snap,
    })
    .to_string()
}

/// Apply end-of-season legacy (mirrors the bridge's `apply_season_end`:
/// CollectWage, BatchTickPeers, rival crystallisation, ApplySeasonEndLegacy
/// with the season counters incl. season_clutch_index). Returns a summary
/// text + state.
#[wasm_bindgen]
pub fn season_end() -> String {
    let state = match take_state() {
        Ok(s) => s,
        Err(e) => return err_json(e),
    };
    if state.season_round < ROUNDS_PER_SEASON as u32 {
        set_state(state);
        return err_json("the season isn't over yet — play out the remaining rounds");
    }

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
    let league_name = world.leagues[div_idx].name.clone();
    let club_name = state.pc_club.clone();

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
    let player_of_year = goat_meta::compute_player_of_year(
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

    // Rival crystallisation (mirrors the bridge).
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
            // International football isn't wired into the web demo (same
            // deferred gap as the bridge layer).
            season_caps: 0,
            season_international_goals: 0,
            season_world_cups_won: 0,
            season_continental_championships_won: 0,
        },
        &mut GoatRng::new(0),
    );

    let promo_note = if won_title {
        " CHAMPIONS!".to_string()
    } else if finish_pos as usize <= goat_world::world::PROMO_RELEGATION_N {
        " — promotion zone.".to_string()
    } else if (finish_pos as usize)
        > goat_world::world::CLUBS_PER_DIV - goat_world::world::PROMO_RELEGATION_N
    {
        " — relegation zone.".to_string()
    } else {
        String::new()
    };
    let text = format!(
        "Season {season} complete — {club_name} finished {finish_pos} in {league_name}.{promo_note}\n\
         Wage collected: {} (annual {}). Player of the Year: {}.\n\
         Promotion/relegation resolves when the next season starts.",
        state.pc_savings,
        state.pc_wage_annual,
        if player_of_year { "YOU" } else { "not you" },
    );

    let snap = build_snapshot(&state);
    set_state(state);
    serde_json::json!({ "text": text, "state": snap }).to_string()
}

/// Start the next season (mirrors the bridge's `start_next_season`, INCLUDING
/// the A3.3 promotion resolution against the persisted pc_nation_membership
/// overlay). Returns the promotion/relegation events as readable text + state.
#[wasm_bindgen]
pub fn start_next_season() -> String {
    let mut state = match take_state() {
        Ok(s) => s,
        Err(e) => return err_json(e),
    };
    if state.season_round < ROUNDS_PER_SEASON as u32 {
        set_state(state);
        return err_json("the season isn't over yet");
    }

    // A3.3: promotion/relegation for the PC's nation at the season boundary —
    // the PC's league from the REAL played table (table_raw), the two sibling
    // leagues batch-simmed from static strengths. The cached world is never
    // mutated: drift lives only in the persisted pc_nation_membership.
    let events_text = {
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
        let events = apply_season_end_for_nation(
            &world,
            &mut membership,
            nation,
            state.season_number,
            &tables,
        );

        let texts: Vec<String> = events
            .iter()
            .map(|e| {
                let club = &world.clubs[e.club].name;
                let from = &world.leagues[e.from_league].name;
                let to = &world.leagues[e.to_league].name;
                match e.transition {
                    goat_world::promotion::TransitionType::DirectPromotion => {
                        format!("Promoted: {club} ({from} → {to})")
                    }
                    goat_world::promotion::TransitionType::DirectRelegation => {
                        format!("Relegated: {club} ({from} → {to})")
                    }
                }
            })
            .collect();

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
        texts
    };

    let state = reduce(
        state,
        Intent::StartSeason { fixtures: vec![] },
        &mut GoatRng::new(0),
    );
    let text = if events_text.is_empty() {
        format!(
            "Season {} begins — no promotion/relegation movement.",
            state.season_number
        )
    } else {
        format!(
            "Season {} begins.\n{}",
            state.season_number,
            events_text.join("\n")
        )
    };
    let snap = build_snapshot(&state);
    set_state(state);
    serde_json::json!({ "text": text, "events": events_text, "state": snap }).to_string()
}

/// Serialize the current game to save bytes (from_world_state + to_bytes).
/// Returns an empty buffer when no game is active.
#[wasm_bindgen]
pub fn save_game() -> Vec<u8> {
    match with_state(|s| {
        s.pc_player_id.map(|pc_id| {
            let view = s.players.snapshot(pc_id);
            let data = goat_save::save::from_world_state(s, &view);
            goat_save::save::to_bytes(&data)
        })
    }) {
        Ok(Some(bytes)) => bytes,
        _ => Vec::new(),
    }
}

/// Load a game from save bytes (from_bytes + to_world_state), rebuild the
/// session, and return the state snapshot JSON.
#[wasm_bindgen]
pub fn load_game(bytes: &[u8]) -> String {
    let data = match goat_save::save::from_bytes(bytes) {
        Ok(d) => d,
        Err(e) => return err_json(format!("invalid save: {e:?}")),
    };
    let world = get_world(data.world_seed);
    let state = goat_save::save::to_world_state(&data, &world);
    set_traits(roll_traits(data.world_seed));
    *ACTIVE_MATCH.lock().expect("match lock poisoned") = None;
    let snap = build_snapshot(&state);
    set_state(state);
    to_json(&snap)
}
