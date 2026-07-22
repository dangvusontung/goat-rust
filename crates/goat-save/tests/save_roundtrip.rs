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
    state = reduce(state, Intent::StartSeason, &mut GoatRng::new(0));

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
