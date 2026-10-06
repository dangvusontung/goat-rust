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
use goat_save::save::{
    from_bytes_layout_only, from_world_state, load_from_file, save_to_file, to_world_state,
};
use goat_world::world::WorldGenesis;

const TEST_WORLD_SEED: u64 = 54321;

/// Byte length of the v21 (merge) block — the local PA2 line's appended fields:
/// academy arc (1 + 4 + 4) + personal staff (5 × (1 + 8)) + manager trust/favor
/// (4 + 4) + orbit-records empty-list count (4) + injury-return week (4). The
/// truncation tests below simulate pre-v21 saves, so each must strip this block
/// in addition to the fields the pre-merge formulas already stripped.
const V21_BLOCK_LEN: usize = 9 + 45 + 8 + 4 + 4;

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
            staff_mods: goat_world::staff::club_staff_mods(world.clubs[pc_club_id].strength),
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
    let mut data = from_world_state(&state, &view);
    // Historical layout fixture predates the v22 history extension.
    data.pc_development_history = Default::default();
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
    let mut data = from_world_state(&state, &view);
    // Historical layout fixture predates the v22 history extension.
    data.pc_development_history = Default::default();

    let full_path =
        std::env::temp_dir().join(format!("goat_save_v9_full_{}.gsav", std::process::id()));
    save_to_file(&data, &full_path).unwrap();
    let mut bytes = std::fs::read(&full_path).unwrap();
    std::fs::remove_file(&full_path).ok();

    // Truncate the 5 trailing v9 fields (pc_career_standout_matches,
    // pc_season_standout_matches, pc_career_best_ovr, pc_career_transfer_requests,
    // pc_season_transfer_requests — all 4-byte fields) to simulate a pre-v9 save.
    let v8_len = bytes.len() - 5 * 4 - 4 - V21_BLOCK_LEN; // +4: drop the v20 sim_version trailer too
    bytes.truncate(v8_len);

    // Pre-v20 layouts have no sim_version trailer, so the guarded `from_bytes`
    // would refuse them — these tests exercise layout migration specifically.
    let loaded = from_bytes_layout_only(&bytes).unwrap();

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

    // Pre-v20 layouts have no sim_version trailer, so the guarded `from_bytes`
    // would refuse them — these tests exercise layout migration specifically.
    let loaded = from_bytes_layout_only(&v10_bytes).unwrap();

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
    let mut data = from_world_state(&state, &view);
    // Historical layout fixture predates the v22 history extension.
    data.pc_development_history = Default::default();

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
    // Pre-v20 layouts have no sim_version trailer, so the guarded `from_bytes`
    // would refuse them — these tests exercise layout migration specifically.
    let loaded = from_bytes_layout_only(&bytes).unwrap();

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
    // Historical layout fixture predates the v22 history extension.
    data.pc_development_history = Default::default();
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
    let v11_len = bytes.len()
        - (4 + 3 * 8)
        - 4
        - (4 + 4 + 4 + 4)
        - 2 * 4
        - 4
        - 2 * 4
        - 4
        - 4
        - 4
        - V21_BLOCK_LEN; // last -4: v20 sim_version trailer
    bytes.truncate(v11_len);

    // Pre-v20 layouts have no sim_version trailer, so the guarded `from_bytes`
    // would refuse them — these tests exercise layout migration specifically.
    let loaded = from_bytes_layout_only(&bytes).unwrap();

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
    // Historical layout fixture predates the v22 history extension.
    data.pc_development_history = Default::default();
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
    let v12_len =
        bytes.len() - (4 + 3) - (4 + 4 + 4 + 4) - 2 * 4 - 4 - 2 * 4 - 4 - 4 - 4 - V21_BLOCK_LEN; // last -4: v20 sim_version trailer
    bytes.truncate(v12_len);

    // Pre-v20 layouts have no sim_version trailer, so the guarded `from_bytes`
    // would refuse them — these tests exercise layout migration specifically.
    let loaded = from_bytes_layout_only(&bytes).unwrap();

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
    let mut data = from_world_state(&state, &view);
    // Historical layout fixture predates the v22 history extension.
    data.pc_development_history = Default::default();

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
    let v13_len = bytes.len()
        - (4 + manager_blob_len)
        - (4 + 4)
        - 4
        - 2 * 4
        - 4
        - 2 * 4
        - 4
        - 4
        - 4
        - V21_BLOCK_LEN; // last -4: v20 sim_version trailer
    bytes.truncate(v13_len);

    // Pre-v20 layouts have no sim_version trailer, so the guarded `from_bytes`
    // would refuse them — these tests exercise layout migration specifically.
    let loaded = from_bytes_layout_only(&bytes).unwrap();

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
    let mut data = from_world_state(&state, &view);
    // Historical layout fixture predates the v22 history extension.
    data.pc_development_history = Default::default();

    let full_path =
        std::env::temp_dir().join(format!("goat_save_v15_full_{}.gsav", std::process::id()));
    save_to_file(&data, &full_path).unwrap();
    let mut bytes = std::fs::read(&full_path).unwrap();
    std::fs::remove_file(&full_path).ok();

    // Trailing encoding: pc_season_assists + pc_career_assists (2 × 4-byte u32),
    // then v16's pc_season_decisive_moments (4-byte u32).
    let v14_len = bytes.len() - 2 * 4 - 4 - 2 * 4 - 4 - 4 - 4 - V21_BLOCK_LEN; // last -4: v20 sim_version trailer
    bytes.truncate(v14_len);

    // Pre-v20 layouts have no sim_version trailer, so the guarded `from_bytes`
    // would refuse them — these tests exercise layout migration specifically.
    let loaded = from_bytes_layout_only(&bytes).unwrap();

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
    let mut data = from_world_state(&state, &view);
    // Historical layout fixture predates the v22 history extension.
    data.pc_development_history = Default::default();

    let full_path =
        std::env::temp_dir().join(format!("goat_save_v16_full_{}.gsav", std::process::id()));
    save_to_file(&data, &full_path).unwrap();
    let mut bytes = std::fs::read(&full_path).unwrap();
    std::fs::remove_file(&full_path).ok();

    // Trailing encoding: pc_season_decisive_moments (1 × 4-byte u32), then v17's
    // two clutch-index u32s (BL5.3).
    let v15_len = bytes.len() - 4 - 2 * 4 - 4 - 4 - 4 - V21_BLOCK_LEN; // last -4: v20 sim_version trailer
    bytes.truncate(v15_len);

    // Pre-v20 layouts have no sim_version trailer, so the guarded `from_bytes`
    // would refuse them — these tests exercise layout migration specifically.
    let loaded = from_bytes_layout_only(&bytes).unwrap();

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
    let mut data = from_world_state(&state, &view);
    // Historical layout fixture predates the v22 history extension.
    data.pc_development_history = Default::default();

    let full_path =
        std::env::temp_dir().join(format!("goat_save_v17_full_{}.gsav", std::process::id()));
    save_to_file(&data, &full_path).unwrap();
    let mut bytes = std::fs::read(&full_path).unwrap();
    std::fs::remove_file(&full_path).ok();

    // Trailing encoding: pc_season_clutch_index + pc_career_clutch_index (2 × 4-byte).
    let v16_len = bytes.len() - 2 * 4 - 4 - 4 - 4 - V21_BLOCK_LEN; // last -4: v20 sim_version trailer
    bytes.truncate(v16_len);

    // Pre-v20 layouts have no sim_version trailer, so the guarded `from_bytes`
    // would refuse them — these tests exercise layout migration specifically.
    let loaded = from_bytes_layout_only(&bytes).unwrap();

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
    let mut data = from_world_state(&state, &view);
    // Historical layout fixture predates the v22 history extension.
    data.pc_development_history = Default::default();

    let full_path =
        std::env::temp_dir().join(format!("goat_save_v18_full_{}.gsav", std::process::id()));
    save_to_file(&data, &full_path).unwrap();
    let mut bytes = std::fs::read(&full_path).unwrap();
    std::fs::remove_file(&full_path).ok();

    // Trailing encoding: 4-byte count + 60 entries × 4 bytes.
    let v17_len = bytes.len() - 4 - 60 * 4 - 4 - 4 - V21_BLOCK_LEN; // last -4: v20 sim_version trailer
    bytes.truncate(v17_len);

    // Pre-v20 layouts have no sim_version trailer, so the guarded `from_bytes`
    // would refuse them — these tests exercise layout migration specifically.
    let loaded = from_bytes_layout_only(&bytes).unwrap();

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
    let mut data = from_world_state(&state, &view);
    // Historical layout fixture predates the v22 history extension.
    data.pc_development_history = Default::default();

    let full_path =
        std::env::temp_dir().join(format!("goat_save_v19_full_{}.gsav", std::process::id()));
    save_to_file(&data, &full_path).unwrap();
    let mut bytes = std::fs::read(&full_path).unwrap();
    std::fs::remove_file(&full_path).ok();

    // Trailing encoding: career_base_year (1 × 4-byte u32).
    let v18_len = bytes.len() - 4 - 4 - V21_BLOCK_LEN; // second -4: v20 sim_version trailer
    bytes.truncate(v18_len);

    // Pre-v20 layouts have no sim_version trailer, so the guarded `from_bytes`
    // would refuse them — these tests exercise layout migration specifically.
    let loaded = from_bytes_layout_only(&bytes).unwrap();

    assert_eq!(loaded.career_base_year, 2025);
    let restored = to_world_state(&loaded, &test_world());
    assert_eq!(restored.career_base_year, 2025);
}

// ── sim_version guard (v20+) ─────────────────────────────────────────────────

#[test]
fn pre_v20_save_is_refused_by_guarded_load_but_readable_layout_only() {
    use goat_save::save::{from_bytes, SaveError, SIM_VERSION};

    let state = setup_state();
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    // Full current save, then strip the trailing sim_version u32 → pre-v20 stream.
    let path = std::env::temp_dir().join(format!("goat_save_v20_full_{}.gsav", std::process::id()));
    save_to_file(&data, &path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).ok();
    let legacy = &bytes[..bytes.len() - 4];

    // The guarded entry point refuses it: sim semantics unknown (decodes as 0).
    assert!(
        matches!(
            from_bytes(legacy),
            Err(SaveError::SimVersionMismatch {
                found: 0,
                expected
            }) if expected == SIM_VERSION
        ),
        "a stream without the v20 sim_version trailer must be refused"
    );

    // The layout-only escape hatch still parses it (layout stays tail-append
    // compatible; only the sim-semantics guarantee is withheld).
    let loaded = from_bytes_layout_only(legacy).unwrap();
    assert_eq!(loaded.world_seed, data.world_seed);

    // A header retagged to v19 is likewise refused: pre-v20 layouts carry no
    // sim_version at all, so the guarded path must not trust the body bytes.
    let mut retagged = bytes.clone();
    retagged[4..8].copy_from_slice(&19u32.to_le_bytes());
    assert!(
        matches!(
            from_bytes(&retagged),
            Err(SaveError::SimVersionMismatch { .. })
        ),
        "a pre-v20 layout tag must be refused regardless of body content"
    );
}

#[test]
fn current_save_roundtrips_through_guarded_from_bytes() {
    use goat_save::save::from_bytes;

    let state = setup_state();
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let path =
        std::env::temp_dir().join(format!("goat_save_v20_guarded_{}.gsav", std::process::id()));
    save_to_file(&data, &path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).ok();

    let loaded = from_bytes(&bytes).expect("current saves carry the current SIM_VERSION");
    assert_eq!(loaded.world_seed, data.world_seed);
    assert_eq!(loaded.career_base_year, data.career_base_year);
}

#[test]
fn save_load_restores_manager_relation_v11() {
    // Full byte path for the v21 manager trust/favor fields (PA2 M1.5).
    let mut state = setup_state();
    state.pc_manager_trust = 73;
    state.pc_manager_favor = 21;
    let pc_id = state.pc_player_id.unwrap();
    let view = state.players.snapshot(pc_id);
    let data = from_world_state(&state, &view);

    let path = std::env::temp_dir().join("goat_save_manager_v11_roundtrip.gsav");
    save_to_file(&data, &path).unwrap();
    let restored = to_world_state(&load_from_file(&path).unwrap(), &test_world());
    std::fs::remove_file(&path).ok();

    assert_eq!(
        restored.pc_manager_trust, 73,
        "manager trust survives round-trip"
    );
    assert_eq!(
        restored.pc_manager_favor, 21,
        "manager favor survives round-trip"
    );
}

#[test]
fn development_history_roundtrips_and_continues_identically() {
    use goat_save::save::{from_bytes, to_bytes};
    let mut state = setup_state();
    state = reduce(
        state,
        Intent::ApplyMatchResult {
            familiarity_xp: [Fixed::ZERO; NUM_ROLES],
            energy_cost: Fixed::from_int(15),
            injury_weeks: Some(3),
        },
        &mut GoatRng::new(71),
    );
    let view = state.players.snapshot(state.pc_player_id.unwrap());
    let bytes = to_bytes(&from_world_state(&state, &view));
    let data = from_bytes(&bytes).unwrap();
    let restored = to_world_state(&data, &test_world());
    assert!(!state.pc_development_history.weeks.is_empty());
    assert_eq!(
        restored.pc_development_history,
        state.pc_development_history
    );
    assert_eq!(restored.pc_development_history.matches.len(), 1);
    let a = reduce(
        state,
        Intent::StartSeason { fixtures: vec![] },
        &mut GoatRng::new(93),
    );
    let b = reduce(
        restored,
        Intent::StartSeason { fixtures: vec![] },
        &mut GoatRng::new(93),
    );
    assert_eq!(a.pc_development_history, b.pc_development_history);
    assert_eq!(a.players.snapshot(0).current, b.players.snapshot(0).current);
}

#[test]
fn malformed_history_counts_and_truncation_are_rejected() {
    use goat_save::save::{from_bytes, to_bytes};
    let state = setup_state();
    let view = state.players.snapshot(state.pc_player_id.unwrap());
    let bytes = to_bytes(&from_world_state(&state, &view));
    let marker = 0x4853_5459u32.to_le_bytes();
    let offset = bytes.windows(4).rposition(|w| w == marker).unwrap();
    let mut corrupt = bytes.clone();
    corrupt[offset + 4..offset + 8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(from_bytes(&corrupt).is_err());
    for end in offset + 4..bytes.len() {
        assert!(from_bytes(&bytes[..end]).is_err(), "end={end}");
    }
}

#[test]
fn history_storage_stays_small_for_twenty_years() {
    use goat_core::history::{DevelopmentHistory, MatchWorkload, TrainingWeek};
    use goat_save::save::to_bytes;
    let state = setup_state();
    let view = state.players.snapshot(state.pc_player_id.unwrap());
    let mut data = from_world_state(&state, &view);
    data.pc_development_history = DevelopmentHistory::default();
    let baseline = to_bytes(&data).len();
    for week in 0..20 * 52 {
        data.pc_development_history.weeks.push(TrainingWeek {
            epoch_day: week * 7,
            age_weeks: 16 * 52 + week,
            focus_mask: 1,
            requested_intensity: 1,
            effective_intensity: 1,
            facilities: Fixed::ONE,
            energy_before: Fixed::from_int(75),
            energy_after: Fixed::from_int(75),
            injury_before: 0,
            injury_after: 0,
            total_attribute_delta: Fixed::ZERO,
        });
        data.pc_development_history.matches.push(MatchWorkload {
            epoch_day: week * 7,
            energy_before: Fixed::from_int(90),
            energy_after: Fixed::from_int(75),
            energy_cost: Fixed::from_int(15),
            injury_before: 0,
            injury_after: 0,
        });
    }
    assert_eq!(to_bytes(&data).len() - baseline, 16 + 1040 * (38 + 24));
}

#[test]
fn dated_npc_minutes_survive_bytes_and_reject_bad_counts() {
    use goat_core::history::NpcMatchLoad;
    use goat_save::save::{from_bytes, to_bytes};
    let mut state = setup_state();
    let load = NpcMatchLoad {
        competition_id: 1,
        pop_idx: 7,
        fixture_id: 99,
        epoch_day: 54,
        minutes: 23,
    };
    state = reduce(
        state,
        Intent::RecordNpcMatchLoads {
            loads: vec![load, load],
        },
        &mut GoatRng::new(1),
    );
    assert_eq!(state.npc_match_loads, vec![load]);
    let view = state.players.snapshot(state.pc_player_id.unwrap());
    let bytes = to_bytes(&from_world_state(&state, &view));
    let data = from_bytes(&bytes).unwrap();
    assert_eq!(
        to_world_state(&data, &test_world()).npc_match_loads,
        vec![load]
    );
    let mut retained = goat_world::session::SimulationSession::new();
    let before = retained
        .population(
            state.world_seed,
            state.season_number,
            &state.orbit_records,
            &state.npc_match_loads,
        )
        .training_history(7, 0, 12);
    let mut resumed = goat_world::session::SimulationSession::new();
    let after = resumed
        .population(
            data.world_seed,
            data.season_number,
            &data.orbit_records,
            &data.npc_match_loads,
        )
        .training_history(7, 0, 12);
    assert_eq!(before, after);
    let offset = bytes
        .windows(4)
        .rposition(|w| w == 0x4E4C_4F44u32.to_le_bytes())
        .unwrap();
    let mut bad = bytes.clone();
    bad[offset + 4..offset + 8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(from_bytes(&bad).is_err());
    for end in offset + 4..bytes.len() {
        assert!(from_bytes(&bytes[..end]).is_err());
    }
}

#[test]
fn dated_calendar_partial_week_and_rescheduled_fixture_survive_save() {
    use goat_save::save::{from_bytes, to_bytes};
    let mut state = setup_state();
    state.dated_calendar = true;
    state.career_base_year = 2023;
    let c = goat_core::chronology::Chronology::new(2023);
    let target = c.frame(1).next_preparation_start + 2;
    state = reduce(
        state,
        Intent::AdvanceToDate {
            epoch_day: target,
            train: false,
        },
        &mut GoatRng::new(0),
    );
    state = reduce(
        state,
        Intent::StartSeason { fixtures: vec![] },
        &mut GoatRng::new(0),
    );
    let temp = from_world_state(&state, &state.pc_display_view());
    state = to_world_state(&temp, &test_world());
    assert!(!state.pc_season_fixtures.is_empty());
    state.pc_season_fixtures[0].scheduled_day += 3;
    state
        .pc_played_fixture_ids
        .push(state.pc_season_fixtures[3].id);
    state.season_round = 1;
    let data = from_bytes(&to_bytes(&from_world_state(
        &state,
        &state.pc_display_view(),
    )))
    .unwrap();
    let mut restored = to_world_state(&data, &test_world());
    assert!(restored.dated_calendar);
    assert_eq!(restored.pc_played_fixture_ids, state.pc_played_fixture_ids);
    assert_eq!(restored.pc_epoch_day, target);
    assert_eq!(
        restored.pc_season_fixtures[0].scheduled_day,
        state.pc_season_fixtures[0].scheduled_day
    );
    assert_eq!(
        restored
            .players
            .get_age_weeks(restored.pc_player_id.unwrap()),
        state.players.get_age_weeks(state.pc_player_id.unwrap())
    );
    let next = target + 7;
    let original = reduce(
        state,
        Intent::AdvanceToDate {
            epoch_day: next,
            train: true,
        },
        &mut GoatRng::new(13),
    );
    restored = reduce(
        restored,
        Intent::AdvanceToDate {
            epoch_day: next,
            train: true,
        },
        &mut GoatRng::new(13),
    );
    assert_eq!(
        restored.pc_development_history,
        original.pc_development_history
    );
    assert_eq!(
        restored.pc_display_view().current,
        original.pc_display_view().current
    );
}

#[test]
fn deep_scope_scores_and_sorted_journals_resume_without_rerolling() {
    use goat_save::save::{from_bytes, to_bytes};
    use goat_world::session::SimulationSession;
    let world = test_world();
    let mut s = setup_state();
    s.dated_calendar = true;
    s.career_base_year = 2023;
    let c = s.chronology().unwrap();
    let mut warm = SimulationSession::new();
    s = warm.advance_deep(s, &world);
    s = reduce(
        s,
        Intent::AdvanceToDate {
            epoch_day: goat_world::calendar::dated_fixture_day(c, 1, 0),
            train: false,
        },
        &mut GoatRng::new(0),
    );
    s = warm.advance_deep(s, &world);
    let view = s.players.snapshot(s.pc_player_id.unwrap());
    let data = from_world_state(&s, &view);
    let bytes = to_bytes(&data);
    let parsed = from_bytes(&bytes).unwrap();
    assert_eq!(parsed.deep_scopes, s.deep_scopes);
    assert_eq!(parsed.deep_results, s.deep_results);
    let mut resumed = to_world_state(&parsed, &world);
    let mut cold = SimulationSession::new();
    resumed = cold.advance_deep(resumed, &world);
    assert_eq!(resumed.deep_results, s.deep_results);
    let next = goat_world::calendar::dated_fixture_day(c, 1, 1);
    s = reduce(
        s,
        Intent::AdvanceToDate {
            epoch_day: next,
            train: false,
        },
        &mut GoatRng::new(1),
    );
    resumed = reduce(
        resumed,
        Intent::AdvanceToDate {
            epoch_day: next,
            train: false,
        },
        &mut GoatRng::new(1),
    );
    s = warm.advance_deep(s, &world);
    resumed = cold.advance_deep(resumed, &world);
    assert_eq!(resumed.deep_results, s.deep_results);
    assert_eq!(resumed.deep_scopes, s.deep_scopes);
    assert_eq!(resumed.orbit_records, s.orbit_records);
    assert_eq!(resumed.npc_match_loads, s.npc_match_loads);
    assert_eq!(
        warm.population_deep(&s).career_fingerprint(),
        cold.population_deep(&resumed).career_fingerprint()
    );
    let idx = s.npc_match_loads[0].pop_idx as usize;
    let week = next / 7 + 1;
    assert_eq!(
        warm.population_deep(&s).current_ovr(idx, week),
        cold.population_deep(&resumed).current_ovr(idx, week)
    );
    assert_eq!(
        warm.population_deep(&s).energy_at(idx, week),
        cold.population_deep(&resumed).energy_at(idx, week)
    );
    assert_eq!(warm.rebuild_count(), 1);
    // Duplicate fixture identities must be rejected by the save parser.
    let mut invalid = parsed.clone();
    invalid.deep_results.push(invalid.deep_results[0]);
    assert!(from_bytes(&to_bytes(&invalid)).is_err());
}

#[test]
fn compact_journal_preserves_multi_season_health_and_future_replay() {
    use goat_core::history::NpcMatchLoad;
    use goat_save::save::{from_bytes, to_bytes};
    let mut state = setup_state();
    state.dated_calendar = true;
    state.career_base_year = 2023;
    state.season_number = 3;
    state.pc_epoch_day = 735;
    let id = state.pc_player_id.unwrap();
    state.players.set_injury_weeks(id, 4);
    state.pc_injury_return_week = Some(102);
    // Match the save fixture to its deterministic age clock.
    state
        .players
        .set_age_weeks(id, goat_core::tuning::START_AGE_WEEKS + 105);
    for fixture in 0..120u32 {
        for player in 0..50u32 {
            state.npc_match_loads.push(NpcMatchLoad {
                competition_id: if fixture % 5 == 0 { 6 } else { 1 },
                fixture_id: fixture as u64 + 100,
                epoch_day: fixture * 6,
                pop_idx: player,
                minutes: if player % 3 == 0 { 0 } else { 90 },
            });
        }
    }
    state
        .orbit_records
        .push(goat_core::state::OrbitMatchRecord {
            season: 1,
            round: 0,
            div: 1,
            credits: (1..50)
                .filter(|idx| idx % 3 != 0)
                .map(|pop_idx| goat_core::state::NpcMatchCredit {
                    pop_idx,
                    goals: u8::from(pop_idx == 7),
                    assists: u8::from(pop_idx == 8),
                    result: 1,
                })
                .collect(),
        });
    let view = state.players.snapshot(id);
    let mut baseline = from_world_state(&state, &view);
    baseline.npc_match_loads.clear();
    let base_size = to_bytes(&baseline).len();
    let bytes = to_bytes(&from_world_state(&state, &view));
    assert!(bytes.len() - base_size < state.npc_match_loads.len() * 8);
    let loaded = from_bytes(&bytes).unwrap();
    let mut resumed = to_world_state(&loaded, &test_world());
    assert_eq!(resumed.npc_match_loads, state.npc_match_loads);
    assert_eq!(resumed.orbit_records, state.orbit_records);
    assert_eq!(resumed.pc_medical_status(), state.pc_medical_status());
    let mut live = goat_world::session::SimulationSession::new();
    let mut cold = goat_world::session::SimulationSession::new();
    assert_eq!(
        live.population_deep(&state).career_fingerprint(),
        cold.population_deep(&resumed).career_fingerprint()
    );
    for idx in [0, 7, 49] {
        assert_eq!(
            live.population_deep(&state).training_history(idx, 0, 105),
            cold.population_deep(&resumed).training_history(idx, 0, 105)
        );
        assert_eq!(
            live.population_deep(&state).medical_status(idx, 105),
            cold.population_deep(&resumed).medical_status(idx, 105)
        );
    }
    let end = state.pc_epoch_day + 35;
    let mut rng_a = GoatRng::new(77);
    let mut rng_b = GoatRng::new(77);
    state = reduce(
        state,
        Intent::AdvanceToDate {
            epoch_day: end,
            train: false,
        },
        &mut rng_a,
    );
    resumed = reduce(
        resumed,
        Intent::AdvanceToDate {
            epoch_day: end,
            train: false,
        },
        &mut rng_b,
    );
    assert_eq!(state.pc_medical_status(), resumed.pc_medical_status());
    assert_eq!(state.pc_development_history, resumed.pc_development_history);
    assert_eq!(state.players.get_injury_weeks(id), 0);
}

#[test]
fn v25_workload_layout_can_be_read_without_silently_accepting_old_simulation() {
    use goat_save::save::{from_bytes, to_bytes};
    let state = setup_state();
    let mut data = from_world_state(&state, &state.players.snapshot(state.pc_player_id.unwrap()));
    let load = goat_core::history::NpcMatchLoad {
        competition_id: 1,
        fixture_id: 99,
        epoch_day: 20,
        pop_idx: 7,
        minutes: 0,
    };
    data.npc_match_loads.push(load);
    let mut bytes = to_bytes(&data);
    let start = bytes
        .windows(4)
        .rposition(|w| w == 0x4E4C_4F44u32.to_le_bytes())
        .unwrap();
    bytes.truncate(start + 4);
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&load.competition_id.to_le_bytes());
    bytes.extend_from_slice(&load.pop_idx.to_le_bytes());
    bytes.extend_from_slice(&load.fixture_id.to_le_bytes());
    bytes.extend_from_slice(&load.epoch_day.to_le_bytes());
    bytes.extend_from_slice(&(load.minutes as u32).to_le_bytes());
    bytes.extend_from_slice(&11u32.to_le_bytes());
    bytes[4..8].copy_from_slice(&25u32.to_le_bytes());
    assert_eq!(
        from_bytes_layout_only(&bytes).unwrap().npc_match_loads,
        vec![load]
    );
    assert!(matches!(
        from_bytes(&bytes),
        Err(goat_save::SaveError::SimVersionMismatch { .. })
    ));
}

#[test]
fn reactive_npc_cards_save_and_next_fixture_are_exact_after_loading() {
    use goat_save::save::{from_bytes, to_bytes};
    let world = test_world();
    let mut state = setup_state();
    state.dated_calendar = true;
    state.realistic_npc = true;
    state.career_base_year = 2023;
    let calendar = state.chronology().unwrap();
    state.pc_epoch_day = goat_world::calendar::dated_fixture_day(calendar, 1, 0);
    let mut warm = goat_world::session::SimulationSession::new();
    state = warm.advance_deep(state, &world);
    assert!(!state.npc_cards.is_empty());
    let bytes = to_bytes(&from_world_state(
        &state,
        &state.players.snapshot(state.pc_player_id.unwrap()),
    ));
    let mut loaded = to_world_state(&from_bytes(&bytes).unwrap(), &world);
    assert!(loaded.realistic_npc);
    assert_eq!(loaded.npc_cards, state.npc_cards);
    let mut cold = goat_world::session::SimulationSession::new();
    state.pc_epoch_day = goat_world::calendar::dated_fixture_day(calendar, 1, 1);
    loaded.pc_epoch_day = state.pc_epoch_day;
    state = warm.advance_deep(state, &world);
    loaded = cold.advance_deep(loaded, &world);
    assert_eq!(state.deep_results, loaded.deep_results);
    assert_eq!(state.npc_cards, loaded.npc_cards);
    assert_eq!(state.npc_match_loads, loaded.npc_match_loads);
    assert_eq!(state.orbit_records, loaded.orbit_records);
    assert_eq!(
        warm.population_deep(&state).career_fingerprint(),
        cold.population_deep(&loaded).career_fingerprint()
    );
    let offset = bytes
        .windows(4)
        .rposition(|w| w == 0x4E50_4344u32.to_le_bytes())
        .unwrap();
    let mut bad = bytes.clone();
    bad[offset + 5..offset + 9].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(from_bytes(&bad).is_err());
    let mut bad = bytes.clone();
    bad[offset + 4] = 2;
    assert!(from_bytes(&bad).is_err());
    let mut bad = bytes.clone();
    bad[offset + 9 + 25] = 3;
    assert!(from_bytes(&bad).is_err());
    let mut duplicate = bytes.clone();
    let first = duplicate[offset + 9..offset + 35].to_vec();
    duplicate[offset + 35..offset + 61].copy_from_slice(&first);
    assert!(from_bytes(&duplicate).is_err());
    let mut reordered = bytes.clone();
    let second = reordered[offset + 35..offset + 61].to_vec();
    reordered[offset + 9..offset + 35].copy_from_slice(&second);
    reordered[offset + 35..offset + 61].copy_from_slice(&first);
    assert!(from_bytes(&reordered).is_err());
    for end in offset + 4..bytes.len() {
        assert!(from_bytes(&bytes[..end]).is_err());
    }
}

#[test]
fn local_resume_checkpoint_roundtrips_and_replays_next_fixture() {
    use goat_save::save::{from_bytes, from_world_state_with_session, session_from_save, to_bytes};
    let world = test_world();
    let mut state = setup_state();
    state.dated_calendar = true;
    state.realistic_npc = true;
    state.career_base_year = 2023;
    state.pc_epoch_day = goat_world::calendar::dated_fixture_day(state.chronology().unwrap(), 1, 0);
    let mut warm = goat_world::session::SimulationSession::new();
    state = warm.advance_deep(state, &world);
    let data = from_world_state_with_session(
        &state,
        &state.players.snapshot(state.pc_player_id.unwrap()),
        &mut warm,
    );
    assert!(!data.resume_checkpoint.is_empty());
    let bytes = to_bytes(&data);
    let decoded = from_bytes(&bytes).unwrap();
    assert_eq!(data.resume_checkpoint, decoded.resume_checkpoint);
    let mut loaded = to_world_state(&decoded, &world);
    let mut resumed = session_from_save(&decoded, &loaded);
    assert_eq!(resumed.rebuild_count(), 0);
    assert_eq!(
        warm.population_deep(&state).fingerprint(),
        resumed.population_deep(&loaded).fingerprint()
    );
    assert_eq!(resumed.rebuild_count(), 0);
    state.pc_epoch_day = goat_world::calendar::dated_fixture_day(state.chronology().unwrap(), 1, 1);
    loaded.pc_epoch_day = state.pc_epoch_day;
    state = warm.advance_deep(state, &world);
    loaded = resumed.advance_deep(loaded, &world);
    assert_eq!(state.deep_results, loaded.deep_results);
    assert_eq!(state.npc_match_loads, loaded.npc_match_loads);
    assert_eq!(state.npc_cards, loaded.npc_cards);
    assert_eq!(state.orbit_records, loaded.orbit_records);
    let mut corrupted = decoded.clone();
    let end = corrupted.resume_checkpoint.len() - 1;
    corrupted.resume_checkpoint[end] ^= 1;
    let valid_state = to_world_state(&decoded, &world);
    let mut fallback = session_from_save(&corrupted, &valid_state);
    assert_eq!(
        fallback.population_deep(&valid_state).career_fingerprint(),
        warm.population_deep(&valid_state).career_fingerprint()
    );
    assert_eq!(fallback.rebuild_count(), 1);
    let offset = bytes
        .windows(4)
        .rposition(|w| w == 0x4350_4B54u32.to_le_bytes())
        .unwrap();
    let mut bad = bytes.clone();
    bad[offset + 4..offset + 8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(from_bytes(&bad).is_err());
    for end in [offset + 4, offset + 8, bytes.len() - 1] {
        assert!(from_bytes(&bytes[..end]).is_err());
    }
}
#[test]
fn layout_27_without_checkpoint_remains_compatible_with_same_simulation() {
    assert_eq!(goat_save::save::SIM_VERSION, goat_world::checkpoint::MODEL);
    use goat_save::save::{from_bytes, to_bytes};
    let state = setup_state();
    let mut bytes = to_bytes(&from_world_state(
        &state,
        &state.players.snapshot(state.pc_player_id.unwrap()),
    ));
    bytes[4..8].copy_from_slice(&27u32.to_le_bytes());
    let data = from_bytes(&bytes).unwrap();
    assert!(data.resume_checkpoint.is_empty());
}

#[test]
fn atomic_save_replaces_complete_file_and_cleans_failed_rename() {
    let dir = std::env::temp_dir().join(format!("goat_atomic_save_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut state = WorldState::new();
    let view = goat_core::player::PlayerView::default();
    let path = dir.join("career.gsav");
    save_to_file(&from_world_state(&state, &view), &path).unwrap();
    state.world_seed = 9876;
    save_to_file(&from_world_state(&state, &view), &path).unwrap();
    assert_eq!(load_from_file(&path).unwrap().world_seed, 9876);
    let occupied = dir.join("occupied");
    std::fs::create_dir_all(&occupied).unwrap();
    let marker = occupied.join("preserve");
    std::fs::write(&marker, b"existing").unwrap();
    assert!(save_to_file(&from_world_state(&state, &view), &occupied).is_err());
    assert_eq!(std::fs::read(&marker).unwrap(), b"existing");
    assert!(std::fs::read_dir(&dir).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .ends_with(".tmp")));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn sim13_legacy_remains_readable_but_cannot_claim_dated_competitions() {
    let state = setup_state();
    let mut bytes = goat_save::save::to_bytes(&from_world_state(&state, &state.pc_display_view()));
    let trailer = bytes.len() - 4;
    bytes[trailer..].copy_from_slice(&13u32.to_le_bytes());
    assert!(goat_save::save::from_bytes(&bytes).is_ok());
    bytes[trailer..].copy_from_slice(&12u32.to_le_bytes());
    assert!(goat_save::save::from_bytes(&bytes).is_err());
    let mut existing = state;
    existing.dated_calendar = true;
    let existing = reduce(
        existing,
        Intent::EnableDatedCompetitions,
        &mut GoatRng::new(0),
    );
    assert!(existing.competition_calendar.is_none());
    let mut dated = WorldState::new();
    dated.dated_calendar = true;
    dated.career_base_year = 2023;
    dated.pc_player_id = Some(dated.players.push(Default::default()));
    dated = reduce(dated, Intent::EnableDatedCompetitions, &mut GoatRng::new(0));
    assert!(dated.competition_calendar.is_some());
    let mut bytes = goat_save::save::to_bytes(&from_world_state(&dated, &dated.pc_display_view()));
    let trailer = bytes.len() - 4;
    bytes[trailer..].copy_from_slice(&13u32.to_le_bytes());
    assert!(goat_save::save::from_bytes(&bytes).is_err());
}
