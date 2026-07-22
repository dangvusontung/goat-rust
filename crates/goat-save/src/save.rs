//! Binary save format — big-endian, fixed-size fields, no external deps.
//!
//! Magic: b"GOAT" (4 bytes)
//! Version: u32 (4 bytes)
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
pub const VERSION: u32 = 12;

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
}

#[derive(Debug)]
pub enum SaveError {
    Io(io::Error),
    BadMagic,
    BadVersion(u32),
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
    }
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

    state
}

// ── Serialisation ─────────────────────────────────────────────────────────────

fn to_bytes(d: &SaveData) -> Vec<u8> {
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
    v
}

fn from_bytes(b: &[u8]) -> Result<SaveData, SaveError> {
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

    Ok(SaveData {
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
    })
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
