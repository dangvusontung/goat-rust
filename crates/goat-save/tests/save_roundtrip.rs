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
    // entries), the v13 `academy_boosts` bytes, or the v14 manager-pool bytes (each an
    // empty-list length prefix here, since this test leaves them unset). Simulate that by
    // truncating those bytes off a real v14 buffer — exercises the actual `.unwrap_or(0)`
    // backward-compat reads in `from_bytes`, not just the in-memory struct default (same
    // idiom as the v8/v9 Pantheon-signal truncation test).
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

    // Trailing encoding: club_budgets (4-byte count + 3 * 8-byte entries), followed by
    // academy_boosts's empty-list 4-byte length prefix, followed by v14's manager section:
    // manager_blob's own 4-byte byte-length prefix + its empty-pool 4-byte inner manager
    // count, then club_manager's and free_agents' empty-list 4-byte length prefixes,
    // then v15's two assist u32s (BL5.1), v16's decisive-moments u32 (BL5.2), and
    // v17's two clutch-index u32s (BL5.3).
    let v11_len = bytes.len() - (4 + 3 * 8) - 4 - (4 + 4 + 4 + 4) - 2 * 4 - 4 - 2 * 4 - 4 - 4;
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
    assert!(
        loaded.manager_blob.is_empty()
            && loaded.club_manager.is_empty()
            && loaded.free_agents.is_empty(),
        "a pre-v14 save must default the manager pool to empty too"
    );
    let restored = to_world_state(&loaded, &test_world());
    assert!(restored.club_budgets.is_empty());
    assert!(restored.academy_boosts.is_empty());
    assert!(
        restored.managers.is_empty()
            && restored.club_manager.is_empty()
            && restored.free_agents.is_empty()
    );
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
    // entries), or the v14 manager-pool bytes. Simulate that by truncating those bytes off a
    // real v14 buffer.
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

    // Trailing encoding: academy_boosts (4-byte count + 3 * 1-byte entries), followed by
    // v14's manager section: manager_blob's own 4-byte byte-length prefix + its empty-pool
    // 4-byte inner manager count, then club_manager's and free_agents' empty-list 4-byte
    // length prefixes, then v15's two assist u32s (BL5.1) and v16's decisive-moments
    // u32 (BL5.2), and v17's two clutch-index u32s (BL5.3).
    let v12_len = bytes.len() - (4 + 3) - (4 + 4 + 4 + 4) - 2 * 4 - 4 - 2 * 4 - 4 - 4;
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
    assert!(
        loaded.manager_blob.is_empty()
            && loaded.club_manager.is_empty()
            && loaded.free_agents.is_empty(),
        "a pre-v14 save must default the manager pool to empty too"
    );
    let restored = to_world_state(&loaded, &test_world());
    assert!(restored.academy_boosts.is_empty());
    assert!(
        restored.managers.is_empty()
            && restored.club_manager.is_empty()
            && restored.free_agents.is_empty()
    );
}

// ── Managers (Design round 5, Slice 7-8) ─────────────────────────────────────

#[test]
fn old_v13_save_without_managers_defaults_to_empty() {
    // A real v13 binary never wrote the trailing v14 manager-pool bytes at all. Simulate
    // that by truncating just those bytes off a real v14 buffer, isolating the v13->v14
    // boundary specifically (unlike the v11/v12 tests above, which also strip the earlier
    // v12/v13 fields).
    use goat_core::roles::NUM_ROLES;
    use goat_core::state::{ManagerState, MANAGER_FORM_WINDOW};
    use goat_core::tactical_identity::TacticalIdentity;

    let mut state = setup_state();
    state.managers = vec![ManagerState {
        name: "Should Vanish".to_string(),
        identity_bias: TacticalIdentity {
            role_weight: [Fixed::from_int(1); NUM_ROLES],
        },
        recent_points: [0u8; MANAGER_FORM_WINDOW],
        recent_idx: 0,
        tenure_start_season: 1,
        matches_played: 5,
    }];
    state.club_manager = vec![0];
    state.free_agents = vec![];

    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let full_path =
        std::env::temp_dir().join(format!("goat_save_v13_full2_{}.gsav", std::process::id()));
    save_to_file(&data, &full_path).unwrap();
    let mut bytes = std::fs::read(&full_path).unwrap();
    std::fs::remove_file(&full_path).ok();

    // Trailing encoding: manager_blob's own 4-byte byte-length prefix + its 1-manager
    // content, then club_manager's 4-byte count + 1 entry (4 bytes), then free_agents'
    // empty-list 4-byte length prefix, then v15's two assist u32s (BL5.1) and v16's
    // decisive-moments u32 (BL5.2), and v17's two clutch-index u32s (BL5.3).
    let manager_blob_len = data.manager_blob.len();
    let v13_len = bytes.len() - (4 + manager_blob_len) - (4 + 4) - 4 - 2 * 4 - 4 - 2 * 4 - 4 - 4;
    bytes.truncate(v13_len);

    let v13_path = std::env::temp_dir().join(format!(
        "goat_save_v13_truncated_{}.gsav",
        std::process::id()
    ));
    std::fs::write(&v13_path, &bytes).unwrap();
    let loaded = load_from_file(&v13_path).unwrap();
    std::fs::remove_file(&v13_path).ok();

    assert!(
        loaded.manager_blob.is_empty()
            && loaded.club_manager.is_empty()
            && loaded.free_agents.is_empty(),
        "a pre-v14 save must default the manager pool to empty, not panic or fabricate entries"
    );
    let restored = to_world_state(&loaded, &test_world());
    assert!(
        restored.managers.is_empty()
            && restored.club_manager.is_empty()
            && restored.free_agents.is_empty()
    );
}

#[test]
fn save_load_restores_managers_through_bytes() {
    // v14+: the manager pool (managers, club_manager, free_agents) must survive a full byte
    // round-trip, including a manager's non-default tactical identity, mid-window ring
    // buffer, and a nonzero tenure/matches_played.
    use goat_core::roles::NUM_ROLES;
    use goat_core::state::{ManagerState, MANAGER_FORM_WINDOW};
    use goat_core::tactical_identity::TacticalIdentity;

    let mut state = setup_state();
    let mut role_weight = [Fixed::from_int(1); NUM_ROLES];
    role_weight[0] = Fixed::raw(1_600);
    role_weight[1] = Fixed::raw(400);
    let mut recent_points = [0u8; MANAGER_FORM_WINDOW];
    recent_points[0] = 3;
    recent_points[1] = 1;
    state.managers = vec![
        ManagerState {
            name: "Round-Trip Manager".to_string(),
            identity_bias: TacticalIdentity { role_weight },
            recent_points,
            recent_idx: 2,
            tenure_start_season: 5,
            matches_played: 27,
        },
        ManagerState {
            name: "Free Agent Manager".to_string(),
            identity_bias: TacticalIdentity {
                role_weight: [Fixed::from_int(1); NUM_ROLES],
            },
            recent_points: [0u8; MANAGER_FORM_WINDOW],
            recent_idx: 0,
            tenure_start_season: 0,
            matches_played: 0,
        },
    ];
    state.club_manager = vec![0];
    state.free_agents = vec![1];

    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let path = std::env::temp_dir().join(format!(
        "goat_save_managers_roundtrip_{}.gsav",
        std::process::id()
    ));
    save_to_file(&data, &path).unwrap();
    let restored = to_world_state(&load_from_file(&path).unwrap(), &test_world());
    std::fs::remove_file(&path).ok();

    assert_eq!(restored.managers.len(), 2);
    assert_eq!(restored.managers[0].name, "Round-Trip Manager");
    assert_eq!(restored.managers[0].identity_bias.role_weight, role_weight);
    assert_eq!(restored.managers[0].recent_points, recent_points);
    assert_eq!(restored.managers[0].recent_idx, 2);
    assert_eq!(restored.managers[0].tenure_start_season, 5);
    assert_eq!(restored.managers[0].matches_played, 27);
    assert_eq!(restored.managers[1].name, "Free Agent Manager");
    assert_eq!(restored.club_manager, vec![0]);
    assert_eq!(restored.free_agents, vec![1]);
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

// ── Goal/assist split (BL5.1, v15) ───────────────────────────────────────────

#[test]
fn save_load_restores_assists_through_bytes() {
    // v15+: pc_season_assists (live mid-season counter) and pc_career_assists must
    // survive a full byte round-trip, same idiom as the goals counters.
    let mut state = setup_state();
    state.pc_season_assists = 4;
    state.pc_career_assists = 31;
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let path = std::env::temp_dir().join(format!(
        "goat_save_assists_roundtrip_{}.gsav",
        std::process::id()
    ));
    save_to_file(&data, &path).unwrap();
    let restored = to_world_state(&load_from_file(&path).unwrap(), &test_world());
    std::fs::remove_file(&path).ok();

    assert_eq!(restored.pc_season_assists, 4);
    assert_eq!(restored.pc_career_assists, 31);
}

#[test]
fn old_v14_save_without_assists_defaults_to_zero() {
    // A real v14 binary never wrote the two trailing v15 assist u32s (8 bytes).
    // Simulate that by truncating them off a real v15 buffer — exercises the actual
    // `.unwrap_or(0)` backward-compat reads in `from_bytes` (same idiom as the
    // v8→v9 Pantheon-signal truncation test).
    let mut state = setup_state();
    state.pc_season_assists = 7;
    state.pc_career_assists = 42;
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let full_path =
        std::env::temp_dir().join(format!("goat_save_v15_full_{}.gsav", std::process::id()));
    save_to_file(&data, &full_path).unwrap();
    let mut bytes = std::fs::read(&full_path).unwrap();
    std::fs::remove_file(&full_path).ok();

    // Trailing encoding: pc_season_assists + pc_career_assists (2 × 4-byte u32),
    // then v16's pc_season_decisive_moments (4-byte u32).
    let v14_len = bytes.len() - 2 * 4 - 4 - 2 * 4 - 4 - 4;
    bytes.truncate(v14_len);

    let v14_path = std::env::temp_dir().join(format!(
        "goat_save_v14_truncated_{}.gsav",
        std::process::id()
    ));
    std::fs::write(&v14_path, &bytes).unwrap();
    let loaded = load_from_file(&v14_path).unwrap();
    std::fs::remove_file(&v14_path).ok();

    assert_eq!(loaded.pc_season_assists, 0);
    assert_eq!(loaded.pc_career_assists, 0);
    assert_eq!(
        loaded.pc_season_decisive_moments, 0,
        "a pre-v16 save must default the decisive-moments staging counter too"
    );

    let restored = to_world_state(&loaded, &test_world());
    assert_eq!(restored.pc_season_assists, 0);
    assert_eq!(restored.pc_career_assists, 0);
}

#[test]
fn save_load_restores_decisive_moments_through_bytes() {
    // v16+: the live season staging counter must survive a full byte round-trip,
    // same idiom as every other pc_season_* staging field.
    let mut state = setup_state();
    state.pc_season_decisive_moments = 9;
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let path = std::env::temp_dir().join(format!(
        "goat_save_decisive_roundtrip_{}.gsav",
        std::process::id()
    ));
    save_to_file(&data, &path).unwrap();
    let restored = to_world_state(&load_from_file(&path).unwrap(), &test_world());
    std::fs::remove_file(&path).ok();

    assert_eq!(restored.pc_season_decisive_moments, 9);
}

#[test]
fn old_v15_save_without_decisive_moments_defaults_to_zero() {
    // A real v15 binary never wrote the trailing v16 u32 (4 bytes). Simulate that
    // by truncating it off a real v16 buffer — exercises the actual `.unwrap_or(0)`
    // backward-compat read in `from_bytes` (same idiom as the v14/v15 test above).
    let mut state = setup_state();
    state.pc_season_decisive_moments = 5;
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let full_path =
        std::env::temp_dir().join(format!("goat_save_v16_full_{}.gsav", std::process::id()));
    save_to_file(&data, &full_path).unwrap();
    let mut bytes = std::fs::read(&full_path).unwrap();
    std::fs::remove_file(&full_path).ok();

    // Trailing encoding: pc_season_decisive_moments (1 × 4-byte u32), then v17's
    // two clutch-index u32s (BL5.3).
    let v15_len = bytes.len() - 4 - 2 * 4 - 4 - 4;
    bytes.truncate(v15_len);

    let v15_path = std::env::temp_dir().join(format!(
        "goat_save_v15_truncated_{}.gsav",
        std::process::id()
    ));
    std::fs::write(&v15_path, &bytes).unwrap();
    let loaded = load_from_file(&v15_path).unwrap();
    std::fs::remove_file(&v15_path).ok();

    assert_eq!(loaded.pc_season_decisive_moments, 0);

    let restored = to_world_state(&loaded, &test_world());
    assert_eq!(restored.pc_season_decisive_moments, 0);
}

// ── Clutch index (BL5.3, v17) ────────────────────────────────────────────────

#[test]
fn save_load_restores_clutch_index_through_bytes() {
    // v17+: both clutch counters must survive a full byte round-trip, same idiom
    // as the assists/decisive counters.
    let mut state = setup_state();
    state.pc_season_clutch_index = 6;
    state.pc_career_clutch_index = 24;
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let path = std::env::temp_dir().join(format!(
        "goat_save_clutch_roundtrip_{}.gsav",
        std::process::id()
    ));
    save_to_file(&data, &path).unwrap();
    let restored = to_world_state(&load_from_file(&path).unwrap(), &test_world());
    std::fs::remove_file(&path).ok();

    assert_eq!(restored.pc_season_clutch_index, 6);
    assert_eq!(restored.pc_career_clutch_index, 24);
}

#[test]
fn old_v16_save_without_clutch_index_defaults_to_zero() {
    // A real v16 binary never wrote the two trailing v17 u32s (8 bytes). Simulate
    // that by truncating them off a real v17 buffer — exercises the actual
    // `.unwrap_or(0)` backward-compat reads (same idiom as the tests above).
    let mut state = setup_state();
    state.pc_season_clutch_index = 3;
    state.pc_career_clutch_index = 17;
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let full_path =
        std::env::temp_dir().join(format!("goat_save_v17_full_{}.gsav", std::process::id()));
    save_to_file(&data, &full_path).unwrap();
    let mut bytes = std::fs::read(&full_path).unwrap();
    std::fs::remove_file(&full_path).ok();

    // Trailing encoding: pc_season_clutch_index + pc_career_clutch_index (2 × 4-byte).
    let v16_len = bytes.len() - 2 * 4 - 4 - 4;
    bytes.truncate(v16_len);

    let v16_path = std::env::temp_dir().join(format!(
        "goat_save_v16_truncated_{}.gsav",
        std::process::id()
    ));
    std::fs::write(&v16_path, &bytes).unwrap();
    let loaded = load_from_file(&v16_path).unwrap();
    std::fs::remove_file(&v16_path).ok();

    assert_eq!(loaded.pc_season_clutch_index, 0);
    assert_eq!(loaded.pc_career_clutch_index, 0);

    let restored = to_world_state(&loaded, &test_world());
    assert_eq!(restored.pc_season_clutch_index, 0);
    assert_eq!(restored.pc_career_clutch_index, 0);
}

// ── Live promotion/relegation membership (A3.3, v18) ─────────────────────────

#[test]
fn save_load_restores_nation_membership_through_bytes() {
    // v18+: the promotion-advanced membership must survive a full byte round-trip
    // — it's path-dependent (driven by real played results), so it must persist
    // rather than being regenerated from world_seed.
    let mut state = setup_state();
    state.pc_nation_membership = (0..60).map(|i| 1000 + i * 7).collect();
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let path = std::env::temp_dir().join(format!(
        "goat_save_membership_roundtrip_{}.gsav",
        std::process::id()
    ));
    save_to_file(&data, &path).unwrap();
    let restored = to_world_state(&load_from_file(&path).unwrap(), &test_world());
    std::fs::remove_file(&path).ok();

    assert_eq!(
        restored.pc_nation_membership,
        (0..60).map(|i| 1000 + i * 7).collect::<Vec<u32>>()
    );
}

#[test]
fn old_v17_save_without_membership_defaults_to_genesis_static() {
    // A real v17 binary never wrote the trailing v18 length-prefixed list (its
    // empty-vec form is just a 4-byte count). Simulate by truncating those 4 bytes
    // off a real v18 buffer — the load must default to EMPTY, which every reader
    // (`effective_league_clubs`/`overlay_nation_membership`) treats as
    // genesis-static membership.
    let mut state = setup_state();
    state.pc_nation_membership = (0..60).collect();
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let full_path =
        std::env::temp_dir().join(format!("goat_save_v18_full_{}.gsav", std::process::id()));
    save_to_file(&data, &full_path).unwrap();
    let mut bytes = std::fs::read(&full_path).unwrap();
    std::fs::remove_file(&full_path).ok();

    // Trailing encoding: 4-byte count + 60 entries × 4 bytes.
    let v17_len = bytes.len() - 4 - 60 * 4 - 4;
    bytes.truncate(v17_len);

    let v17_path = std::env::temp_dir().join(format!(
        "goat_save_v17_truncated_{}.gsav",
        std::process::id()
    ));
    std::fs::write(&v17_path, &bytes).unwrap();
    let loaded = load_from_file(&v17_path).unwrap();
    std::fs::remove_file(&v17_path).ok();

    assert!(
        loaded.pc_nation_membership.is_empty(),
        "a pre-v18 save must default the membership to empty (= genesis-static)"
    );
    let restored = to_world_state(&loaded, &test_world());
    assert!(restored.pc_nation_membership.is_empty());
}

// ── Career base year (v19) ───────────────────────────────────────────────────

#[test]
fn save_load_restores_career_base_year_through_bytes() {
    // v19+: the wall-clock year captured at new-game must round-trip so a save
    // shows identical dates on every load.
    let mut state = setup_state();
    state.career_base_year = 2031;
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let path = std::env::temp_dir().join(format!(
        "goat_save_base_year_roundtrip_{}.gsav",
        std::process::id()
    ));
    save_to_file(&data, &path).unwrap();
    let restored = to_world_state(&load_from_file(&path).unwrap(), &test_world());
    std::fs::remove_file(&path).ok();

    assert_eq!(restored.career_base_year, 2031);
}

#[test]
fn old_v18_save_without_career_base_year_defaults_to_2025() {
    // A pre-v19 binary never wrote the trailing year u32 — truncate it off a
    // real v19 buffer and confirm the default keeps old saves' dates identical
    // (2025 = the hardcoded BASE_CAREER_YEAR they were created with).
    let mut state = setup_state();
    state.career_base_year = 2031;
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let full_path =
        std::env::temp_dir().join(format!("goat_save_v19_full_{}.gsav", std::process::id()));
    save_to_file(&data, &full_path).unwrap();
    let mut bytes = std::fs::read(&full_path).unwrap();
    std::fs::remove_file(&full_path).ok();

    // Trailing encoding: career_base_year (1 × 4-byte u32).
    let v18_len = bytes.len() - 4;
    bytes.truncate(v18_len);

    let v18_path = std::env::temp_dir().join(format!(
        "goat_save_v18_truncated_{}.gsav",
        std::process::id()
    ));
    std::fs::write(&v18_path, &bytes).unwrap();
    let loaded = load_from_file(&v18_path).unwrap();
    std::fs::remove_file(&v18_path).ok();

    assert_eq!(loaded.career_base_year, 2025);
    let restored = to_world_state(&loaded, &test_world());
    assert_eq!(restored.career_base_year, 2025);
}
