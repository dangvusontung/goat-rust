//! Binary save format — little-endian, fixed-size fields, no external deps.
//! (The header comment said "big-endian" for years; every field has always been LE.)
//!
//! Magic: b"GOAT" (4 bytes)
//! Version: u32 (4 bytes) — the LAYOUT version.
//! Then all fields as little-endian primitives.

use goat_core::{attrs::NUM_ATTRS, player::PlayerView, roles::NUM_ROLES, state::WorldState};
use std::io;
use std::path::Path;

pub const MAGIC: &[u8; 4] = b"GOAT";
/// v8+: adds `pc_lifestyle_score` (the emergent-lifestyle readout, bible §8.5/§8.6) as a
/// new appended field. `pc_lifestyle` is still persisted for legacy readers but is now
/// fully re-derivable from the score via `lifestyle_tier_from_score` — `to_world_state`
/// recomputes it rather than trusting the stored byte.
/// v9+: adds the Pantheon raw-signal evidence (Design round 1) — `pc_career_standout_matches`,
/// `pc_season_standout_matches`, `pc_career_best_ovr`, `pc_career_transfer_requests`,
/// `pc_season_transfer_requests`. The two `pc_season_*` staging fields are persisted too
/// (like `pc_season_goals` already is) so a mid-season save/load doesn't lose in-progress
/// counters.
/// v10+: world genesis scale-up (Design round 2, Doc A) — `table_raw` widens from
/// `[u32; 80]` (5 × 16 clubs/tier) to `[u32; 100]` (5 × 20 clubs/tier), since
/// `CLUBS_PER_DIV` itself changed. This is a real binary-layout break (not just an
/// appended field — `table_raw` sits mid-stream), so unlike v8/v9 it cannot be a pure
/// tail-append: `from_bytes` reads the old 80-wide layout for `ver < 10` and discards it
/// (a season's half-played table has no meaningful mapping into the regenerated
/// ~1,200-club world anyway — every club id means something different post-scale-up).
/// Also adds `pc_career_caps`/`pc_career_international_goals` (Doc B §B.4) as new tail
/// fields, defaulted to 0 for older saves.
/// v11+: (Design round 4, Slice 5 §5.1/§5.2) `pc_suspension_weeks: u32` (a single global
/// scalar) is replaced by `pc_suspensions: Vec<(competition_id, matches_remaining)>` — a
/// ban is now scoped to the exact competition it was earned in. Another real mid-stream
/// layout break, same idiom as v10's `table_raw` widening: `from_bytes` reads the old
/// bare-`u32` shape for `ver < 11` and migrates a nonzero value into a single
/// League-scoped ledger entry (the only competition that could suspend a player before
/// this slice), rather than a pure tail-append.
/// v12+: (Design round 5, Doc A §Slice 1) adds `club_budgets: Vec<i64>` — every AI club's
/// running transfer/wage war-chest, £k, indexed by `ClubId`. A pure tail-append (a
/// length-prefixed list, defaulting to empty for older saves), unlike v10/v11's mid-stream
/// breaks.
/// v13+: (Design round 5, Doc A §Slice 6) adds `academy_boosts: Vec<u8>` — every AI club's
/// youth-academy investment lever, indexed by `ClubId`. Another pure tail-append, same idiom
/// as v12's `club_budgets`.
/// v14+: (Design round 5, Slice 7-8 §7.1/8.4) adds the `ManagerPool`'s three fields —
/// `manager_blob` (every generated manager, packed like `peer_blob`), `club_manager`, and
/// `free_agents` (both length-prefixed `Vec<u32>` `ManagerId` lists) — path-dependent
/// fire/rehire history and rolling form that cannot be regenerated from `world_seed` alone.
/// Another pure tail-append, same idiom as v12/v13.
/// v15+: (BL5.1 goal/assist split) adds `pc_season_assists`/`pc_career_assists` as two
/// trailing u32s — a pure tail-append, same idiom as v12/v13/v14; pre-15 saves default
/// both to 0 via `.unwrap_or(0)` reads.
/// v16+: (BL5.2 decisive moments) adds `pc_season_decisive_moments` — the live season
/// staging counter for `pc_decisive_moments` (which was already persisted). Persisted
/// like every other `pc_season_*` staging field so a mid-season save/load doesn't lose
/// it. Pure tail-append; pre-16 saves default it to 0.
/// v17+: (BL5.3 clutch index) adds `pc_season_clutch_index`/`pc_career_clutch_index`
/// as two trailing u32s — a pure tail-append, same idiom as v15/v16; pre-17 saves
/// default both to 0.
/// v18+: (A3.3 live promotion/relegation) adds `pc_nation_membership` — the PC's
/// nation's 3 leagues × 20 club ids, flattened in tier order, as a length-prefixed
/// `Vec<u32>`. Path-dependent (driven by real played results), so it must persist;
/// pre-18 saves default to empty, which every reader treats as genesis-static.
/// v19+: adds `career_base_year` — the real-world year the career started, read
/// from wall-clock once by the outer layer at new-game. One trailing u32;
/// pre-19 saves default to 2025 (the old hardcoded BASE_CAREER_YEAR), keeping
/// their displayed dates byte-identical.
/// v20+: adds `sim_version` — one trailing u32, pure tail-append. Pre-20 saves read
/// it as 0, which mismatches the current SIM_VERSION and is refused on load (their
/// sim behaviour predates the ceiling-lottery restore, so they are genuinely
/// incompatible).
pub const VERSION: u32 = 20;

/// The SIMULATION-BEHAVIOUR version — independent of the layout VERSION above.
/// Bump this whenever a change alters sim outcomes without changing the binary layout
/// (tuning constants, roll formulas, RNG consumption order). `VERSION` guards "can we
/// parse these bytes"; `SIM_VERSION` guards "do these bytes still mean the same world".
/// Without this guard a sim change with no layout change loads old saves *silently*
/// into a differently-computed universe — the exact failure mode the design's
/// save-versioning decision (decision list §3) exists to prevent.
pub const SIM_VERSION: u32 = 1;

/// All the path-dependent data that must be persisted across save/load.
#[derive(Debug, Clone)]
pub struct SaveData {
    // ── World seed ────────────────────────────────────────────────────────────
    pub world_seed: u64,
    // ── PC creation ───────────────────────────────────────────────────────────
    pub pc_name: String,
    pub pc_position: u8, // PrimaryPosition as u8 (0..7)
    pub pc_nationality_idx: u8,
    pub pc_club_idx: u16,
    pub pc_div_idx: u8,
    // ── PC attributes (path-dependent — changed by training) ──────────────────
    pub pc_current_attrs: [i32; NUM_ATTRS], // Fixed::raw values
    pub pc_familiarity: [u8; NUM_ROLES],    // FamiliarityTier as 0-3
    pub pc_familiarity_xp: [i32; NUM_ROLES], // Fixed::raw values
    pub pc_age_weeks: u32,
    pub pc_energy: i32, // Fixed::raw
    pub pc_injury_weeks: u32,
    // ── PC routine ────────────────────────────────────────────────────────────
    pub routine_attrs: Vec<u8>, // AttrId as u8
    pub routine_intensity: u8,  // Intensity as 0/1/2
    // ── Season ────────────────────────────────────────────────────────────────
    pub season_number: u32,
    pub season_round: u32,
    pub pc_form: i32, // Fixed::raw
    pub pc_season_goals: u32,
    pub pc_season_matches: u32,
    pub pc_season_output: i32,
    pub table_raw: [u32; 100], // 5 × CLUBS_PER_DIV (20, v10+)
    // ── Phase 6 discipline ────────────────────────────────────────────────────
    pub pc_yellow_cards_season: u32,
    /// (competition_id, matches_remaining) per active ban (v11+, Design round 4 Slice 5
    /// §5.1) — replaces the old single global scalar.
    pub pc_suspensions: Vec<(u32, u32)>,
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
    pub pc_sporting_rep: i32,
    pub pc_club_fan_rep: i32,
    // ── Phase 8 contract + market ─────────────────────────────────────────────
    pub pc_contract_seasons_left: u32,
    pub pc_wage_annual: i64,
    pub pc_power_ladder: u8,
    pub pc_savings: i64,
    // ── Phase 9 peers ─────────────────────────────────────────────────────────
    /// Packed peer data: for each peer: seed(u64), goals(u32), matches(u32), avg(u8), titles(u32) + name_len(u8) + name bytes
    pub peer_blob: Vec<u8>,
    pub pc_rival_idx: Option<u8>,
    pub pc_rival_declared_season: Option<u32>,
    // ── Phase 10 lifestyle + retirement ──────────────────────────────────────
    pub pc_lifestyle: u8,
    pub pc_retired: bool,
    // ── Calendar ─────────────────────────────────────────────────────────────
    pub pc_week_training_done: bool,
    /// Live calendar position in epoch days since career start (v6+).
    pub pc_epoch_day: u32,
    // ── Phase 10 economy + life (v7+) ─────────────────────────────────────────
    pub pc_business_value: i64,
    pub pc_bankrupt: bool,
    pub pc_dev_invest_level: u8,
    pub pc_marketability: i32,
    pub pc_sponsor_tier: u8,
    pub pc_relationships: [i32; 3],
    pub pc_character_rep: i32,
    // ── Lifestyle score (v8+) ──────────────────────────────────────────────────
    /// Raw `Fixed` value of the emergent lifestyle score (bible §8.5/§8.6). New saves
    /// write it; older saves default to 0 (Balanced) on load.
    pub pc_lifestyle_score: i32,
    // ── Pantheon raw-signal evidence (v9+) ────────────────────────────────────
    pub pc_career_standout_matches: u32,
    pub pc_season_standout_matches: u32,
    pub pc_career_best_ovr: i32,
    pub pc_career_transfer_requests: u32,
    pub pc_season_transfer_requests: u32,
    // ── National-team caps (v10+, Design round 2 Doc B §B.4) ──────────────────
    pub pc_career_caps: u32,
    pub pc_season_caps: u32,
    pub pc_career_international_goals: u32,
    pub pc_season_international_goals: u32,
    // ── Club economy (v12+, Design round 5 Doc A §Slice 1) ────────────────────
    /// Every AI club's running war-chest, £k, indexed by `ClubId`. Empty for saves that
    /// never seeded it (older saves, or a fresh game before genesis wiring).
    pub club_budgets: Vec<i64>,
    // ── Club economy (v13+, Design round 5 Doc A §Slice 6) ────────────────────
    /// Every AI club's academy-boost lever, indexed by `ClubId`. Empty for saves that never
    /// seeded it (older saves, or a fresh game before genesis wiring).
    pub academy_boosts: Vec<u8>,
    // ── Managers (v14+, Design round 5 Slice 7-8) ──────────────────────────────
    /// Packed manager data: for each manager: name (u8 len + bytes) + identity_bias
    /// (`NUM_ROLES` × i32 `Fixed` raw) + recent_points (`MANAGER_FORM_WINDOW` × u8) +
    /// recent_idx (u8) + tenure_start_season (u32) + matches_played (u16). Mirrors
    /// `peer_blob`'s packed-blob idiom.
    pub manager_blob: Vec<u8>,
    /// Per-club current manager index into the unpacked `manager_blob`, indexed by `ClubId`.
    pub club_manager: Vec<u32>,
    /// Currently-unemployed manager indices into the unpacked `manager_blob`.
    pub free_agents: Vec<u32>,
    // ── Goal/assist split (v15+, BL5.1) ───────────────────────────────────────
    /// Live season counter, persisted (like `pc_season_goals`) so a mid-season
    /// save/load doesn't lose it; folded into the career counter at season end.
    pub pc_season_assists: u32,
    pub pc_career_assists: u32,
    // ── Decisive moments (v16+, BL5.2) ────────────────────────────────────────
    /// Live season staging counter for `pc_decisive_moments` (already persisted
    /// above); folded into the career counter at season end.
    pub pc_season_decisive_moments: u32,
    // ── Clutch index (v17+, BL5.3) ────────────────────────────────────────────
    /// Live season staging counter, persisted like every other `pc_season_*`
    /// staging field; folded into the career index at season end.
    pub pc_season_clutch_index: u32,
    pub pc_career_clutch_index: u32,
    // ── Live promotion/relegation (v18+, A3.3) ────────────────────────────────
    /// PC's nation's current league membership: 3 leagues × 20 club ids,
    /// flattened in tier order. Empty = genesis-static (pre-18 saves).
    pub pc_nation_membership: Vec<u32>,
    // ── Career epoch (v19+) ───────────────────────────────────────────────────
    /// Real-world year the career started (wall-clock, captured by the outer
    /// layer at new-game). Pre-19 saves default to 2025.
    pub career_base_year: u32,
}

#[derive(Debug)]
pub enum SaveError {
    Io(io::Error),
    BadMagic,
    BadVersion(u32),
    /// The save parses fine but was written by an incompatible simulation —
    /// its world would silently recompute differently. Refuse, don't migrate.
    SimVersionMismatch { found: u32, expected: u32 },
    Corrupt(&'static str),
}

impl From<io::Error> for SaveError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Io(e) => write!(f, "IO error: {e}"),
            SaveError::BadMagic => write!(f, "not a GOAT save file"),
            SaveError::BadVersion(v) => write!(f, "unsupported save version {v}"),
            SaveError::SimVersionMismatch { found, expected } => write!(
                f,
                "save was written by sim version {found}, this build runs sim version {expected} — the world would recompute differently; start a new career or use the older build"
            ),
            SaveError::Corrupt(s) => write!(f, "corrupt save: {s}"),
        }
    }
}

/// Extract save data from WorldState + PC player view.
pub fn from_world_state(state: &WorldState, view: &PlayerView) -> SaveData {
    let familiarity_bytes: [u8; NUM_ROLES] = core::array::from_fn(|r| view.familiarity[r] as u8);
    let familiarity_xp: [i32; NUM_ROLES] =
        core::array::from_fn(|r| view.familiarity_xp[r].to_raw());
    let current_attrs: [i32; NUM_ATTRS] = core::array::from_fn(|a| view.current[a].to_raw());

    let routine_attrs: Vec<u8> = state
        .pc_routine
        .focus_attrs
        .iter()
        .map(|&a| a as u8)
        .collect();
    let routine_intensity: u8 = state.pc_routine.intensity as u8;

    SaveData {
        world_seed: state.world_seed,
        pc_name: view.name.clone(),
        pc_position: state.pc_position,
        pc_nationality_idx: 0u8,
        pc_club_idx: state.pc_club_idx,
        pc_div_idx: state.pc_div_idx,
        pc_current_attrs: current_attrs,
        pc_familiarity: familiarity_bytes,
        pc_familiarity_xp: familiarity_xp,
        pc_age_weeks: view.age_weeks,
        pc_energy: view.energy.to_raw(),
        pc_injury_weeks: view.injury_weeks,
        routine_attrs,
        routine_intensity,
        season_number: state.season_number,
        season_round: state.season_round,
        pc_form: state.pc_form.to_raw(),
        pc_season_goals: state.pc_season_goals,
        pc_season_matches: state.pc_season_matches,
        pc_season_output: state.pc_season_output,
        table_raw: state.table_raw,
        pc_yellow_cards_season: state.pc_yellow_cards_season,
        pc_suspensions: state
            .pc_suspensions
            .iter()
            .map(|l| (l.competition_id, l.matches_remaining))
            .collect(),
        pc_discipline_rep: state.pc_discipline_rep,
        pc_career_goals: state.pc_career_goals,
        pc_career_matches: state.pc_career_matches,
        pc_career_output_sum: state.pc_career_output_sum,
        pc_best_season_avg_output: state.pc_best_season_avg_output,
        pc_seasons_played: state.pc_seasons_played,
        pc_decisive_moments: state.pc_decisive_moments,
        pc_player_of_year_wins: state.pc_player_of_year_wins,
        pc_league_titles: state.pc_league_titles,
        pc_clubs_served: state.pc_clubs_served,
        pc_longest_club_tenure: state.pc_longest_club_tenure,
        pc_sporting_rep: state.pc_sporting_rep,
        pc_club_fan_rep: state.pc_club_fan_rep,
        pc_contract_seasons_left: state.pc_contract_seasons_left,
        pc_wage_annual: state.pc_wage_annual,
        pc_power_ladder: state.pc_power_ladder,
        pc_savings: state.pc_savings,
        peer_blob: encode_peers(&state.pc_peers),
        pc_rival_idx: state.pc_rival_idx.map(|i| i as u8),
        pc_rival_declared_season: state.pc_rival_declared_season,
        pc_lifestyle: state.pc_lifestyle,
        pc_retired: state.pc_retired,
        pc_week_training_done: state.pc_week_training_done,
        pc_epoch_day: state.pc_epoch_day,
        pc_business_value: state.pc_business_value,
        pc_bankrupt: state.pc_bankrupt,
        pc_dev_invest_level: state.pc_dev_invest_level,
        pc_marketability: state.pc_marketability,
        pc_sponsor_tier: state.pc_sponsor_tier,
        pc_relationships: state.pc_relationships,
        pc_character_rep: state.pc_character_rep,
        pc_lifestyle_score: state.pc_lifestyle_score.to_raw(),
        pc_career_standout_matches: state.pc_career_standout_matches,
        pc_season_standout_matches: state.pc_season_standout_matches,
        pc_career_best_ovr: state.pc_career_best_ovr,
        pc_career_transfer_requests: state.pc_career_transfer_requests,
        pc_season_transfer_requests: state.pc_season_transfer_requests,
        pc_career_caps: state.pc_career_caps,
        pc_season_caps: state.pc_season_caps,
        pc_career_international_goals: state.pc_career_international_goals,
        pc_season_international_goals: state.pc_season_international_goals,
        club_budgets: state.club_budgets.clone(),
        academy_boosts: state.academy_boosts.clone(),
        manager_blob: encode_managers(&state.managers),
        club_manager: state.club_manager.clone(),
        free_agents: state.free_agents.clone(),
        pc_season_assists: state.pc_season_assists,
        pc_career_assists: state.pc_career_assists,
        pc_season_decisive_moments: state.pc_season_decisive_moments,
        pc_season_clutch_index: state.pc_season_clutch_index,
        pc_career_clutch_index: state.pc_career_clutch_index,
        pc_nation_membership: state.pc_nation_membership.clone(),
        career_base_year: state.career_base_year,
    }
}

fn encode_managers(managers: &[goat_core::state::ManagerState]) -> Vec<u8> {
    let mut v = Vec::new();
    push_u32(&mut v, managers.len() as u32);
    for m in managers {
        let name_bytes = m.name.as_bytes();
        v.push(name_bytes.len().min(63) as u8);
        v.extend_from_slice(&name_bytes[..name_bytes.len().min(63)]);
        for w in m.identity_bias.role_weight {
            push_i32(&mut v, w.to_raw());
        }
        v.extend_from_slice(&m.recent_points);
        v.push(m.recent_idx);
        push_u32(&mut v, m.tenure_start_season);
        push_u16(&mut v, m.matches_played);
    }
    v
}

fn decode_managers(blob: &[u8]) -> Vec<goat_core::state::ManagerState> {
    use goat_core::state::{ManagerState, MANAGER_FORM_WINDOW};
    use goat_core::tactical_identity::TacticalIdentity;
    use goat_fixed::Fixed;

    if blob.len() < 4 {
        return Vec::new();
    }
    let mut cur = 0usize;
    let count = u32::from_le_bytes(blob[0..4].try_into().unwrap_or([0; 4])) as usize;
    cur += 4;
    let mut managers = Vec::with_capacity(count.min(4096));
    for _ in 0..count {
        let Some(&name_len) = blob.get(cur) else {
            break;
        };
        let name_len = name_len as usize;
        cur += 1;
        let name = std::str::from_utf8(blob.get(cur..cur + name_len).unwrap_or(b""))
            .unwrap_or("?")
            .to_string();
        cur += name_len;

        let mut role_weight = [Fixed::ZERO; NUM_ROLES];
        for w in role_weight.iter_mut() {
            let raw = blob
                .get(cur..cur + 4)
                .and_then(|s| s.try_into().ok())
                .map(i32::from_le_bytes)
                .unwrap_or(0);
            *w = Fixed::raw(raw);
            cur += 4;
        }

        let mut recent_points = [0u8; MANAGER_FORM_WINDOW];
        for p in recent_points.iter_mut() {
            *p = blob.get(cur).copied().unwrap_or(0);
            cur += 1;
        }
        let recent_idx = blob.get(cur).copied().unwrap_or(0);
        cur += 1;
        let tenure_start_season = blob
            .get(cur..cur + 4)
            .and_then(|s| s.try_into().ok())
            .map(u32::from_le_bytes)
            .unwrap_or(0);
        cur += 4;
        let matches_played = blob
            .get(cur..cur + 2)
            .and_then(|s| s.try_into().ok())
            .map(u16::from_le_bytes)
            .unwrap_or(0);
        cur += 2;

        managers.push(ManagerState {
            name,
            identity_bias: TacticalIdentity { role_weight },
            recent_points,
            recent_idx,
            tenure_start_season,
            matches_played,
        });
    }
    managers
}

fn encode_peers(peers: &[goat_core::state::PeerState]) -> Vec<u8> {
    let mut v = Vec::new();
    v.push(peers.len() as u8);
    for p in peers {
        v.extend_from_slice(&p.seed.to_le_bytes());
        v.extend_from_slice(&p.career_goals.to_le_bytes());
        v.extend_from_slice(&p.career_matches.to_le_bytes());
        v.push(p.avg_output);
        v.extend_from_slice(&p.titles.to_le_bytes());
        let name_bytes = p.name.as_bytes();
        v.push(name_bytes.len().min(63) as u8);
        v.extend_from_slice(&name_bytes[..name_bytes.len().min(63)]);
        let nat_bytes = p.nationality.as_bytes();
        v.push(nat_bytes.len().min(31) as u8);
        v.extend_from_slice(&nat_bytes[..nat_bytes.len().min(31)]);
    }
    v
}

fn decode_peers(blob: &[u8]) -> Vec<goat_core::state::PeerState> {
    use goat_core::state::PeerState;
    if blob.is_empty() {
        return Vec::new();
    }
    let count = blob[0] as usize;
    let mut cur = 1usize;
    let mut peers = Vec::new();
    for _ in 0..count {
        if cur + 8 + 4 + 4 + 1 + 4 > blob.len() {
            break;
        }
        let seed = u64::from_le_bytes(blob[cur..cur + 8].try_into().unwrap_or([0; 8]));
        cur += 8;
        let career_goals = u32::from_le_bytes(blob[cur..cur + 4].try_into().unwrap_or([0; 4]));
        cur += 4;
        let career_matches = u32::from_le_bytes(blob[cur..cur + 4].try_into().unwrap_or([0; 4]));
        cur += 4;
        let avg_output = blob[cur];
        cur += 1;
        let titles = u32::from_le_bytes(blob[cur..cur + 4].try_into().unwrap_or([0; 4]));
        cur += 4;
        let name_len = blob.get(cur).copied().unwrap_or(0) as usize;
        cur += 1;
        let name = std::str::from_utf8(blob.get(cur..cur + name_len).unwrap_or(b""))
            .unwrap_or("?")
            .to_string();
        cur += name_len;
        let nat_len = blob.get(cur).copied().unwrap_or(0) as usize;
        cur += 1;
        let nationality = std::str::from_utf8(blob.get(cur..cur + nat_len).unwrap_or(b""))
            .unwrap_or("?")
            .to_string();
        cur += nat_len;
        peers.push(PeerState {
            seed,
            name,
            nationality,
            career_goals,
            career_matches,
            avg_output,
            titles,
        });
    }
    peers
}

pub fn save_to_file(data: &SaveData, path: impl AsRef<Path>) -> Result<(), SaveError> {
    let bytes = to_bytes(data);
    std::fs::write(path, bytes)?;
    Ok(())
}

pub fn load_from_file(path: impl AsRef<Path>) -> Result<SaveData, SaveError> {
    let bytes = std::fs::read(path)?;
    from_bytes(&bytes)
}

// ── Save slots (Design round 1, Slice 3) ────────────────────────────────────────
//
// Numbered slots (1-9), not free-text-named ones — avoids filename
// sanitization/path-traversal surface for a text UI that would otherwise need to
// validate arbitrary user-typed strings. Pure save-data logic, no game-loop
// concerns, so it lives here rather than in goat-tui — reusable later by
// goat-bridge for a Flutter multi-slot UI.

/// Summary of one save slot, cheap enough to compute for every slot up front.
pub struct SaveSlotSummary {
    pub slot: u8,
    pub occupied: bool,
    /// Empty string / 0 when `occupied` is false.
    pub pc_name: String,
    pub season_number: u32,
    pub pc_age_weeks: u32,
}

/// The file path for a given numbered slot inside `dir`.
pub fn slot_path(dir: impl AsRef<Path>, slot: u8) -> std::path::PathBuf {
    dir.as_ref().join(format!("slot-{slot}.sav"))
}

/// List slots 1..=num_slots in `dir`. Reads every existing file to summarize it — cheap:
/// each save is a few hundred bytes (tiny-save principle), so reading up to `num_slots` of
/// them stays well inside CALENDAR.md's NFR-02 "load under 1s" budget.
pub fn list_slots(dir: impl AsRef<Path>, num_slots: u8) -> Vec<SaveSlotSummary> {
    (1..=num_slots)
        .map(|slot| match load_from_file(slot_path(&dir, slot)) {
            Ok(data) => SaveSlotSummary {
                slot,
                occupied: true,
                pc_name: data.pc_name,
                season_number: data.season_number,
                pc_age_weeks: data.pc_age_weeks,
            },
            Err(_) => SaveSlotSummary {
                slot,
                occupied: false,
                pc_name: String::new(),
                season_number: 0,
                pc_age_weeks: 0,
            },
        })
        .collect()
}

/// Reconstruct a `WorldState` from saved data.
///
/// Potentials are re-derived from the world seed (they are never stored).
/// Path-dependent fields (current attrs, familiarity, age, energy, etc.) are
/// restored from the save. This is the inverse of `from_world_state`.
///
/// `world` must be `WorldGenesis::generate(data.world_seed)` — regenerated by the caller,
/// same "seed is the universe" pattern as `History`, not persisted in `SaveData` itself.
pub fn to_world_state(data: &SaveData, world: &goat_world::world::WorldGenesis) -> WorldState {
    use goat_core::attrs::AttrId;
    use goat_core::generation::{generate_player, CreationChoices};
    use goat_core::player::PlayerStore;
    use goat_core::positions::PrimaryPosition;
    use goat_core::roles::FamiliarityTier;
    use goat_core::state::{lifestyle_tier_from_score, WorldState};
    use goat_core::week::{Intensity, Routine};

    let club = &world.clubs[data.pc_club_idx as usize];
    let nationality = world.nation_name(club.nation).to_string();
    let primary_position =
        PrimaryPosition::from_u8(data.pc_position).unwrap_or(PrimaryPosition::ST);
    let choices = CreationChoices {
        name: data.pc_name.clone(),
        primary_position,
        nationality: nationality.clone(),
        club: club.name.clone(),
    };

    // Regenerate from world seed to get original potentials.
    let mut view = generate_player(data.world_seed, &choices);

    // Overwrite path-dependent fields from save.
    for (a, &raw) in data.pc_current_attrs.iter().enumerate() {
        view.current[a] = goat_fixed::Fixed::raw(raw);
    }
    for (r, &tier_u8) in data.pc_familiarity.iter().enumerate() {
        view.familiarity[r] = match tier_u8 {
            1 => FamiliarityTier::Unconvincing,
            2 => FamiliarityTier::Competent,
            3 => FamiliarityTier::Natural,
            _ => FamiliarityTier::Awkward,
        };
    }
    for (r, &raw) in data.pc_familiarity_xp.iter().enumerate() {
        view.familiarity_xp[r] = goat_fixed::Fixed::raw(raw);
    }
    view.age_weeks = data.pc_age_weeks;
    view.energy = goat_fixed::Fixed::raw(data.pc_energy);
    view.injury_weeks = data.pc_injury_weeks;
    view.name = data.pc_name.clone();

    let mut players = PlayerStore::new();
    let pc_id = players.push(view);

    let focus_attrs: Vec<AttrId> = data
        .routine_attrs
        .iter()
        .filter(|&&b| (b as usize) < goat_core::attrs::NUM_ATTRS)
        .map(|&b| AttrId::ALL[b as usize])
        .collect();

    let intensity = match data.routine_intensity {
        0 => Intensity::Low,
        2 => Intensity::High,
        _ => Intensity::Medium,
    };

    let mut state = WorldState::new();
    state.players = players;
    state.pc_player_id = Some(pc_id);
    state.pc_routine = Routine {
        focus_attrs,
        intensity,
    };
    state.pc_club = club.name.clone();
    state.pc_nationality = nationality;
    state.pc_position = data.pc_position;
    state.world_seed = data.world_seed;
    state.pc_club_idx = data.pc_club_idx;
    state.pc_div_idx = data.pc_div_idx;
    state.pc_facilities_mult = club.facilities_mult();
    state.season_number = data.season_number;
    state.season_round = data.season_round;
    state.pc_form = goat_fixed::Fixed::raw(data.pc_form);
    state.pc_season_goals = data.pc_season_goals;
    state.pc_season_matches = data.pc_season_matches;
    state.pc_season_output = data.pc_season_output;
    state.table_raw = data.table_raw;
    state.pc_yellow_cards_season = data.pc_yellow_cards_season;
    state.pc_suspensions = data
        .pc_suspensions
        .iter()
        .map(
            |&(competition_id, matches_remaining)| goat_calendar::SuspensionLedger {
                player_id: pc_id,
                competition_id,
                matches_remaining,
            },
        )
        .collect();
    state.pc_discipline_rep = data.pc_discipline_rep;
    state.pc_career_goals = data.pc_career_goals;
    state.pc_career_matches = data.pc_career_matches;
    state.pc_career_output_sum = data.pc_career_output_sum;
    state.pc_best_season_avg_output = data.pc_best_season_avg_output;
    state.pc_seasons_played = data.pc_seasons_played;
    state.pc_decisive_moments = data.pc_decisive_moments;
    state.pc_player_of_year_wins = data.pc_player_of_year_wins;
    state.pc_league_titles = data.pc_league_titles;
    state.pc_clubs_served = data.pc_clubs_served;
    state.pc_longest_club_tenure = data.pc_longest_club_tenure;
    state.pc_sporting_rep = data.pc_sporting_rep;
    state.pc_club_fan_rep = data.pc_club_fan_rep;
    state.pc_contract_seasons_left = data.pc_contract_seasons_left;
    state.pc_wage_annual = data.pc_wage_annual;
    state.pc_power_ladder = data.pc_power_ladder;
    state.pc_savings = data.pc_savings;
    state.pc_peers = decode_peers(&data.peer_blob);
    state.pc_rival_idx = data.pc_rival_idx.map(|i| i as usize);
    state.pc_rival_declared_season = data.pc_rival_declared_season;
    // Lifestyle is derived, not trusted from the stored byte (bible §8.6) — recompute
    // the tier from the score so an old save (score defaults to 0) and a fresh derive
    // always agree.
    state.pc_lifestyle_score = goat_fixed::Fixed::raw(data.pc_lifestyle_score);
    state.pc_lifestyle = lifestyle_tier_from_score(state.pc_lifestyle_score);
    state.pc_retired = data.pc_retired;
    state.pc_week_training_done = data.pc_week_training_done;
    state.pc_epoch_day = data.pc_epoch_day;
    state.pc_business_value = data.pc_business_value;
    state.pc_bankrupt = data.pc_bankrupt;
    state.pc_dev_invest_level = data.pc_dev_invest_level;
    state.pc_marketability = data.pc_marketability;
    state.pc_sponsor_tier = data.pc_sponsor_tier;
    state.pc_relationships = data.pc_relationships;
    state.pc_character_rep = data.pc_character_rep;
    state.pc_career_standout_matches = data.pc_career_standout_matches;
    state.pc_season_standout_matches = data.pc_season_standout_matches;
    state.pc_career_best_ovr = data.pc_career_best_ovr;
    state.pc_career_transfer_requests = data.pc_career_transfer_requests;
    state.pc_season_transfer_requests = data.pc_season_transfer_requests;
    state.pc_career_caps = data.pc_career_caps;
    state.pc_season_caps = data.pc_season_caps;
    state.pc_career_international_goals = data.pc_career_international_goals;
    state.pc_season_international_goals = data.pc_season_international_goals;
    state.club_budgets = data.club_budgets.clone();
    state.academy_boosts = data.academy_boosts.clone();
    state.managers = decode_managers(&data.manager_blob);
    state.club_manager = data.club_manager.clone();
    state.free_agents = data.free_agents.clone();
    state.pc_season_assists = data.pc_season_assists;
    state.pc_career_assists = data.pc_career_assists;
    state.pc_season_decisive_moments = data.pc_season_decisive_moments;
    state.pc_season_clutch_index = data.pc_season_clutch_index;
    state.pc_career_clutch_index = data.pc_career_clutch_index;
    state.pc_nation_membership = data.pc_nation_membership.clone();
    state.career_base_year = data.career_base_year;

    state
}

// ── Serialisation ─────────────────────────────────────────────────────────────

/// Serialize to the raw byte format (no I/O) — the web/WASM boundary stores
/// these bytes itself (localStorage/IndexedDB), per the "no fs in core" rule.
pub fn to_bytes(d: &SaveData) -> Vec<u8> {
    let mut v: Vec<u8> = Vec::new();
    v.extend_from_slice(MAGIC);
    push_u32(&mut v, VERSION);
    push_u64(&mut v, d.world_seed);
    push_str(&mut v, &d.pc_name);
    v.push(d.pc_position);
    v.push(d.pc_nationality_idx);
    push_u16(&mut v, d.pc_club_idx);
    v.push(d.pc_div_idx);
    for &x in &d.pc_current_attrs {
        push_i32(&mut v, x);
    }
    for &x in &d.pc_familiarity {
        v.push(x);
    }
    for &x in &d.pc_familiarity_xp {
        push_i32(&mut v, x);
    }
    push_u32(&mut v, d.pc_age_weeks);
    push_i32(&mut v, d.pc_energy);
    push_u32(&mut v, d.pc_injury_weeks);
    push_u32(&mut v, d.routine_attrs.len() as u32);
    v.extend_from_slice(&d.routine_attrs);
    v.push(d.routine_intensity);
    push_u32(&mut v, d.season_number);
    push_u32(&mut v, d.season_round);
    push_i32(&mut v, d.pc_form);
    push_u32(&mut v, d.pc_season_goals);
    push_u32(&mut v, d.pc_season_matches);
    push_i32(&mut v, d.pc_season_output);
    for &x in &d.table_raw {
        push_u32(&mut v, x);
    }
    // Phase 6 fields
    push_u32(&mut v, d.pc_yellow_cards_season);
    // v11+: length-prefixed (competition_id, matches_remaining) pairs, replacing the
    // old bare-u32 `pc_suspension_weeks` scalar.
    push_u32(&mut v, d.pc_suspensions.len() as u32);
    for &(competition_id, matches_remaining) in &d.pc_suspensions {
        push_u32(&mut v, competition_id);
        push_u32(&mut v, matches_remaining);
    }
    push_i32(&mut v, d.pc_discipline_rep);
    // Phase 7 legacy fields
    push_u32(&mut v, d.pc_career_goals);
    push_u32(&mut v, d.pc_career_matches);
    push_u64(&mut v, d.pc_career_output_sum as u64);
    push_i32(&mut v, d.pc_best_season_avg_output);
    push_u32(&mut v, d.pc_seasons_played);
    push_u32(&mut v, d.pc_decisive_moments);
    push_u32(&mut v, d.pc_player_of_year_wins);
    push_u32(&mut v, d.pc_league_titles);
    push_u32(&mut v, d.pc_clubs_served);
    push_u32(&mut v, d.pc_longest_club_tenure);
    push_i32(&mut v, d.pc_sporting_rep);
    push_i32(&mut v, d.pc_club_fan_rep);
    // Phase 8 fields
    push_u32(&mut v, d.pc_contract_seasons_left);
    push_u64(&mut v, d.pc_wage_annual as u64);
    v.push(d.pc_power_ladder);
    push_u64(&mut v, d.pc_savings as u64);
    // Phase 9 peers
    push_u32(&mut v, d.peer_blob.len() as u32);
    v.extend_from_slice(&d.peer_blob);
    v.push(d.pc_rival_idx.unwrap_or(255));
    push_u32(&mut v, d.pc_rival_declared_season.unwrap_or(0));
    // Phase 10
    v.push(d.pc_lifestyle);
    v.push(u8::from(d.pc_retired));
    // Calendar
    v.push(u8::from(d.pc_week_training_done));
    push_u32(&mut v, d.pc_epoch_day); // v6+
                                      // Phase 10 economy + life (v7+)
    push_u64(&mut v, d.pc_business_value as u64);
    v.push(u8::from(d.pc_bankrupt));
    v.push(d.pc_dev_invest_level);
    push_i32(&mut v, d.pc_marketability);
    v.push(d.pc_sponsor_tier);
    for r in d.pc_relationships {
        push_i32(&mut v, r);
    }
    push_i32(&mut v, d.pc_character_rep);
    // v8+
    push_i32(&mut v, d.pc_lifestyle_score);
    // v9+ — Pantheon raw-signal evidence
    push_u32(&mut v, d.pc_career_standout_matches);
    push_u32(&mut v, d.pc_season_standout_matches);
    push_i32(&mut v, d.pc_career_best_ovr);
    push_u32(&mut v, d.pc_career_transfer_requests);
    push_u32(&mut v, d.pc_season_transfer_requests);
    // v10+ — national-team caps (Design round 2 Doc B §B.4)
    push_u32(&mut v, d.pc_career_caps);
    push_u32(&mut v, d.pc_season_caps);
    push_u32(&mut v, d.pc_career_international_goals);
    push_u32(&mut v, d.pc_season_international_goals);
    // v12+ — club economy (Design round 5 Doc A §Slice 1): length-prefixed war-chest list.
    push_u32(&mut v, d.club_budgets.len() as u32);
    for &budget in &d.club_budgets {
        push_u64(&mut v, budget as u64);
    }
    // v13+ — academy investment (Design round 5 Doc A §Slice 6): length-prefixed boost list.
    push_u32(&mut v, d.academy_boosts.len() as u32);
    for &boost in &d.academy_boosts {
        v.push(boost);
    }
    // v14+ — managers (Design round 5 Slice 7-8): packed manager blob + two ManagerId lists.
    push_u32(&mut v, d.manager_blob.len() as u32);
    v.extend_from_slice(&d.manager_blob);
    push_u32(&mut v, d.club_manager.len() as u32);
    for &id in &d.club_manager {
        push_u32(&mut v, id);
    }
    push_u32(&mut v, d.free_agents.len() as u32);
    for &id in &d.free_agents {
        push_u32(&mut v, id);
    }
    // v15+ — goal/assist split (BL5.1): two trailing u32s, pure tail-append.
    push_u32(&mut v, d.pc_season_assists);
    push_u32(&mut v, d.pc_career_assists);
    // v16+ — decisive moments (BL5.2): one trailing u32, pure tail-append.
    push_u32(&mut v, d.pc_season_decisive_moments);
    // v17+ — clutch index (BL5.3): two trailing u32s, pure tail-append.
    push_u32(&mut v, d.pc_season_clutch_index);
    push_u32(&mut v, d.pc_career_clutch_index);
    // v18+ — live promotion/relegation (A3.3): length-prefixed club-id list.
    push_u32(&mut v, d.pc_nation_membership.len() as u32);
    for &id in &d.pc_nation_membership {
        push_u32(&mut v, id);
    }
    // v19+ — career base year (one trailing u32).
    push_u32(&mut v, d.career_base_year);
    // v20+ — simulation-behaviour version (one trailing u32). Always the CURRENT
    // constant on write; the guard lives in `from_bytes`.
    push_u32(&mut v, SIM_VERSION);
    v
}

/// Deserialize from the raw byte format (no I/O) — the web/WASM counterpart of
/// `to_bytes`, for clients that store save bytes themselves.
///
/// This is the GUARDED entry point: it enforces the `SIM_VERSION` check. Use
/// `from_bytes_layout_only` only for layout-migration tests / future migration tooling.
pub fn from_bytes(b: &[u8]) -> Result<SaveData, SaveError> {
    let (data, sim_version) = parse(b)?;
    if sim_version != SIM_VERSION {
        return Err(SaveError::SimVersionMismatch {
            found: sim_version,
            expected: SIM_VERSION,
        });
    }
    Ok(data)
}

/// Layout-migration parse WITHOUT the sim-version guard. Pre-v20 byte streams decode
/// with sim_version = 0 and would be refused by `from_bytes`; this entry exists so the
/// v8–v19 tail-append migration logic stays exercised by its round-trip tests.
/// Never wire this into a load path players can reach.
pub fn from_bytes_layout_only(b: &[u8]) -> Result<SaveData, SaveError> {
    Ok(parse(b)?.0)
}

/// Inner parser: returns the data plus the save's sim_version (0 for pre-v20 layouts).
fn parse(b: &[u8]) -> Result<(SaveData, u32), SaveError> {
    if b.len() < 8 {
        return Err(SaveError::Corrupt("too short"));
    }
    if &b[0..4] != MAGIC {
        return Err(SaveError::BadMagic);
    }
    let mut cur = 4usize;
    let ver = read_u32(b, &mut cur)?;
    if ver > VERSION {
        return Err(SaveError::BadVersion(ver));
    }

    let world_seed = read_u64(b, &mut cur)?;
    let pc_name = read_str(b, &mut cur)?;
    let pc_position = read_u8(b, &mut cur)?;
    let pc_nationality_idx = read_u8(b, &mut cur)?;
    let pc_club_idx = read_u16(b, &mut cur)?;
    let pc_div_idx = read_u8(b, &mut cur)?;
    let mut pc_current_attrs = [0i32; NUM_ATTRS];
    for x in &mut pc_current_attrs {
        *x = read_i32(b, &mut cur)?;
    }
    let mut pc_familiarity = [0u8; NUM_ROLES];
    for x in &mut pc_familiarity {
        *x = read_u8(b, &mut cur)?;
    }
    let mut pc_familiarity_xp = [0i32; NUM_ROLES];
    for x in &mut pc_familiarity_xp {
        *x = read_i32(b, &mut cur)?;
    }
    let pc_age_weeks = read_u32(b, &mut cur)?;
    let pc_energy = read_i32(b, &mut cur)?;
    let pc_injury_weeks = read_u32(b, &mut cur)?;
    let nattrs = read_u32(b, &mut cur)? as usize;
    if cur + nattrs > b.len() {
        return Err(SaveError::Corrupt("routine_attrs too long"));
    }
    let routine_attrs: Vec<u8> = b[cur..cur + nattrs].to_vec();
    cur += nattrs;
    let routine_intensity = read_u8(b, &mut cur)?;
    let season_number = read_u32(b, &mut cur)?;
    let season_round = read_u32(b, &mut cur)?;
    let pc_form = read_i32(b, &mut cur)?;
    let pc_season_goals = read_u32(b, &mut cur)?;
    let pc_season_matches = read_u32(b, &mut cur)?;
    let pc_season_output = read_i32(b, &mut cur)?;
    // v10+ widened table_raw from 80 (5×16) to 100 (5×20) — a real mid-stream layout
    // break, not a tail-append. Pre-v10 saves' 80-wide table has no meaningful mapping
    // into the regenerated ~1,200-club world anyway (every club id means something
    // different post-scale-up), so it's read (to keep the cursor aligned for the fields
    // that follow) and discarded rather than reinterpreted.
    let mut table_raw = [0u32; 100];
    if ver < 10 {
        for _ in 0..80 {
            read_u32(b, &mut cur)?;
        }
    } else {
        for x in &mut table_raw {
            *x = read_u32(b, &mut cur)?;
        }
    }
    // Phase 6 fields (default if missing — supports older saves)
    let pc_yellow_cards_season = read_u32(b, &mut cur).unwrap_or(0);
    // v11+: `pc_suspension_weeks` (bare u32) becomes a length-prefixed list of
    // (competition_id, matches_remaining) pairs — a real mid-stream layout break, same
    // idiom as v10's `table_raw` widening. `ver < 11` migrates a nonzero old scalar into
    // a single League-scoped ledger entry (the only competition that could suspend a
    // player before this slice).
    let pc_suspensions: Vec<(u32, u32)> = if ver < 11 {
        let old = read_u32(b, &mut cur).unwrap_or(0);
        if old > 0 {
            vec![(goat_core::calendar_loop::LEAGUE_COMPETITION_ID, old)]
        } else {
            Vec::new()
        }
    } else {
        let count = read_u32(b, &mut cur).unwrap_or(0) as usize;
        let mut list = Vec::with_capacity(count.min(64));
        for _ in 0..count {
            let competition_id = read_u32(b, &mut cur).unwrap_or(0);
            let matches_remaining = read_u32(b, &mut cur).unwrap_or(0);
            list.push((competition_id, matches_remaining));
        }
        list
    };
    let pc_discipline_rep = read_i32(b, &mut cur).unwrap_or(50);
    // Phase 7 legacy fields (all default to 0 / 50 if missing)
    let pc_career_goals = read_u32(b, &mut cur).unwrap_or(0);
    let pc_career_matches = read_u32(b, &mut cur).unwrap_or(0);
    let pc_career_output_sum = read_u64(b, &mut cur).unwrap_or(0) as i64;
    let pc_best_season_avg_output = read_i32(b, &mut cur).unwrap_or(0);
    let pc_seasons_played = read_u32(b, &mut cur).unwrap_or(0);
    let pc_decisive_moments = read_u32(b, &mut cur).unwrap_or(0);
    let pc_player_of_year_wins = read_u32(b, &mut cur).unwrap_or(0);
    let pc_league_titles = read_u32(b, &mut cur).unwrap_or(0);
    let pc_clubs_served = read_u32(b, &mut cur).unwrap_or(1);
    let pc_longest_club_tenure = read_u32(b, &mut cur).unwrap_or(0);
    let pc_sporting_rep = read_i32(b, &mut cur).unwrap_or(50);
    let pc_club_fan_rep = read_i32(b, &mut cur).unwrap_or(50);
    // Phase 8
    let pc_contract_seasons_left = read_u32(b, &mut cur).unwrap_or(2);
    let pc_wage_annual = read_u64(b, &mut cur).unwrap_or(20) as i64;
    let pc_power_ladder = read_u8(b, &mut cur).unwrap_or(0);
    let pc_savings = read_u64(b, &mut cur).unwrap_or(0) as i64;
    // Phase 9 peers
    let peer_blob_len = read_u32(b, &mut cur).unwrap_or(0) as usize;
    let peer_blob = if cur + peer_blob_len <= b.len() {
        let blob = b[cur..cur + peer_blob_len].to_vec();
        cur += peer_blob_len;
        blob
    } else {
        Vec::new()
    };
    let rival_raw = read_u8(b, &mut cur).unwrap_or(255);
    let pc_rival_idx = if rival_raw == 255 {
        None
    } else {
        Some(rival_raw)
    };
    let rival_season_raw = read_u32(b, &mut cur).unwrap_or(0);
    let pc_rival_declared_season = if rival_season_raw == 0 {
        None
    } else {
        Some(rival_season_raw)
    };
    // Phase 10
    let pc_lifestyle = read_u8(b, &mut cur).unwrap_or(1);
    let pc_retired = read_u8(b, &mut cur).unwrap_or(0) != 0;
    // Calendar (v5+; default false for older saves)
    let pc_week_training_done = read_u8(b, &mut cur).unwrap_or(0) != 0;
    // Calendar position (v6+; default 0 for older saves)
    let pc_epoch_day = read_u32(b, &mut cur).unwrap_or(0);
    // Phase 10 economy + life (v7+; defaults for older saves keep them neutral)
    let pc_business_value = read_u64(b, &mut cur).unwrap_or(0) as i64;
    let pc_bankrupt = read_u8(b, &mut cur).unwrap_or(0) != 0;
    let pc_dev_invest_level = read_u8(b, &mut cur).unwrap_or(0);
    let pc_marketability = read_i32(b, &mut cur).unwrap_or(50);
    let pc_sponsor_tier = read_u8(b, &mut cur).unwrap_or(0);
    let pc_relationships = [
        read_i32(b, &mut cur).unwrap_or(70),
        read_i32(b, &mut cur).unwrap_or(70),
        read_i32(b, &mut cur).unwrap_or(70),
    ];
    let pc_character_rep = read_i32(b, &mut cur).unwrap_or(50);
    // Lifestyle score (v8+; default 0 = Balanced for older saves).
    let pc_lifestyle_score = read_i32(b, &mut cur).unwrap_or(0);
    // Pantheon raw-signal evidence (v9+; default 0 for older saves).
    let pc_career_standout_matches = read_u32(b, &mut cur).unwrap_or(0);
    let pc_season_standout_matches = read_u32(b, &mut cur).unwrap_or(0);
    let pc_career_best_ovr = read_i32(b, &mut cur).unwrap_or(0);
    let pc_career_transfer_requests = read_u32(b, &mut cur).unwrap_or(0);
    let pc_season_transfer_requests = read_u32(b, &mut cur).unwrap_or(0);
    // National-team caps (v10+; default 0 for older saves).
    let pc_career_caps = read_u32(b, &mut cur).unwrap_or(0);
    let pc_season_caps = read_u32(b, &mut cur).unwrap_or(0);
    let pc_career_international_goals = read_u32(b, &mut cur).unwrap_or(0);
    let pc_season_international_goals = read_u32(b, &mut cur).unwrap_or(0);
    // Club economy (v12+; default empty for older saves).
    let club_budgets_len = read_u32(b, &mut cur).unwrap_or(0) as usize;
    let mut club_budgets = Vec::with_capacity(club_budgets_len.min(4096));
    for _ in 0..club_budgets_len {
        club_budgets.push(read_u64(b, &mut cur).unwrap_or(0) as i64);
    }
    // Academy investment (v13+; default empty for older saves).
    let academy_boosts_len = read_u32(b, &mut cur).unwrap_or(0) as usize;
    let mut academy_boosts = Vec::with_capacity(academy_boosts_len.min(4096));
    for _ in 0..academy_boosts_len {
        academy_boosts.push(read_u8(b, &mut cur).unwrap_or(0));
    }
    // Managers (v14+; default empty for older saves).
    let manager_blob_len = read_u32(b, &mut cur).unwrap_or(0) as usize;
    let manager_blob = if cur + manager_blob_len <= b.len() {
        let blob = b[cur..cur + manager_blob_len].to_vec();
        cur += manager_blob_len;
        blob
    } else {
        Vec::new()
    };
    let club_manager_len = read_u32(b, &mut cur).unwrap_or(0) as usize;
    let mut club_manager = Vec::with_capacity(club_manager_len.min(4096));
    for _ in 0..club_manager_len {
        club_manager.push(read_u32(b, &mut cur).unwrap_or(0));
    }
    let free_agents_len = read_u32(b, &mut cur).unwrap_or(0) as usize;
    let mut free_agents = Vec::with_capacity(free_agents_len.min(4096));
    for _ in 0..free_agents_len {
        free_agents.push(read_u32(b, &mut cur).unwrap_or(0));
    }
    // Goal/assist split (v15+; default 0 for older saves).
    let pc_season_assists = read_u32(b, &mut cur).unwrap_or(0);
    let pc_career_assists = read_u32(b, &mut cur).unwrap_or(0);
    // Decisive moments (v16+; default 0 for older saves).
    let pc_season_decisive_moments = read_u32(b, &mut cur).unwrap_or(0);
    // Clutch index (v17+; default 0 for older saves).
    let pc_season_clutch_index = read_u32(b, &mut cur).unwrap_or(0);
    let pc_career_clutch_index = read_u32(b, &mut cur).unwrap_or(0);
    // Live promotion/relegation membership (v18+; default empty = genesis-static).
    let pc_nation_membership_len = read_u32(b, &mut cur).unwrap_or(0) as usize;
    let mut pc_nation_membership = Vec::with_capacity(pc_nation_membership_len.min(4096));
    for _ in 0..pc_nation_membership_len {
        pc_nation_membership.push(read_u32(b, &mut cur).unwrap_or(0));
    }
    // Career base year (v19+; default 2025 for older saves — the old hardcoded
    // BASE_CAREER_YEAR, so pre-v19 saves keep their displayed dates).
    let career_base_year = read_u32(b, &mut cur).unwrap_or(2025);
    // Sim-behaviour version (v20+). The field only EXISTS in v20+ layouts: for older
    // layout tags the cursor isn't at the trailer, so we must not read there at all —
    // a pre-20 save's sim_version is definitionally 0 (unknown/legacy semantics).
    // The guard itself lives in `from_bytes` — this parser just surfaces the value.
    let sim_version = if ver >= 20 {
        read_u32(b, &mut cur).unwrap_or(0)
    } else {
        0
    };

    Ok((SaveData {
        world_seed,
        pc_name,
        pc_position,
        pc_nationality_idx,
        pc_club_idx,
        pc_div_idx,
        pc_current_attrs,
        pc_familiarity,
        pc_familiarity_xp,
        pc_age_weeks,
        pc_energy,
        pc_injury_weeks,
        routine_attrs,
        routine_intensity,
        season_number,
        season_round,
        pc_form,
        pc_season_goals,
        pc_season_matches,
        pc_season_output,
        table_raw,
        pc_yellow_cards_season,
        pc_suspensions,
        pc_discipline_rep,
        pc_career_goals,
        pc_career_matches,
        pc_career_output_sum,
        pc_best_season_avg_output,
        pc_seasons_played,
        pc_decisive_moments,
        pc_player_of_year_wins,
        pc_league_titles,
        pc_clubs_served,
        pc_longest_club_tenure,
        pc_sporting_rep,
        pc_club_fan_rep,
        pc_contract_seasons_left,
        pc_wage_annual,
        pc_power_ladder,
        pc_savings,
        peer_blob,
        pc_rival_idx,
        pc_rival_declared_season,
        pc_lifestyle,
        pc_retired,
        pc_week_training_done,
        pc_epoch_day,
        pc_business_value,
        pc_bankrupt,
        pc_dev_invest_level,
        pc_marketability,
        pc_sponsor_tier,
        pc_relationships,
        pc_character_rep,
        pc_lifestyle_score,
        pc_career_standout_matches,
        pc_season_standout_matches,
        pc_career_best_ovr,
        pc_career_transfer_requests,
        pc_season_transfer_requests,
        pc_career_caps,
        pc_season_caps,
        pc_career_international_goals,
        pc_season_international_goals,
        club_budgets,
        academy_boosts,
        manager_blob,
        club_manager,
        free_agents,
        pc_season_assists,
        pc_career_assists,
        pc_season_decisive_moments,
        pc_season_clutch_index,
        pc_career_clutch_index,
        pc_nation_membership,
        career_base_year,
    }, sim_version))
}

// ── Primitive helpers ─────────────────────────────────────────────────────────

fn push_u16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn push_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn push_u64(v: &mut Vec<u8>, x: u64) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn push_i32(v: &mut Vec<u8>, x: i32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn push_str(v: &mut Vec<u8>, s: &str) {
    push_u32(v, s.len() as u32);
    v.extend_from_slice(s.as_bytes());
}

fn read_u8(b: &[u8], cur: &mut usize) -> Result<u8, SaveError> {
    if *cur >= b.len() {
        return Err(SaveError::Corrupt("unexpected EOF reading u8"));
    }
    let v = b[*cur];
    *cur += 1;
    Ok(v)
}
fn read_u16(b: &[u8], cur: &mut usize) -> Result<u16, SaveError> {
    if *cur + 2 > b.len() {
        return Err(SaveError::Corrupt("unexpected EOF reading u16"));
    }
    let v = u16::from_le_bytes([b[*cur], b[*cur + 1]]);
    *cur += 2;
    Ok(v)
}
fn read_u32(b: &[u8], cur: &mut usize) -> Result<u32, SaveError> {
    if *cur + 4 > b.len() {
        return Err(SaveError::Corrupt("unexpected EOF reading u32"));
    }
    let v = u32::from_le_bytes(b[*cur..*cur + 4].try_into().unwrap());
    *cur += 4;
    Ok(v)
}
fn read_u64(b: &[u8], cur: &mut usize) -> Result<u64, SaveError> {
    if *cur + 8 > b.len() {
        return Err(SaveError::Corrupt("unexpected EOF reading u64"));
    }
    let v = u64::from_le_bytes(b[*cur..*cur + 8].try_into().unwrap());
    *cur += 8;
    Ok(v)
}
fn read_i32(b: &[u8], cur: &mut usize) -> Result<i32, SaveError> {
    read_u32(b, cur).map(|v| v as i32)
}
fn read_str(b: &[u8], cur: &mut usize) -> Result<String, SaveError> {
    let len = read_u32(b, cur)? as usize;
    if *cur + len > b.len() {
        return Err(SaveError::Corrupt("string too long"));
    }
    let s = std::str::from_utf8(&b[*cur..*cur + len])
        .map_err(|_| SaveError::Corrupt("invalid UTF-8 in string"))?
        .to_string();
    *cur += len;
    Ok(s)
}
