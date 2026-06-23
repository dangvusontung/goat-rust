//! Struct-of-arrays player storage.
//!
//! World players are columnar data (parallel `Vec`s), not heap objects per player.
//! Player identity is an index into those columns. A `PlayerView` provides an
//! ergonomic per-player snapshot without changing the storage model.

use crate::attrs::NUM_ATTRS;
use crate::positions::PrimaryPosition;
use crate::roles::{FamiliarityTier, NUM_ROLES};
use goat_fixed::Fixed;

/// Opaque player identity — an index into `PlayerStore` columns.
pub type PlayerId = usize;

/// Struct-of-arrays player store.
///
/// All attribute values are stored in column-major order so that whole-population
/// scans (phase 9 tiered sim) are cache-friendly.
#[derive(Debug, Clone)]
pub struct PlayerStore {
    // ── Phase 1 columns ──────────────────────────────────────────────────────
    /// Current attribute values, indexed `[attr_idx][player_id]`.
    pub current: [Vec<Fixed>; NUM_ATTRS],
    /// Potential (ceiling) attribute values, indexed `[attr_idx][player_id]`.
    pub potential: [Vec<Fixed>; NUM_ATTRS],
    /// Familiarity tier per role, indexed `[role_idx][player_id]`.
    pub familiarity: [Vec<FamiliarityTier>; NUM_ROLES],
    /// Player names — kept separate; not Fixed.
    pub names: Vec<String>,
    /// Primary position for OVR (appendix C.4) — migrates with reinvention.
    pub primary_positions: Vec<PrimaryPosition>,
    // ── Phase 3 columns ──────────────────────────────────────────────────────
    /// Current age in weeks (`[player_id]`).
    pub age_weeks: Vec<u32>,
    /// Current energy level 0–100 (`[player_id]`).
    pub energy: Vec<Fixed>,
    /// Weeks of injury remaining; 0 = healthy (`[player_id]`).
    pub injury_weeks: Vec<u32>,
    /// Accumulated XP toward the next familiarity-tier upgrade,
    /// indexed `[role_idx][player_id]`. Resets to 0 on each upgrade.
    pub familiarity_xp: [Vec<Fixed>; NUM_ROLES],
}

impl PlayerStore {
    /// Create an empty store. Use `push` to add players.
    pub fn new() -> Self {
        Self {
            current: core::array::from_fn(|_| Vec::new()),
            potential: core::array::from_fn(|_| Vec::new()),
            familiarity: core::array::from_fn(|_| Vec::new()),
            names: Vec::new(),
            primary_positions: Vec::new(),
            age_weeks: Vec::new(),
            energy: Vec::new(),
            injury_weeks: Vec::new(),
            familiarity_xp: core::array::from_fn(|_| Vec::new()),
        }
    }

    /// Number of players currently stored.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// Append a player and return their `PlayerId`.
    pub fn push(&mut self, view: PlayerView) -> PlayerId {
        let id = self.names.len();
        self.names.push(view.name);
        for a in 0..NUM_ATTRS {
            self.current[a].push(view.current[a]);
            self.potential[a].push(view.potential[a]);
        }
        for r in 0..NUM_ROLES {
            self.familiarity[r].push(view.familiarity[r]);
            self.familiarity_xp[r].push(view.familiarity_xp[r]);
        }
        self.primary_positions.push(view.primary_position);
        self.age_weeks.push(view.age_weeks);
        self.energy.push(view.energy);
        self.injury_weeks.push(view.injury_weeks);
        id
    }

    // ── Attribute accessors ───────────────────────────────────────────────────

    pub fn get_current(&self, player: PlayerId, attr: usize) -> Fixed {
        self.current[attr][player]
    }

    pub fn get_potential(&self, player: PlayerId, attr: usize) -> Fixed {
        self.potential[attr][player]
    }

    /// Write the current value — enforces the talent-ceiling invariant.
    ///
    /// Panics in debug builds if `value > potential[attr][player]`.
    pub fn set_current(&mut self, player: PlayerId, attr: usize, value: Fixed) {
        debug_assert!(
            value <= self.potential[attr][player],
            "talent ceiling violated: current > potential for attr {attr}"
        );
        self.current[attr][player] = value;
    }

    // ── Familiarity accessors ─────────────────────────────────────────────────

    pub fn get_familiarity(&self, player: PlayerId, role: usize) -> FamiliarityTier {
        self.familiarity[role][player]
    }

    pub fn set_familiarity(&mut self, player: PlayerId, role: usize, tier: FamiliarityTier) {
        self.familiarity[role][player] = tier;
    }

    pub fn get_familiarity_xp(&self, player: PlayerId, role: usize) -> Fixed {
        self.familiarity_xp[role][player]
    }

    pub fn set_familiarity_xp(&mut self, player: PlayerId, role: usize, xp: Fixed) {
        self.familiarity_xp[role][player] = xp;
    }

    // ── Phase 3 accessors ─────────────────────────────────────────────────────

    pub fn get_age_weeks(&self, player: PlayerId) -> u32 {
        self.age_weeks[player]
    }

    pub fn set_age_weeks(&mut self, player: PlayerId, weeks: u32) {
        self.age_weeks[player] = weeks;
    }

    pub fn get_energy(&self, player: PlayerId) -> Fixed {
        self.energy[player]
    }

    pub fn set_energy(&mut self, player: PlayerId, energy: Fixed) {
        self.energy[player] = energy;
    }

    pub fn get_injury_weeks(&self, player: PlayerId) -> u32 {
        self.injury_weeks[player]
    }

    pub fn set_injury_weeks(&mut self, player: PlayerId, weeks: u32) {
        self.injury_weeks[player] = weeks;
    }

    /// Produce an owned ergonomic snapshot of a single player.
    pub fn snapshot(&self, player: PlayerId) -> PlayerView {
        let mut current = [Fixed::ZERO; NUM_ATTRS];
        let mut potential = [Fixed::ZERO; NUM_ATTRS];
        let mut familiarity = [FamiliarityTier::Awkward; NUM_ROLES];
        let mut familiarity_xp = [Fixed::ZERO; NUM_ROLES];
        for (a, cur) in current.iter_mut().enumerate() {
            *cur = self.current[a][player];
        }
        for (a, pot) in potential.iter_mut().enumerate() {
            *pot = self.potential[a][player];
        }
        for (r, fam) in familiarity.iter_mut().enumerate() {
            *fam = self.familiarity[r][player];
        }
        for (r, xp) in familiarity_xp.iter_mut().enumerate() {
            *xp = self.familiarity_xp[r][player];
        }
        PlayerView {
            name: self.names[player].clone(),
            current,
            potential,
            familiarity,
            familiarity_xp,
            primary_position: self.primary_positions[player],
            age_weeks: self.age_weeks[player],
            energy: self.energy[player],
            injury_weeks: self.injury_weeks[player],
        }
    }

    // ── Primary-position accessors ────────────────────────────────────────────

    pub fn get_primary_position(&self, player: PlayerId) -> PrimaryPosition {
        self.primary_positions[player]
    }

    pub fn set_primary_position(&mut self, player: PlayerId, pos: PrimaryPosition) {
        self.primary_positions[player] = pos;
    }
}

impl Default for PlayerStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Ergonomic single-player snapshot.
///
/// Not the canonical storage — convenient for generation output, TUI rendering,
/// and tests that construct players inline.
#[derive(Debug, Clone)]
pub struct PlayerView {
    pub name: String,
    pub current: [Fixed; NUM_ATTRS],
    pub potential: [Fixed; NUM_ATTRS],
    pub familiarity: [FamiliarityTier; NUM_ROLES],
    /// Primary position for OVR (appendix C.4). Defaults to ST.
    pub primary_position: PrimaryPosition,
    // Phase 3 fields
    pub familiarity_xp: [Fixed; NUM_ROLES],
    pub age_weeks: u32,
    pub energy: Fixed,
    pub injury_weeks: u32,
}

impl PlayerView {
    /// Build a view with sensible defaults (age 16, full energy, all Awkward).
    pub fn zeroed(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }
}

impl Default for PlayerView {
    fn default() -> Self {
        Self {
            name: String::new(),
            current: [Fixed::MIN_ATTR; NUM_ATTRS],
            potential: [Fixed::MIN_ATTR; NUM_ATTRS],
            familiarity: [FamiliarityTier::Awkward; NUM_ROLES],
            primary_position: PrimaryPosition::ST,
            familiarity_xp: [Fixed::ZERO; NUM_ROLES],
            age_weeks: 16 * 52, // 832
            energy: Fixed::raw(90_000),
            injury_weeks: 0,
        }
    }
}
