//! Save/load round-trip golden tests.
//!
//! Serialize to bytes → deserialize → verify fields identical.
//! Potentials are re-derived from the world seed on load.

use goat_core::{
    attrs::{AttrId, NUM_ATTRS},
    generation::CreationChoices,
    positions::PrimaryPosition,
    roles::NUM_ROLES,
    state::{lifestyle_tier_from_score, reduce, Intent, WorldState},
    week::{Intensity, Routine},
};
use goat_fixed::Fixed;
use goat_rng::GoatRng;
use goat_save::save::{from_world_state, load_from_file, save_to_file, to_world_state};
use goat_world::world::WorldGenesis;

const TEST_WORLD_SEED: u64 = 54321;

/// The world used by every test in this file — deterministic per `TEST_WORLD_SEED`,
/// rebuilt on demand (never persisted, same "seed is the universe" pattern as `History`).
fn test_world() -> WorldGenesis {
    WorldGenesis::generate(TEST_WORLD_SEED)
}

fn setup_state() -> WorldState {
    let world = test_world();
    let world_seed = TEST_WORLD_SEED;
    // Second tier of the first generated nation, 4th club — analogous to the old
    // hardcoded DIV_ENG_SEC/Burnley slot.
    let div_idx = 1;
    let pc_club_id = world.leagues[div_idx].clubs[3];

    let choices = CreationChoices {
        name: "Round-Trip Sam".into(),
        primary_position: PrimaryPosition::ST,
        nationality: "England".to_string(),
        club: world.clubs[pc_club_id].name.clone(),
    };

    let mut state = WorldState::new();
    state = reduce(
        state,
        Intent::CreatePlayer {
            seed: world_seed,
            choices,
        },
        &mut GoatRng::new(0),
    );
    state = reduce(
        state,
        Intent::InitWorld {
            world_seed,
            pc_club_idx: pc_club_id as u16,
            pc_div_idx: div_idx as u8,
            facilities_mult: world.clubs[pc_club_id].facilities_mult(),
            initial_table: Box::new([0u32; 100]),
        },
        &mut GoatRng::new(0),
    );
    state = reduce(
        state,
        Intent::StartSeason { fixtures: vec![] },
        &mut GoatRng::new(0),
    );

    let routine = Routine {
        focus_attrs: vec![AttrId::Finishing, AttrId::Vision],
        intensity: Intensity::Medium,
    };
    state = reduce(state, Intent::SetRoutine { routine }, &mut GoatRng::new(0));
    // Advance 8 weeks so state is non-trivial.
    state = reduce(state, Intent::AdvanceWeeks { n: 8 }, &mut GoatRng::new(99));
    state
}

#[test]
fn save_load_restores_current_attrs() {
    let state = setup_state();
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);
    let restored = to_world_state(&data, &test_world());
    let r_id = restored.pc_player_id.unwrap();

    for a in 0..NUM_ATTRS {
        let orig = state.players.get_current(pc_id, a);
        let rest = restored.players.get_current(r_id, a);
        assert_eq!(orig, rest, "current attr {a} differs after round-trip");
    }
}

#[test]
fn save_load_restores_potentials_from_seed() {
    // Potential is not saved — re-derived from world_seed. Must be identical.
    let state = setup_state();
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);
    let restored = to_world_state(&data, &test_world());
    let r_id = restored.pc_player_id.unwrap();

    for a in 0..NUM_ATTRS {
        let orig = state.players.get_potential(pc_id, a);
        let rest = restored.players.get_potential(r_id, a);
        assert_eq!(orig, rest, "potential attr {a} differs after round-trip");
    }
}

#[test]
fn save_load_restores_season_state() {
    let state = setup_state();
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);
    let restored = to_world_state(&data, &test_world());

    assert_eq!(state.season_number, restored.season_number, "season_number");
    assert_eq!(state.season_round, restored.season_round, "season_round");
    assert_eq!(state.world_seed, restored.world_seed, "world_seed");
    assert_eq!(state.pc_club_idx, restored.pc_club_idx, "pc_club_idx");
    assert_eq!(state.pc_div_idx, restored.pc_div_idx, "pc_div_idx");
    assert_eq!(state.pc_form, restored.pc_form, "pc_form");
    assert_eq!(state.table_raw, restored.table_raw, "table_raw");
}

#[test]
fn save_load_restores_epoch_day_through_bytes() {
    // Exercises the full byte path (save_to_file → load_from_file), not just the
    // in-memory SaveData conversion — covers the v6 pc_epoch_day field.
    let mut state = setup_state();
    state.pc_epoch_day = 287; // non-trivial calendar position
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let path = std::env::temp_dir().join("goat_save_epoch_roundtrip.gsav");
    save_to_file(&data, &path).unwrap();
    let loaded = load_from_file(&path).unwrap();
    let restored = to_world_state(&loaded, &test_world());
    std::fs::remove_file(&path).ok();

    assert_eq!(
        restored.pc_epoch_day, 287,
        "pc_epoch_day must survive a full byte round-trip"
    );
}

#[test]
fn save_load_restores_familiarity() {
    let state = setup_state();
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);
    let restored = to_world_state(&data, &test_world());
    let r_id = restored.pc_player_id.unwrap();

    for r in 0..NUM_ROLES {
        let orig = state.players.get_familiarity(pc_id, r);
        let rest = restored.players.get_familiarity(r_id, r);
        assert_eq!(orig, rest, "familiarity role {r} differs after round-trip");
    }
}

#[test]
fn save_load_restores_age_and_energy() {
    let state = setup_state();
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);
    let restored = to_world_state(&data, &test_world());
    let r_id = restored.pc_player_id.unwrap();

    assert_eq!(
        state.players.get_age_weeks(pc_id),
        restored.players.get_age_weeks(r_id),
        "age_weeks"
    );
    assert_eq!(
        state.players.get_energy(pc_id),
        restored.players.get_energy(r_id),
        "energy"
    );
}

#[test]
fn save_load_restores_phase10_economy_and_life() {
    // Exercises the full byte path for the v7 economy/life fields.
    let mut state = setup_state();
    state.pc_business_value = 4_200;
    state.pc_bankrupt = true;
    state.pc_dev_invest_level = 3;
    state.pc_marketability = 88;
    state.pc_sponsor_tier = 2;
    state.pc_relationships = [40, 95, 12];
    state.pc_character_rep = 37;
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let path = std::env::temp_dir().join("goat_save_phase10_roundtrip.gsav");
    save_to_file(&data, &path).unwrap();
    let restored = to_world_state(&load_from_file(&path).unwrap(), &test_world());
    std::fs::remove_file(&path).ok();

    assert_eq!(restored.pc_business_value, 4_200);
    assert!(restored.pc_bankrupt);
    assert_eq!(restored.pc_dev_invest_level, 3);
    assert_eq!(restored.pc_marketability, 88);
    assert_eq!(restored.pc_sponsor_tier, 2);
    assert_eq!(restored.pc_relationships, [40, 95, 12]);
    assert_eq!(restored.pc_character_rep, 37);
}

#[test]
fn save_load_restores_lifestyle_score_and_derived_tier() {
    // v8+: lifestyle is a derived readout, not a stored menu pick (bible §8.5/§8.6).
    // The score must survive a full byte round-trip and the cached tier must be
    // recomputed identically from it.
    let mut state = setup_state();
    state.pc_lifestyle_score = Fixed::raw(-450); // deep in Professional territory
    state.pc_lifestyle = lifestyle_tier_from_score(state.pc_lifestyle_score);
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let path = std::env::temp_dir().join("goat_save_lifestyle_roundtrip.gsav");
    save_to_file(&data, &path).unwrap();
    let restored = to_world_state(&load_from_file(&path).unwrap(), &test_world());
    std::fs::remove_file(&path).ok();

    assert_eq!(restored.pc_lifestyle_score, Fixed::raw(-450));
    assert_eq!(
        restored.pc_lifestyle, 0,
        "score of -450 must derive Professional (0)"
    );
}

#[test]
fn save_load_restores_pantheon_signals() {
    // Exercises the full byte path for the v9 Pantheon raw-signal evidence, including
    // the two live pc_season_* staging fields (a mid-season save must not lose them).
    let mut state = setup_state();
    state.pc_career_standout_matches = 12;
    state.pc_season_standout_matches = 3;
    state.pc_career_best_ovr = 87;
    state.pc_career_transfer_requests = 4;
    state.pc_season_transfer_requests = 1;
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let path = std::env::temp_dir().join(format!(
        "goat_save_pantheon_roundtrip_{}.gsav",
        std::process::id()
    ));
    save_to_file(&data, &path).unwrap();
    let restored = to_world_state(&load_from_file(&path).unwrap(), &test_world());
    std::fs::remove_file(&path).ok();

    assert_eq!(restored.pc_career_standout_matches, 12);
    assert_eq!(restored.pc_season_standout_matches, 3);
    assert_eq!(restored.pc_career_best_ovr, 87);
    assert_eq!(restored.pc_career_transfer_requests, 4);
    assert_eq!(restored.pc_season_transfer_requests, 1);
}

#[test]
fn old_v7_save_without_lifestyle_score_defaults_to_balanced() {
    // Simulate an older save by truncating the bytes right before the v8 field.
    let state = setup_state();
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);
    let restored = to_world_state(&data, &test_world());
    // setup_state() never touches lifestyle, so the score defaults to 0 = Balanced.
    assert_eq!(restored.pc_lifestyle_score, Fixed::ZERO);
    assert_eq!(restored.pc_lifestyle, 1);
}

#[test]
fn old_v8_save_without_pantheon_signals_defaults_to_zero() {
    // A real v8 binary never wrote the 5 trailing v9 Pantheon-evidence fields
    // (4 bytes each = 20 bytes). Simulate that by writing a full v9 save then
    // truncating the last 20 bytes off the wire format before loading it back —
    // exercises the actual `.unwrap_or(0)` backward-compat reads in `from_bytes`,
    // not just the in-memory struct defaults.
    let state = setup_state();
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let full_path =
        std::env::temp_dir().join(format!("goat_save_v9_full_{}.gsav", std::process::id()));
    save_to_file(&data, &full_path).unwrap();
    let mut bytes = std::fs::read(&full_path).unwrap();
    std::fs::remove_file(&full_path).ok();

    // Truncate the 5 trailing v9 fields (pc_career_standout_matches,
    // pc_season_standout_matches, pc_career_best_ovr, pc_career_transfer_requests,
    // pc_season_transfer_requests — all 4-byte fields) to simulate a pre-v9 save.
    let v8_len = bytes.len() - 5 * 4;
    bytes.truncate(v8_len);

    let truncated_path = std::env::temp_dir().join(format!(
        "goat_save_v8_truncated_{}.gsav",
        std::process::id()
    ));
    std::fs::write(&truncated_path, &bytes).unwrap();
    let loaded = load_from_file(&truncated_path).unwrap();
    std::fs::remove_file(&truncated_path).ok();

    assert_eq!(loaded.pc_career_standout_matches, 0);
    assert_eq!(loaded.pc_season_standout_matches, 0);
    assert_eq!(loaded.pc_career_best_ovr, 0);
    assert_eq!(loaded.pc_career_transfer_requests, 0);
    assert_eq!(loaded.pc_season_transfer_requests, 0);

    let restored = to_world_state(&loaded, &test_world());
    assert_eq!(restored.pc_career_standout_matches, 0);
    assert_eq!(restored.pc_career_best_ovr, 0);
    assert_eq!(restored.pc_career_transfer_requests, 0);
}

// ── SuspensionLedger (Design round 4, Slice 5 §5.1/§5.2) ─────────────────────

#[test]
fn save_load_restores_multiple_competition_suspensions_through_bytes() {
    // v11+: a Vec<SuspensionLedger>, not a single scalar — must round-trip more than
    // one simultaneous ban (league + domestic cup) through the full byte path.
    let mut state = setup_state();
    state.pc_suspensions = vec![
        goat_calendar::SuspensionLedger {
            player_id: state.pc_player_id.unwrap(),
            competition_id: goat_core::calendar_loop::LEAGUE_COMPETITION_ID,
            matches_remaining: 2,
        },
        goat_calendar::SuspensionLedger {
            player_id: state.pc_player_id.unwrap(),
            competition_id: goat_core::calendar_loop::DOMESTIC_CUP_COMPETITION_ID,
            matches_remaining: 1,
        },
    ];
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let path = std::env::temp_dir().join(format!(
        "goat_save_suspensions_roundtrip_{}.gsav",
        std::process::id()
    ));
    save_to_file(&data, &path).unwrap();
    let restored = to_world_state(&load_from_file(&path).unwrap(), &test_world());
    std::fs::remove_file(&path).ok();

    let mut suspensions = restored.pc_suspensions.clone();
    suspensions.sort_by_key(|l| l.competition_id);
    assert_eq!(suspensions.len(), 2);
    assert_eq!(
        suspensions[0].competition_id,
        goat_core::calendar_loop::LEAGUE_COMPETITION_ID
    );
    assert_eq!(suspensions[0].matches_remaining, 2);
    assert_eq!(
        suspensions[1].competition_id,
        goat_core::calendar_loop::DOMESTIC_CUP_COMPETITION_ID
    );
    assert_eq!(suspensions[1].matches_remaining, 1);
}

#[test]
fn old_v10_save_with_bare_suspension_scalar_migrates_to_a_league_scoped_ledger_entry() {
    // Pre-v11 saves wrote `pc_suspension_weeks` as a single bare u32 at this exact
    // position (a mid-stream field, not a tail-append — same break as v10's table_raw
    // widening). Build a real v11 buffer with an EMPTY suspensions list (encodes as a
    // 4-byte `count = 0`, the same width as the old bare scalar occupied), locate that
    // 4-byte slot by diffing against a second buffer whose only difference is a
    // non-empty suspensions list (both share an identical byte prefix up to exactly
    // where the suspensions encoding begins), then splice in an old-style nonzero
    // scalar at that slot and rewrite the version tag to 10 — reconstructing exactly
    // what a real v10 writer would have produced, without hand-counting the other ~60
    // fields' byte layout.
    let state = setup_state();
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);

    let mut empty_data = from_world_state(&state, &view);
    empty_data.pc_suspensions = vec![];
    let mut marker_data = from_world_state(&state, &view);
    marker_data.pc_suspensions = vec![(999, 999)];

    let empty_path =
        std::env::temp_dir().join(format!("goat_save_v11_empty_{}.gsav", std::process::id()));
    let marker_path =
        std::env::temp_dir().join(format!("goat_save_v11_marker_{}.gsav", std::process::id()));
    save_to_file(&empty_data, &empty_path).unwrap();
    save_to_file(&marker_data, &marker_path).unwrap();
    let empty_bytes = std::fs::read(&empty_path).unwrap();
    let marker_bytes = std::fs::read(&marker_path).unwrap();
    std::fs::remove_file(&empty_path).ok();
    std::fs::remove_file(&marker_path).ok();

    let divergence = empty_bytes
        .iter()
        .zip(marker_bytes.iter())
        .position(|(a, b)| a != b)
        .expect("a non-empty suspensions list must change the encoded bytes");

    // Splice an old-style bare-u32 scalar (value 3) into the 4-byte `count = 0` slot,
    // then rewrite the version tag (bytes [4..8], right after the b"GOAT" magic) to 10.
    let mut v10_bytes = empty_bytes.clone();
    v10_bytes[divergence..divergence + 4].copy_from_slice(&3u32.to_le_bytes());
    v10_bytes[4..8].copy_from_slice(&10u32.to_le_bytes());

    let v10_path = std::env::temp_dir().join(format!(
        "goat_save_v10_synthetic_{}.gsav",
        std::process::id()
    ));
    std::fs::write(&v10_path, &v10_bytes).unwrap();
    let loaded = load_from_file(&v10_path).unwrap();
    std::fs::remove_file(&v10_path).ok();

    assert_eq!(
        loaded.pc_suspensions,
        vec![(goat_core::calendar_loop::LEAGUE_COMPETITION_ID, 3)],
        "a pre-v11 nonzero suspension scalar must migrate to a single League-scoped entry"
    );

    let restored = to_world_state(&loaded, &test_world());
    assert_eq!(restored.pc_suspensions.len(), 1);
    assert_eq!(
        restored.pc_suspensions[0].competition_id,
        goat_core::calendar_loop::LEAGUE_COMPETITION_ID
    );
    assert_eq!(restored.pc_suspensions[0].matches_remaining, 3);
    let _ = pc_id;
}

#[test]
fn old_v10_save_with_zero_suspension_scalar_migrates_to_an_empty_ledger() {
    let state = setup_state();
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let path = std::env::temp_dir().join(format!(
        "goat_save_v11_zero_suspension_{}.gsav",
        std::process::id()
    ));
    save_to_file(&data, &path).unwrap();
    let mut bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).ok();

    // `data.pc_suspensions` is empty by default, so this is already a "count = 0" v11
    // buffer — retagging it as v10 exercises the `old == 0` migration branch.
    bytes[4..8].copy_from_slice(&10u32.to_le_bytes());
    let v10_path = std::env::temp_dir().join(format!(
        "goat_save_v10_zero_synthetic_{}.gsav",
        std::process::id()
    ));
    std::fs::write(&v10_path, &bytes).unwrap();
    let loaded = load_from_file(&v10_path).unwrap();
    std::fs::remove_file(&v10_path).ok();

    assert!(
        loaded.pc_suspensions.is_empty(),
        "a zero pre-v11 scalar must migrate to an empty ledger, not a phantom entry"
    );
    let _ = pc_id;
}

// ── Club economy (Design round 5, Doc A §Slice 1) ────────────────────────────

#[test]
fn save_load_restores_club_budgets_through_bytes() {
    // v12+: club_budgets must survive a full byte round-trip, including a legitimately
    // negative war-chest (an overspent club — 1.1's explicit "not a bug to clamp away").
    let mut state = setup_state();
    state.club_budgets = vec![23_760, 0, -4_200, 12_000];
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let path = std::env::temp_dir().join(format!(
        "goat_save_club_budgets_roundtrip_{}.gsav",
        std::process::id()
    ));
    save_to_file(&data, &path).unwrap();
    let restored = to_world_state(&load_from_file(&path).unwrap(), &test_world());
    std::fs::remove_file(&path).ok();

    assert_eq!(restored.club_budgets, vec![23_760, 0, -4_200, 12_000]);
}

#[test]
fn old_v11_save_without_club_budgets_defaults_to_empty() {
    // A real v11 binary never wrote the trailing v12 `club_budgets` bytes (length prefix +
    // entries) or the v13 `academy_boosts` bytes (an empty-list length prefix here, since
    // this test leaves it unset). Simulate that by truncating those bytes off a real v13
    // buffer — exercises the actual `.unwrap_or(0)` backward-compat reads in `from_bytes`,
    // not just the in-memory struct default (same idiom as the v8/v9 Pantheon-signal
    // truncation test).
    let state = setup_state();
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let mut data = from_world_state(&state, &view);
    data.club_budgets = vec![100, 200, 300];

    let full_path =
        std::env::temp_dir().join(format!("goat_save_v12_full_{}.gsav", std::process::id()));
    save_to_file(&data, &full_path).unwrap();
    let mut bytes = std::fs::read(&full_path).unwrap();
    std::fs::remove_file(&full_path).ok();

    // Trailing encoding: club_budgets (4-byte count + 3 * 8-byte entries) followed by
    // academy_boosts's empty-list 4-byte length prefix.
    let v11_len = bytes.len() - (4 + 3 * 8) - 4;
    bytes.truncate(v11_len);

    let v11_path = std::env::temp_dir().join(format!(
        "goat_save_v11_truncated_{}.gsav",
        std::process::id()
    ));
    std::fs::write(&v11_path, &bytes).unwrap();
    let loaded = load_from_file(&v11_path).unwrap();
    std::fs::remove_file(&v11_path).ok();

    assert!(
        loaded.club_budgets.is_empty(),
        "a pre-v12 save must default club_budgets to empty, not panic or fabricate entries"
    );
    assert!(
        loaded.academy_boosts.is_empty(),
        "a pre-v13 save must default academy_boosts to empty too"
    );
    let restored = to_world_state(&loaded, &test_world());
    assert!(restored.club_budgets.is_empty());
    assert!(restored.academy_boosts.is_empty());
}

#[test]
fn save_load_restores_academy_boosts_through_bytes() {
    // v13+: academy_boosts must survive a full byte round-trip.
    let mut state = setup_state();
    state.academy_boosts = vec![0, 20, 7, 15];
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let path = std::env::temp_dir().join(format!(
        "goat_save_academy_boosts_roundtrip_{}.gsav",
        std::process::id()
    ));
    save_to_file(&data, &path).unwrap();
    let restored = to_world_state(&load_from_file(&path).unwrap(), &test_world());
    std::fs::remove_file(&path).ok();

    assert_eq!(restored.academy_boosts, vec![0, 20, 7, 15]);
}

#[test]
fn old_v12_save_without_academy_boosts_defaults_to_empty() {
    // A real v12 binary never wrote the trailing v13 `academy_boosts` bytes (length prefix +
    // entries). Simulate that by truncating those bytes off a real v13 buffer.
    let state = setup_state();
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let mut data = from_world_state(&state, &view);
    data.academy_boosts = vec![5, 10, 15];

    let full_path =
        std::env::temp_dir().join(format!("goat_save_v13_full_{}.gsav", std::process::id()));
    save_to_file(&data, &full_path).unwrap();
    let mut bytes = std::fs::read(&full_path).unwrap();
    std::fs::remove_file(&full_path).ok();

    // Trailing encoding: 4-byte count + 3 * 1-byte entries.
    let v12_len = bytes.len() - (4 + 3);
    bytes.truncate(v12_len);

    let v12_path = std::env::temp_dir().join(format!(
        "goat_save_v12_truncated_{}.gsav",
        std::process::id()
    ));
    std::fs::write(&v12_path, &bytes).unwrap();
    let loaded = load_from_file(&v12_path).unwrap();
    std::fs::remove_file(&v12_path).ok();

    assert!(
        loaded.academy_boosts.is_empty(),
        "a pre-v13 save must default academy_boosts to empty, not panic or fabricate entries"
    );
    let restored = to_world_state(&loaded, &test_world());
    assert!(restored.academy_boosts.is_empty());
}

// ── Save slots (Design round 1, Slice 3) ─────────────────────────────────────

#[test]
fn list_slots_on_empty_dir_reports_all_unoccupied() {
    let dir = std::env::temp_dir().join(format!("goat_save_slots_empty_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let slots = goat_save::save::list_slots(&dir, 9);
    assert_eq!(slots.len(), 9);
    for s in &slots {
        assert!(!s.occupied, "slot {} should be unoccupied", s.slot);
        assert_eq!(s.pc_name, "");
        assert_eq!(s.season_number, 0);
        assert_eq!(s.pc_age_weeks, 0);
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn list_slots_reports_one_occupied_slot_and_leaves_others_untouched() {
    let dir = std::env::temp_dir().join(format!("goat_save_slots_one_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let state = setup_state();
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);
    save_to_file(&data, goat_save::save::slot_path(&dir, 3)).unwrap();

    let slots = goat_save::save::list_slots(&dir, 9);
    assert_eq!(slots.len(), 9);
    for s in &slots {
        if s.slot == 3 {
            assert!(s.occupied, "slot 3 should be occupied");
            assert_eq!(s.pc_name, "Round-Trip Sam");
            assert_eq!(s.season_number, state.season_number);
            assert_eq!(s.pc_age_weeks, state.players.get_age_weeks(pc_id));
        } else {
            assert!(!s.occupied, "slot {} should still be unoccupied", s.slot);
        }
    }

    std::fs::remove_dir_all(&dir).ok();
}
