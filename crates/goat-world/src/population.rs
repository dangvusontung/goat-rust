//! Struct-of-arrays background population + deterministic genesis (TASK-09A Slice 9A.1).
//!
//! The outer world is a columnar population (parallel `Vec`s keyed by an index), never
//! per-player heap objects (bible §9 / SoA discipline). Genesis stores only the cheap
//! *identity* columns plus a per-player seed; a background player's full attributes are
//! recomputed on demand from `(seed + birth data + date)` in Slice 9A.2 — they are never
//! stored or stepped weekly (the §9 SoA/perf trap). Same `world_seed` ⇒ bit-for-bit the
//! same universe on every platform: that is the Phase 9 determinism spine, pinned by the
//! `fingerprint` golden.

use crate::world::WorldGenesis;
use goat_core::attrs::NUM_ATTRS;
use goat_core::generation::{generate_player, CreationChoices};
use goat_core::player::PlayerView;
use goat_core::positions::PrimaryPosition;
use goat_fixed::Fixed;
use goat_rng::{GoatRng, RngSource};

/// Age (years) at which a background player retires; past it, lazy-promote refuses so a
/// retired identity can never re-enter the live world as an active player.
pub const RETIRE_AGE_YEARS: u32 = 38;

/// Players generated per club at genesis. Total headcount = `world.clubs.len() * SQUAD_SIZE`
/// — scales directly with the generated club/nation count (~1,200 clubs → 30,000 players,
/// the bible §7.2/§9 population target).
pub const SQUAD_SIZE: usize = 25;

/// Background population as parallel columns. Index `i` identifies one player across all
/// columns — there is no per-player struct.
#[derive(Debug, Clone, Default)]
pub struct Population {
    /// Per-player deterministic seed; everything derivable is recomputed from this.
    pub seed: Vec<u64>,
    /// Club index into `CLUBS`.
    pub club: Vec<u16>,
    /// Nationality (Nation as u8).
    pub nation: Vec<u8>,
    /// Primary position: 0 = Defender, 1 = Midfielder, 2 = Forward.
    pub position: Vec<u8>,
    /// Age in weeks at genesis (birth data is the stored residue; age advances by date).
    pub birth_age_weeks: Vec<u32>,
    /// Headline potential OVR (1–99). Cached identity column; the per-attribute potential
    /// is re-derivable from `seed`.
    pub potential_ovr: Vec<u8>,
    // ── Path-dependent accumulators (batch-tick residue; bible §247) ──────────
    /// Career goals accumulated by season batch-tick. Not derivable — persisted.
    pub career_goals: Vec<u32>,
    /// Career league appearances accumulated by batch-tick.
    pub career_apps: Vec<u32>,
    /// League titles won (player's club finished top of its division that season).
    pub career_titles: Vec<u32>,
}

impl Population {
    pub fn len(&self) -> usize {
        self.seed.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seed.is_empty()
    }

    /// Deterministic FNV-1a fingerprint over every identity column, in the fixed genesis
    /// order (insertion order is itself deterministic, so no sort is needed). Same seed ⇒
    /// same fingerprint on every platform — the spine golden for Phase 9 determinism.
    pub fn fingerprint(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for i in 0..self.len() {
            for x in [
                self.seed[i],
                self.club[i] as u64,
                self.nation[i] as u64,
                self.position[i] as u64,
                self.birth_age_weeks[i] as u64,
                self.potential_ovr[i] as u64,
            ] {
                h ^= x;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        h
    }

    /// Fingerprint over the path-dependent career accumulators (goals/apps/titles).
    /// Stable for a fixed seed + batch-tick sequence — the golden anchor for 9A.3.
    pub fn career_fingerprint(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for i in 0..self.len() {
            for x in [
                self.career_goals[i] as u64,
                self.career_apps[i] as u64,
                self.career_titles[i] as u64,
            ] {
                h ^= x;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        h
    }
}

/// Squad position spread across a 25-man squad: roughly a third each of D / M / F.
fn squad_position(slot: usize) -> u8 {
    (slot % 3) as u8
}

/// Combine the world seed with club + slot into a stable per-player seed. `GoatRng::new`
/// whitens it, so this only needs to be collision-resistant across (club, slot).
fn player_seed(world_seed: u64, club_id: u64, slot: u64) -> u64 {
    world_seed
        ^ club_id.rotate_left(21).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ slot.rotate_left(43).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
}

/// Generate the background population deterministically from `world_seed`. Every club
/// gets a `SQUAD_SIZE` squad; potential is anchored to club stature (stronger clubs draw
/// stronger players) with per-player variance. Pure and order-stable.
pub fn genesis(world_seed: u64, world: &WorldGenesis) -> Population {
    let mut pop = Population::default();
    pop.seed.reserve(world.clubs.len() * SQUAD_SIZE);

    for club in &world.clubs {
        let club_id = club.id;
        for slot in 0..SQUAD_SIZE {
            let pseed = player_seed(world_seed, club_id as u64, slot as u64);
            let mut rng = GoatRng::new(pseed);

            let position = squad_position(slot);
            let age_years = rng.next_range_u32(16, 33);
            let birth_age_weeks = age_years * 52;

            // Potential anchored to club strength ± variance, clamped to a sane band.
            let base = club.strength as i32;
            let variance = rng.next_range_u32(0, 30) as i32 - 15;
            let potential_ovr = (base + variance).clamp(30, 99) as u8;

            pop.seed.push(pseed);
            pop.club.push(club_id as u16);
            pop.nation.push(club.nation as u8);
            pop.position.push(position);
            pop.birth_age_weeks.push(birth_age_weeks);
            pop.potential_ovr.push(potential_ovr);
            pop.career_goals.push(0);
            pop.career_apps.push(0);
            pop.career_titles.push(0);
        }
    }

    pop
}

// ── Formula-driven background growth + lazy-promote (bible §245–246) ───────────

/// Closed-form development curve: the fraction of potential a player has realised at a
/// given age (×1000 fixed-point, i.e. raw 1000 = 1.0). Rises from 0.60 at 16 to a 25–31
/// peak (1.00), then declines. Pure — background growth is *computed on demand* from age,
/// never stored or stepped weekly (the §9 SoA/perf trap).
fn development_fraction(age_years: u32) -> Fixed {
    let pct: u32 = match age_years {
        0..=16 => 600,
        17..=25 => 600 + (age_years - 16) * 45, // 17→645 … 25→1005 (capped to 1.0)
        26..=31 => 1000,                        // peak plateau
        32..=37 => 1000 - (age_years - 31) * 35, // 32→965 … 37→790
        _ => 770,
    };
    Fixed::raw(pct.min(1000) as i32)
}

/// Maps the background population's family-level column (0=Defender,1=Midfielder,
/// 2=Forward) to the same specific position the old 3-way creation picker's
/// `default_primary()` produced, so genesis stays byte-identical.
fn position_from_u8(p: u8) -> PrimaryPosition {
    match p {
        0 => PrimaryPosition::CB,
        1 => PrimaryPosition::CM,
        _ => PrimaryPosition::ST,
    }
}

impl Population {
    /// Age in years of background player `idx` at `elapsed_weeks` after genesis.
    fn age_years_at(&self, idx: usize, elapsed_weeks: u32) -> u32 {
        (self.birth_age_weeks[idx] + elapsed_weeks) / 52
    }

    /// Cheap O(1) current OVR of a background player at a date (epoch weeks since genesis),
    /// derived on demand from `(potential, age)`. Never exceeds the stored potential
    /// (§2.4). Used for outer-world ranking without realising the full player.
    pub fn current_ovr(&self, idx: usize, elapsed_weeks: u32) -> u8 {
        let frac = development_fraction(self.age_years_at(idx, elapsed_weeks));
        let cur = (Fixed::from_int(self.potential_ovr[idx] as i32) * frac).to_int();
        cur.clamp(0, self.potential_ovr[idx] as i32) as u8
    }

    /// True once the player has reached the retirement age at the given date.
    pub fn is_retired(&self, idx: usize, elapsed_weeks: u32) -> bool {
        self.age_years_at(idx, elapsed_weeks) >= RETIRE_AGE_YEARS
    }

    /// Lazy-promote a background player into a full-fidelity `PlayerView` "on contact"
    /// (bible §245) — the moment he becomes relevant (you face him, a transfer links him).
    /// Returns `None` if he has retired. Pure & deterministic: same `(idx, date)` ⇒ the
    /// same player on every run/platform. The caller pushes the view into a `PlayerStore`.
    pub fn promote(
        &self,
        idx: usize,
        elapsed_weeks: u32,
        name: impl Into<String>,
        world: &WorldGenesis,
    ) -> Option<PlayerView> {
        if self.is_retired(idx, elapsed_weeks) {
            return None;
        }
        let choices = CreationChoices {
            name: name.into(),
            primary_position: position_from_u8(self.position[idx]),
            nationality: world.nation_name(self.nation[idx] as usize).to_string(),
            club: world.clubs[self.club[idx] as usize].name.clone(),
        };
        // generate_player gives the realistic per-attribute potential + shape + roles; we
        // overwrite current to the age-appropriate fraction of that potential.
        let mut view = generate_player(self.seed[idx], &choices);
        let frac = development_fraction(self.age_years_at(idx, elapsed_weeks));
        for a in 0..NUM_ATTRS {
            view.current[a] = (view.potential[a] * frac).clamp(Fixed::MIN_ATTR, view.potential[a]);
        }
        view.age_weeks = self.birth_age_weeks[idx] + elapsed_weeks;
        Some(view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::WorldGenesis;

    #[test]
    fn genesis_is_full_and_columnar() {
        let world = WorldGenesis::generate(7);
        let pop = genesis(7, &world);
        let expected = world.clubs.len() * SQUAD_SIZE;
        assert_eq!(pop.len(), expected);
        // All columns are the same length (true SoA — no ragged rows).
        assert_eq!(pop.club.len(), expected);
        assert_eq!(pop.potential_ovr.len(), expected);
        // Invariants on derived columns.
        assert!(pop.position.iter().all(|&p| p <= 2));
        assert!(pop.potential_ovr.iter().all(|&o| (30..=99).contains(&o)));
        assert!(pop
            .birth_age_weeks
            .iter()
            .all(|&w| (16 * 52..=33 * 52).contains(&w)));
    }

    #[test]
    fn genesis_is_deterministic() {
        let world = WorldGenesis::generate(42);
        assert_eq!(genesis(42, &world).fingerprint(), genesis(42, &world).fingerprint());
        let world2 = WorldGenesis::generate(2);
        assert_ne!(genesis(1, &world).fingerprint(), genesis(2, &world2).fingerprint());
    }

    #[test]
    fn background_current_never_exceeds_potential() {
        let world = WorldGenesis::generate(7);
        let pop = genesis(7, &world);
        // Sweep every player across a 24-year span of dates.
        for idx in 0..pop.len() {
            for wk in (0..24 * 52).step_by(26) {
                assert!(
                    pop.current_ovr(idx, wk) <= pop.potential_ovr[idx],
                    "idx {idx}: current > potential at week {wk}"
                );
            }
        }
    }

    #[test]
    fn background_rederive_is_deterministic() {
        let world = WorldGenesis::generate(3);
        let pop = genesis(3, &world);
        assert_eq!(pop.current_ovr(100, 260), pop.current_ovr(100, 260));
        let a = pop.promote(50, 6 * 52, "X", &world).unwrap();
        let b = pop.promote(50, 6 * 52, "X", &world).unwrap();
        assert_eq!(a.current, b.current, "promote must be deterministic");
    }

    #[test]
    fn promoted_player_respects_talent_ceiling() {
        let world = WorldGenesis::generate(9);
        let pop = genesis(9, &world);
        let view = pop.promote(50, 8 * 52, "Prospect", &world).unwrap();
        for i in 0..NUM_ATTRS {
            assert!(
                view.current[i] <= view.potential[i],
                "attr {i} exceeds potential"
            );
        }
    }

    #[test]
    fn lazy_promote_never_resurrects_retired() {
        let world = WorldGenesis::generate(11);
        let pop = genesis(11, &world);
        let idx = 0;
        // Elapsed time that puts this player exactly at the retirement age.
        let elapsed = RETIRE_AGE_YEARS * 52 - pop.birth_age_weeks[idx];
        assert!(pop.is_retired(idx, elapsed));
        assert!(
            pop.promote(idx, elapsed, "Veteran", &world).is_none(),
            "a retired player must never promote to an active view"
        );
    }
}
