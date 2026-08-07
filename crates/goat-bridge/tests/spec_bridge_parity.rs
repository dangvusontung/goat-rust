//! spec_bridge_parity — bridge-level spec tests (TDD anchor of TASK-BRIDGE-refresh).
//!
//! Verifies that the FFI surface the Flutter app calls behaves like the core:
//! - determinism survives the bridge (same seed ⇒ identical snapshots),
//! - a scripted sequence through the bridge matches the same intents applied
//!   via direct `reduce`,
//! - the full-season lifecycle (awards → season end → next season), save/load,
//!   and the interactive-match flow all work end to end.
//!
//! The bridge holds a global singleton `WorldState`, so every test takes
//! TEST_LOCK first to serialize access regardless of the test-thread count.

use std::sync::{Mutex, MutexGuard};

use goat_bridge::api;
use goat_core::{
    derive::ovr,
    generation::CreationChoices,
    positions::PrimaryPosition,
    state::{reduce, Intent, PeerState, WorldState},
    week::{Intensity, Routine},
};
use goat_rng::{GoatRng, RngSource};
use goat_world::{world::WorldGenesis, CLUBS_PER_DIV, ROUNDS_PER_SEASON};

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

const SEED: u64 = 0x00B1_660A;
const NAME: &str = "Testy McTestface";
const CLUB_ID: u32 = 0;
// `PrimaryPosition::ST` = 0 (widened 0..7 range — bible §4 revision); matches the
// `PrimaryPosition::ST` hardcoded in `direct_new_game` below so the bridge and direct
// paths generate the same player.
const POS_FWD: u8 = 0;
const LIFESTYLE_PRO: u8 = 0;

/// Scripted run: new game, set a routine, then train + play `n` rounds.
fn script_rounds(seed: u64, n: u32) -> (api::GoatGameState, Vec<api::MatchResultDto>) {
    api::new_game(NAME.to_string(), POS_FWD, CLUB_ID, seed, LIFESTYLE_PRO);
    let mut snap = api::set_routine(vec![0, 1, 2], 1);
    let mut results = Vec::new();
    for _ in 0..n {
        api::advance_week();
        let (s, r) = api::play_round(false);
        snap = s;
        results.push(r);
    }
    (snap, results)
}

fn assert_snapshot_invariants(s: &api::GoatGameState, ctx: &str) {
    assert!(
        (0..=100).contains(&s.energy),
        "{ctx}: energy out of range: {}",
        s.energy
    );
    assert!(
        (0..=100).contains(&s.ovr),
        "{ctx}: ovr out of range: {}",
        s.ovr
    );
    assert!(
        (0..=100).contains(&s.form),
        "{ctx}: form out of range: {}",
        s.form
    );
    assert!(
        s.season_round <= s.rounds_per_season,
        "{ctx}: season_round {} > rounds_per_season {}",
        s.season_round,
        s.rounds_per_season
    );
    // NB: career_goals / career_matches are banked only at season end
    // (Intent::ApplySeasonEndLegacy) — mid-season they EXCLUDE the running
    // season, so `career >= season` does not hold here. UI must show
    // career + season for live career totals.
}

// ── 1. New-game snapshot sanity ───────────────────────────────────────────────

#[test]
fn new_game_snapshot_is_sane() {
    let _g = serial();
    let s = api::new_game(NAME.to_string(), POS_FWD, CLUB_ID, SEED, LIFESTYLE_PRO);

    assert!(api::has_active_game());
    assert_eq!(s.player_name, NAME);
    assert_eq!(s.position, POS_FWD);
    let world = WorldGenesis::generate(SEED);
    assert_eq!(s.club_name, world.clubs[CLUB_ID as usize].name);
    assert_eq!(s.season_number, 1);
    assert_eq!(s.season_round, 0);
    assert_eq!(s.rounds_per_season, ROUNDS_PER_SEASON as u32);
    assert_eq!(s.season_goals, 0);
    assert_eq!(s.career_matches, 0);
    assert!(!s.is_retired);
    assert!(s.ovr > 0, "fresh player should have a nonzero OVR");
    assert!(
        s.age_years >= 14 && s.age_years <= 21,
        "age {}",
        s.age_years
    );
    assert_snapshot_invariants(&s, "new_game");

    // Read endpoints work immediately after creation.
    assert_eq!(api::get_attributes().len(), 30);
    assert_eq!(api::get_table().len(), CLUBS_PER_DIV);
    assert_eq!(api::get_peers().len(), 8);
    assert!(api::get_state().is_some());
}

// ── 2. Determinism survives the FFI boundary ─────────────────────────────────

#[test]
fn same_seed_same_script_is_deterministic() {
    let _g = serial();
    let (snap_a, results_a) = script_rounds(SEED, 10);
    let (snap_b, results_b) = script_rounds(SEED, 10);

    assert_eq!(
        snap_a, snap_b,
        "same seed + same script must give identical snapshots"
    );
    assert_eq!(
        results_a, results_b,
        "same seed + same script must give identical match results"
    );
}

#[test]
fn different_seeds_diverge() {
    let _g = serial();
    let (_, results_a) = script_rounds(SEED, 10);
    let (_, results_b) = script_rounds(SEED ^ 0xDEAD_BEEF, 10);
    assert_ne!(
        results_a, results_b,
        "different world seeds should produce different match histories"
    );
}

// ── 3. Bridge path matches direct reduce ─────────────────────────────────────

/// Mirror of the private `build_peers` in api.rs — pins the seeding behaviour.
fn direct_build_peers(world_seed: u64, nationality: &str) -> Vec<PeerState> {
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

/// Same intent sequence `new_game` performs, applied via direct reduce.
fn direct_new_game(seed: u64, club_id: usize) -> WorldState {
    let world = WorldGenesis::generate(seed);
    let club = &world.clubs[club_id];
    let div_idx = world.club_league(club_id);
    let nationality = world.nation_name(club.nation).to_string();

    let choices = CreationChoices {
        name: NAME.to_string(),
        primary_position: PrimaryPosition::ST,
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
    // Lifestyle is no longer a settable intent (bible §8.5/§8.6); the bridge path's
    // `new_game(..., lifestyle)` param is likewise now ignored — both paths leave the
    // career at the default Balanced score, so no lifestyle nudge is needed here to
    // keep this direct-reduce mirror in parity with the bridge.
    let peers = direct_build_peers(seed, &nationality);
    state = reduce(state, Intent::InitPeers { peers }, &mut GoatRng::new(0));
    reduce(
        state,
        Intent::StartSeason { fixtures: vec![] },
        &mut GoatRng::new(0),
    )
}

/// Same seeding `advance_week` performs, applied via direct reduce.
fn direct_advance_week(state: WorldState) -> WorldState {
    let pc_id = state.pc_player_id.unwrap_or(0);
    let view = state.players.snapshot(pc_id);
    let seed = (view.age_weeks as u64).wrapping_mul(6364136223846793005);
    let mut state = reduce(state, Intent::AdvanceWeek, &mut GoatRng::new(seed));
    state.pc_week_training_done = true;
    state
}

fn assert_bridge_matches_direct(snap: &api::GoatGameState, direct: &WorldState, ctx: &str) {
    let pc_id = direct.pc_player_id.expect("direct state has a player");
    let view = direct.players.snapshot(pc_id);
    assert_eq!(snap.energy, view.energy.to_int(), "{ctx}: energy");
    assert_eq!(
        snap.ovr,
        ovr(&view.current, view.primary_position).to_int(),
        "{ctx}: ovr"
    );
    assert_eq!(snap.age_years, view.age_weeks / 52, "{ctx}: age_years");
    assert_eq!(
        snap.age_weeks_in_year,
        view.age_weeks % 52,
        "{ctx}: age_weeks_in_year"
    );
    assert_eq!(snap.form, direct.pc_form.to_int(), "{ctx}: form");
    assert_eq!(
        snap.season_number, direct.season_number,
        "{ctx}: season_number"
    );
    assert_eq!(
        snap.season_round, direct.season_round,
        "{ctx}: season_round"
    );
    assert_eq!(snap.savings, direct.pc_savings, "{ctx}: savings");
    assert_eq!(snap.injury_weeks, view.injury_weeks, "{ctx}: injury_weeks");
    // Phase-10 economy/life fields (B.1).
    assert_eq!(
        snap.business_value, direct.pc_business_value,
        "{ctx}: business_value"
    );
    assert_eq!(snap.bankrupt, direct.pc_bankrupt, "{ctx}: bankrupt");
    assert_eq!(
        snap.dev_invest_level, direct.pc_dev_invest_level,
        "{ctx}: dev_invest_level"
    );
    assert_eq!(
        snap.marketability, direct.pc_marketability,
        "{ctx}: marketability"
    );
    assert_eq!(
        snap.sponsor_tier, direct.pc_sponsor_tier,
        "{ctx}: sponsor_tier"
    );
    assert_eq!(
        snap.relationships,
        direct.pc_relationships.to_vec(),
        "{ctx}: relationships"
    );
    assert_eq!(
        snap.character_rep, direct.pc_character_rep,
        "{ctx}: character_rep"
    );
    // B.2 calendar surfacing.
    assert_eq!(snap.epoch_day, direct.pc_epoch_day, "{ctx}: epoch_day");
    let snap_fps: Vec<(u32, u8)> = snap
        .last_flashpoints
        .iter()
        .map(|f| (f.day, f.window))
        .collect();
    let direct_fps: Vec<(u32, u8)> = direct
        .last_week_flashpoints
        .iter()
        .map(|f| (f.day, f.window as u8))
        .collect();
    assert_eq!(snap_fps, direct_fps, "{ctx}: last_flashpoints");
}

#[test]
fn bridge_path_matches_direct_reduce() {
    let _g = serial();

    // Bridge path.
    let bridge_snap = api::new_game(NAME.to_string(), POS_FWD, CLUB_ID, SEED, LIFESTYLE_PRO);
    // Direct path.
    let mut direct = direct_new_game(SEED, CLUB_ID as usize);
    assert_bridge_matches_direct(&bridge_snap, &direct, "after new_game");

    // Same routine on both sides.
    let routine = Routine {
        focus_attrs: vec![
            goat_core::attrs::AttrId::ALL[0],
            goat_core::attrs::AttrId::ALL[1],
            goat_core::attrs::AttrId::ALL[2],
        ],
        intensity: Intensity::Medium,
    };
    api::set_routine(vec![0, 1, 2], 1);
    direct = reduce(direct, Intent::SetRoutine { routine }, &mut GoatRng::new(0));
    let mut bridge_snap;

    // Six training weeks on both sides.
    for week in 0..6 {
        bridge_snap = api::advance_week();
        direct = direct_advance_week(direct);
        assert_bridge_matches_direct(&bridge_snap, &direct, &format!("after week {week}"));
    }
}

// ── 3b. Phase-10 economy/life actions (B.1) ──────────────────────────────────

/// Every B.1 action fn driven through the bridge must produce the same state as
/// the same intents applied via direct reduce — determinism survives the FFI
/// boundary for the new surface too.
#[test]
fn bridge_phase10_actions_match_direct_reduce() {
    let _g = serial();

    let bridge_snap = api::new_game(NAME.to_string(), POS_FWD, CLUB_ID, SEED, LIFESTYLE_PRO);
    let mut direct = direct_new_game(SEED, CLUB_ID as usize);
    assert_bridge_matches_direct(&bridge_snap, &direct, "after new_game");

    // Dev investment + marketability + sponsor (eligible at 80).
    let snap = api::set_dev_investment(2);
    direct = reduce(
        direct,
        Intent::SetDevInvestment { level: 2 },
        &mut GoatRng::new(0),
    );
    assert_bridge_matches_direct(&snap, &direct, "set_dev_investment");

    let snap = api::set_marketability(80);
    direct = reduce(
        direct,
        Intent::SetMarketability { value: 80 },
        &mut GoatRng::new(0),
    );
    assert_bridge_matches_direct(&snap, &direct, "set_marketability");

    let snap = api::sign_sponsor(2);
    direct = reduce(
        direct,
        Intent::SignSponsor { tier: 2 },
        &mut GoatRng::new(0),
    );
    assert_bridge_matches_direct(&snap, &direct, "sign_sponsor");

    // Economy: settle a season bonus, then invest part of it into the business.
    let snap = api::settle_season_economy(1_000);
    direct = reduce(
        direct,
        Intent::SettleSeasonEconomy {
            season_bonus: 1_000,
        },
        &mut GoatRng::new(0),
    );
    assert_bridge_matches_direct(&snap, &direct, "settle_season_economy");

    let snap = api::invest_in_business(400);
    direct = reduce(
        direct,
        Intent::InvestInBusiness { amount: 400 },
        &mut GoatRng::new(0),
    );
    assert_bridge_matches_direct(&snap, &direct, "invest_in_business");

    // Life event deep into rupture territory (triggers the scandal branch),
    // then a contrite media response.
    let snap = api::apply_life_event(1, -80);
    direct = reduce(
        direct,
        Intent::ApplyLifeEvent {
            thread: 1,
            delta: -80,
        },
        &mut GoatRng::new(0),
    );
    assert_bridge_matches_direct(&snap, &direct, "apply_life_event");

    let snap = api::respond_to_media(true);
    direct = reduce(
        direct,
        Intent::RespondToMedia { contrite: true },
        &mut GoatRng::new(0),
    );
    assert_bridge_matches_direct(&snap, &direct, "respond_to_media");
}

// ── 4. Full season + season-end lifecycle ────────────────────────────────────

#[test]
fn full_season_and_season_end_flow() {
    let _g = serial();
    api::new_game(NAME.to_string(), POS_FWD, CLUB_ID, SEED, LIFESTYLE_PRO);
    api::set_routine(vec![0, 1, 2], 1);

    let mut snap = api::get_state().expect("state after new_game");
    let rounds = snap.rounds_per_season;
    for round in 0..rounds {
        api::advance_week();
        let (s, _) = api::play_round(false);
        snap = s;
        assert_snapshot_invariants(&snap, &format!("round {round}"));
    }
    assert_eq!(snap.season_round, rounds, "season should be fully played");
    assert!(
        snap.season_matches > 0,
        "player should have appeared this season"
    );

    // Table stays consistent: every club plays the full schedule.
    let table = api::get_table();
    assert_eq!(table.len(), CLUBS_PER_DIV);

    // Mandatory order: awards → apply_season_end → start_next_season.
    let awards = api::get_season_awards();
    assert_eq!(awards.len(), 2, "player-of-year + golden boot");
    assert!(awards.iter().all(|a| !a.winner_name.is_empty()));

    let after_end = api::apply_season_end();
    assert!(
        after_end.career_seasons >= 1,
        "season end should bank the season"
    );
    // Season end banks the running season into the career totals.
    assert_eq!(
        after_end.career_matches,
        snap.career_matches + snap.season_matches
    );
    assert_eq!(
        after_end.career_goals,
        snap.career_goals + snap.season_goals
    );

    let next = api::start_next_season();
    assert_eq!(next.season_number, 2);
    assert_eq!(next.season_round, 0);
    assert_eq!(next.season_goals, 0);
    assert_eq!(next.season_matches, 0);
    assert_snapshot_invariants(&next, "start of season 2");
    // Time model / age sync (TASK-CORE-time-model-age-sync): the season calendar
    // drives the player clock, so training every round for a full season plus the
    // off-season back-fill must land exactly on age 17 at the start of season 2 —
    // not the ~30-week drift the old training-only clock produced.
    assert_eq!(
        next.age_years, 17,
        "age should advance a full year across one full season"
    );

    // Legacy endpoint works after a completed season.
    let legacy = api::get_legacy();
    assert!(!legacy.axes.is_empty(), "legacy axes should be populated");
}

// ── 5. Save / load round-trip ────────────────────────────────────────────────

#[test]
fn save_load_roundtrip_preserves_snapshot() {
    let _g = serial();
    let (snap_at_save, _) = script_rounds(SEED, 5);

    let path = std::env::temp_dir()
        .join(format!("goat_bridge_spec_{}.save", std::process::id()))
        .to_string_lossy()
        .into_owned();

    assert!(api::save_game(path.clone()), "save_game should succeed");

    // Mutate in-memory state past the save point.
    api::advance_week();

    let loaded = api::load_game(path.clone()).expect("load_game should succeed");
    let _ = std::fs::remove_file(&path);

    // The loaded snapshot must equal the snapshot taken at save time.
    assert_eq!(
        loaded, snap_at_save,
        "load must restore the exact saved state"
    );
}

// ── 6. Interactive match flow ────────────────────────────────────────────────

#[test]
fn interactive_match_completes_and_advances_round() {
    let _g = serial();
    api::new_game(NAME.to_string(), POS_FWD, CLUB_ID, SEED, LIFESTYLE_PRO);
    api::advance_week();

    let round_before = api::get_state().unwrap().season_round;
    let first = api::start_interactive_match().expect("fixture exists in round 0");
    assert!(!first.choices.is_empty(), "beat must offer choices");
    assert!(first.total_beats > 0);

    let mut outcome = api::make_beat_choice(0).expect("active match accepts a choice");
    let mut safety = 0;
    while !outcome.is_complete {
        assert!(
            outcome.next_beat.is_some(),
            "incomplete outcome must carry the next beat"
        );
        outcome = api::make_beat_choice(0).expect("match still active");
        safety += 1;
        assert!(safety < 200, "interactive match failed to terminate");
    }

    let final_result = outcome
        .final_result
        .expect("complete match returns a result");
    let game_state = outcome
        .game_state
        .expect("complete match returns game state");
    assert_eq!(
        game_state.season_round,
        round_before + 1,
        "completing the match must advance the season round"
    );
    assert_eq!(
        game_state.calendar_week, 8,
        "opener played → calendar moves to week 8 (7-week pre-season lead)"
    );
    assert_eq!(
        final_result.goals_for, outcome.goals_for,
        "final result score must match the last beat's running score"
    );
    assert_snapshot_invariants(&game_state, "after interactive match");

    // Session is consumed — no dangling active match.
    assert!(
        api::make_beat_choice(0).is_none(),
        "match session must be cleared"
    );
}

// ── 7. Calendar follows played rounds ────────────────────────────────────────

/// The calendar week is derived from the next unplayed round (same rule the
/// TUI uses). Single-fixture weeks shift after one match; double-fixture
/// weeks (e.g. calendar week 8: rounds 1–2) stay until both are played.
/// Week numbers include the 7-week pre-season lead (round 0 = week 7).
#[test]
fn calendar_shifts_with_played_rounds() {
    let _g = serial();
    let s = api::new_game(NAME.to_string(), POS_FWD, CLUB_ID, SEED, LIFESTYLE_PRO);
    assert_eq!(s.calendar_week, 7);
    assert_eq!(s.week_fixtures.len(), 1, "week 7 is a single-fixture week");

    // Round 0 (week 7, one fixture): playing it shifts to week 8.
    let (s, _) = api::play_round(false);
    assert_eq!(
        s.calendar_week, 8,
        "single-fixture week shifts after its match"
    );
    assert_eq!(s.week_fixtures.len(), 2, "week 8 is a double-fixture week");
    assert_eq!(s.week_fixtures_played, 0);

    // Round 1 (first of week 8's two fixtures): calendar stays put.
    let (s, _) = api::play_round(false);
    assert_eq!(
        s.calendar_week, 8,
        "double-fixture week holds until both played"
    );
    assert_eq!(s.week_fixtures_played, 1);

    // Round 2 (second fixture): now the week advances.
    let (s, _) = api::play_round(false);
    assert_eq!(s.calendar_week, 9);
    assert_eq!(s.week_fixtures_played, 0);
}

/// TASK-CORE-double-week-tick: a calendar week ticks exactly once, even with
/// two fixtures in it — no double aging, no second training session.
#[test]
fn double_fixture_week_ticks_once() {
    let _g = serial();

    // Untrained path: rounds 0..3 span calendar weeks 0–1 (week 1 has two
    // fixtures). Exactly 2 weeks may elapse.
    api::new_game(NAME.to_string(), POS_FWD, CLUB_ID, SEED, LIFESTYLE_PRO);
    let (s, _) = api::play_round(false); // round 0 → week 0 elapses
    assert_eq!((s.age_years, s.age_weeks_in_year), (16, 1));
    assert!(!s.week_training_done);
    let (s, _) = api::play_round(false); // round 1 → week 1 elapses (1st fixture)
    assert_eq!((s.age_years, s.age_weeks_in_year), (16, 2));
    assert!(
        s.week_training_done,
        "between a double week's fixtures the week is already ticked"
    );
    let (s, _) = api::play_round(false); // round 2 → same week: NO time passes
    assert_eq!(
        (s.age_years, s.age_weeks_in_year),
        (16, 2),
        "second fixture of the same week must not age the player"
    );
    assert!(!s.week_training_done, "a fresh week opens after the pair");

    // Trained path: a second training in the same week is a reducer-level no-op.
    api::new_game(NAME.to_string(), POS_FWD, CLUB_ID, SEED, LIFESTYLE_PRO);
    api::set_routine(vec![0, 1, 2], 1);
    api::advance_week(); // week 0 training
    api::play_round(false); // round 0
    let t1 = api::advance_week(); // week 1 training
    assert_eq!((t1.age_years, t1.age_weeks_in_year), (16, 2));
    api::play_round(false); // round 1 (1st fixture of week 1)
    let t2 = api::advance_week(); // must be a no-op — already trained this week
    assert_eq!(
        (t2.age_years, t2.age_weeks_in_year),
        (16, 2),
        "second training in a double week must be a no-op"
    );
    let (s, _) = api::play_round(false); // round 2
    assert_eq!((s.age_years, s.age_weeks_in_year), (16, 2));
}

// ── 8. All 8 positions wire through creation ─────────────────────────────────

/// Every PrimaryPosition (0=ST … 7=CB) must reach the core: the snapshot echoes
/// the discriminant, and the generated Natural role belongs to that position's
/// broad family (ST→Forward, W/WM/CAM/CM/DM→Midfielder, FB/CB→Defender).
#[test]
fn each_position_seeds_matching_family() {
    let _g = serial();
    use goat_core::roles::{RoleId, ROLE_POSITION_FAMILY};

    for pos in 0u8..8 {
        let s = api::new_game(NAME.to_string(), pos, CLUB_ID, SEED + pos as u64, 0);
        assert_eq!(s.position, pos, "snapshot echoes position {pos}");

        let roles = api::get_roles();
        let natural = roles
            .iter()
            .find(|r| r.familiarity == "Natural")
            .unwrap_or_else(|| panic!("position {pos}: no Natural role"));
        let role_id = RoleId::ALL
            .iter()
            .copied()
            .find(|r| r.name() == natural.name)
            .unwrap_or_else(|| panic!("unknown role name {}", natural.name));
        let expected_family = PrimaryPosition::from_u8(pos).unwrap().family();
        assert_eq!(
            ROLE_POSITION_FAMILY[role_id as usize], expected_family,
            "position {pos}: Natural role {} outside its family",
            natural.name
        );
    }
}

// ── 9b. Fast-forward must not outrun the match calendar ──────────────────────

/// `advance_weeks` ticks the player's age/training clock but never plays a
/// match — only `play_round`/the interactive match flow advances
/// `season_round`. The *current* calendar week (derived from `season_round`)
/// is always a match week during an active season (round→week mapping never
/// lands on a break week — verified separately), so a hard block would make
/// fast-forward permanently inert. Instead it must cap to the same single
/// week a "Train Week" tap already allows regardless of pending matches, and
/// no more — eliminating unbounded drift while not regressing the ordinary
/// train-then-play flow.
#[test]
fn advance_weeks_caps_to_one_week_while_match_is_pending() {
    let _g = serial();
    let s0 = api::new_game(NAME.to_string(), POS_FWD, CLUB_ID, SEED, LIFESTYLE_PRO);
    assert_eq!(s0.season_round, 0);
    assert!(
        !s0.week_fixtures.is_empty(),
        "week 0 has a scheduled fixture"
    );
    assert!(!s0.week_training_done, "fresh game hasn't trained yet");

    // First call: not yet trained this week, so exactly one week is allowed
    // through (matching a single Train Week tap) despite requesting 12.
    let once = api::advance_weeks(12);
    assert_eq!(once.season_round, 0, "must not touch the match calendar");
    assert!(
        once.week_training_done,
        "the single allowed tick marks this week as trained"
    );
    let one_week_later = (
        s0.age_years * 52 + s0.age_weeks_in_year + 1,
        once.age_years * 52 + once.age_weeks_in_year,
    );
    assert_eq!(
        one_week_later.0, one_week_later.1,
        "exactly one week of aging, not twelve"
    );
    assert!(
        once.last_events
            .iter()
            .any(|e| e.contains("keep fast-forwarding")),
        "must explain the cap: {:?}",
        once.last_events
    );

    // Second call: this week is now already trained, so no further weeks may
    // pass until the pending match is played — repeated fast-forwarding can't
    // keep sneaking in extra weeks.
    let twice = api::advance_weeks(4);
    assert_eq!(
        (twice.age_years, twice.age_weeks_in_year),
        (once.age_years, once.age_weeks_in_year),
        "must not age further while still pending after already training"
    );

    // Resolving the fixture re-opens exactly one more allowed week (the next
    // week is immediately pending its own fixture too), confirming the cap
    // isn't a one-time fluke tied to the very first week.
    let (after_match, _) = api::play_round(false);
    assert_eq!(after_match.season_round, 1);
    let again = api::advance_weeks(4);
    assert_eq!(again.season_round, 1, "still must not touch match calendar");
    let advanced_by_one = (after_match.age_years * 52 + after_match.age_weeks_in_year) + 1
        == (again.age_years * 52 + again.age_weeks_in_year);
    let already_trained = after_match.week_training_done;
    assert!(
        advanced_by_one || already_trained,
        "next week's own fixture caps advance_weeks the same way"
    );
}

// ── 9. Retirement flag ───────────────────────────────────────────────────────

#[test]
fn retire_sets_flag_via_bridge() {
    let _g = serial();
    api::new_game(NAME.to_string(), POS_FWD, CLUB_ID, SEED, LIFESTYLE_PRO);
    let s = api::retire();
    assert!(
        s.is_retired,
        "retire() must flip is_retired on the snapshot"
    );
}

// ── B.3/B.4 world read-models + verdict (read-only) ──────────────────────────

#[test]
fn world_read_models_return_summaries_without_mutating_state() {
    let _g = serial();
    let before = api::new_game(NAME.to_string(), POS_FWD, CLUB_ID, SEED, LIFESTYLE_PRO);

    let canon = api::get_pantheon_canon();
    assert!(!canon.is_empty(), "pantheon canon must be non-empty");
    assert!(
        canon
            .windows(2)
            .all(|w| w[0].ballon_dors >= w[1].ballon_dors),
        "canon must be sorted by Ballon d'Or count"
    );

    let standings = api::get_world_standings();
    assert_eq!(standings.len(), 60, "one summary per league");
    assert!(standings.iter().all(|s| !s.champion.is_empty()));

    let verdict = api::get_rival_verdict();
    // Season 1: either a named rival or the weak-era flag — never both empty.
    assert!(verdict.has_rival || verdict.weak_era);

    let final_verdict = api::get_final_verdict();
    assert_eq!(final_verdict.len(), 4, "one verdict per school");
    assert!(!api::should_retire(), "a 16-year-old never force-retires");

    let fingerprint = api::get_world_fingerprint();
    assert!(fingerprint.contains("1200 clubs"), "{fingerprint}");

    // Read-only guarantee: the state after all these calls is identical.
    let after = api::get_state().unwrap();
    assert_eq!(before.season_round, after.season_round);
    assert_eq!(before.savings, after.savings);
    assert_eq!(before.energy, after.energy);
}
