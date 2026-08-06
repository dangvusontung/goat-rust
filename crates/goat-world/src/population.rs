//! Struct-of-arrays background population + deterministic genesis (TASK-09A Slice 9A.1).
//!
//! The outer world is a columnar population (parallel `Vec`s keyed by an index), never
//! per-player heap objects (bible §9 / SoA discipline). Genesis stores only the cheap
//! *identity* columns plus a per-player seed; a background player's full attributes are
//! recomputed on demand from `(seed + birth data + date)` in Slice 9A.2 — they are never
//! stored or stepped weekly (the §9 SoA/perf trap). Same `world_seed` ⇒ bit-for-bit the
//! same universe on every platform: that is the Phase 9 determinism spine, pinned by the
//! `fingerprint` golden.

use crate::world::{ClubId, WorldGenesis};
use goat_core::attrs::NUM_ATTRS;
use goat_core::generation::{generate_player_biased, CreationChoices};
use goat_core::player::PlayerView;
use goat_core::positions::PrimaryPosition;
use goat_fixed::Fixed;
use goat_rng::{GoatRng, RngSource};

/// Age (years) at which a background player retires; past it, lazy-promote refuses so a
/// retired identity can never re-enter the live world as an active player.
pub const RETIRE_AGE_YEARS: u32 = 38;

/// Chance (out of 100) that a genesis/intake player's potential ignores `club.strength`
/// entirely and rolls from the full valid band instead (Design round 3, Doc C §Slice 2).
/// Tùng's own example figure, adopted directly — a first-pass constant, not re-derived.
const OUTLIER_CHANCE_PCT: u32 = 2;
/// Lower bound of the valid `potential_ovr` band (both the anchor clamp and the outlier
/// roll's own full range reuse this — one pair of bounds, not two).
const POTENTIAL_MIN: u8 = 30;
/// Upper bound of the valid `potential_ovr` band.
const POTENTIAL_MAX: u8 = 99;

/// Roll a background player's headline `potential_ovr`: usually anchored to `club_strength`
/// ± variance (the pre-existing formula), but with a small (`OUTLIER_CHANCE_PCT`) chance of
/// ignoring the club anchor entirely and rolling uniformly across the full band — the
/// "unearthed at a nobody club" outlier (Design round 3, Doc C §Slice 2). Shared by
/// `genesis` and Slice 4's youth intake — one formula, not duplicated.
fn roll_potential_ovr(rng: &mut GoatRng, club_strength: u8) -> u8 {
    if rng.next_range_u32(0, 99) < OUTLIER_CHANCE_PCT {
        return rng.next_range_u32(POTENTIAL_MIN as u32, POTENTIAL_MAX as u32) as u8;
    }
    let base = club_strength as i32;
    let variance = rng.next_range_u32(0, 30) as i32 - 15;
    (base + variance).clamp(POTENTIAL_MIN as i32, POTENTIAL_MAX as i32) as u8
}

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
    /// Elapsed weeks (since world genesis) at which this player entered the population.
    /// `0` for every genesis-created player (byte-identical to pre-Slice-4 behaviour); a
    /// Slice-4 youth-intake player's is `season * 52`. Needed because `birth_age_weeks`
    /// alone assumes an entry point at `elapsed_weeks = 0` — see `age_years_at`.
    pub intake_week: Vec<u32>,
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
                self.intake_week[i] as u64,
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
/// gets a `club.squad_size` squad; potential is anchored to club stature (stronger clubs
/// draw stronger players) with per-player variance. Pure and order-stable.
pub fn genesis(world_seed: u64, world: &WorldGenesis) -> Population {
    let mut pop = Population::default();
    pop.seed.reserve(
        world
            .clubs
            .iter()
            .map(|c| c.squad_size as usize)
            .sum::<usize>(),
    );

    for club in &world.clubs {
        let club_id = club.id;
        for slot in 0..club.squad_size as usize {
            let pseed = player_seed(world_seed, club_id as u64, slot as u64);
            let mut rng = GoatRng::new(pseed);

            let position = squad_position(slot);
            let age_years = rng.next_range_u32(16, 33);
            let birth_age_weeks = age_years * 52;

            // Potential anchored to club strength ± variance, with a rare outlier roll.
            let potential_ovr = roll_potential_ovr(&mut rng, club.strength);

            pop.seed.push(pseed);
            pop.club.push(club_id as u16);
            pop.nation.push(club.nation as u8);
            pop.position.push(position);
            pop.birth_age_weeks.push(birth_age_weeks);
            pop.potential_ovr.push(potential_ovr);
            pop.intake_week.push(0);
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
    ///
    /// `pub(crate)`, not private: Round 5 Slice 3-4's `scouting` module (a sibling in this
    /// crate) reads this directly for its cheap SoA target-search scan.
    pub(crate) fn age_years_at(&self, idx: usize, elapsed_weeks: u32) -> u32 {
        let weeks_since_intake = elapsed_weeks.saturating_sub(self.intake_week[idx]);
        (self.birth_age_weeks[idx] + weeks_since_intake) / 52
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

    /// Live team strength (1-99): mean current OVR of a club's non-retired squad at
    /// `elapsed_weeks`. O(pop.len()) — a linear scan filtered by club id; fine for an
    /// occasional single-club query (e.g. a UI "opponent strength" lookup), but callers
    /// simulating every club in one pass (batch-tick) should keep using a precomputed
    /// squads-by-club grouping via `live_strength_from_squad`, not call this once per club.
    pub fn live_strength(&self, club_id: ClubId, elapsed_weeks: u32) -> u8 {
        let squad: Vec<usize> = (0..self.len())
            .filter(|&i| self.club[i] as usize == club_id && !self.is_retired(i, elapsed_weeks))
            .collect();
        self.live_strength_from_squad(&squad, elapsed_weeks)
    }

    /// Same formula, given a precomputed squad (the batch-tick bulk path). Both
    /// `live_strength` and `batch_tick::batch_tick_season` route through this — one
    /// formula, not two. Excludes retired players: a club's live strength should reflect
    /// only players who'd actually turn out for it (the "Verified" §3.2 correctness fix —
    /// the pre-existing `batch_tick.rs::club_strength` this replaces did not filter
    /// retirement, so a retired player's still-evaluated `current_ovr` curve kept dragging
    /// on a club's live strength after the squad member could no longer actually play).
    pub fn live_strength_from_squad(&self, squad: &[usize], elapsed_weeks: u32) -> u8 {
        let active: Vec<usize> = squad
            .iter()
            .copied()
            .filter(|&i| !self.is_retired(i, elapsed_weeks))
            .collect();
        if active.is_empty() {
            return 1;
        }
        let sum: u32 = active
            .iter()
            .map(|&i| self.current_ovr(i, elapsed_weeks) as u32)
            .sum();
        (sum / active.len() as u32).clamp(1, 99) as u8
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
        let club = &world.clubs[self.club[idx] as usize];
        let choices = CreationChoices {
            name: name.into(),
            primary_position: position_from_u8(self.position[idx]),
            nationality: world.nation_name(self.nation[idx] as usize).to_string(),
            club: club.name.clone(),
        };
        // generate_player_biased gives the realistic per-attribute potential + shape +
        // roles, nudged by the club's tactical identity (Design round 3, Doc C §Slice 5);
        // we overwrite current to the age-appropriate fraction of that potential.
        let mut view =
            generate_player_biased(self.seed[idx], &choices, Some(&club.tactical_identity));
        let frac = development_fraction(self.age_years_at(idx, elapsed_weeks));
        for a in 0..NUM_ATTRS {
            view.current[a] = (view.potential[a] * frac).clamp(Fixed::MIN_ATTR, view.potential[a]);
        }
        let weeks_since_intake = elapsed_weeks.saturating_sub(self.intake_week[idx]);
        view.age_weeks = self.birth_age_weeks[idx] + weeks_since_intake;
        Some(view)
    }
}

/// Age (years) an intake player enters the population at — mirrors a real academy
/// graduate's age, and is the age `age_years_at` must report exactly at
/// `elapsed_weeks == intake_week` (the 4.4 correctness fix's own regression target).
const INTAKE_AGE_YEARS: u32 = 16;

/// Deterministic seed for a Slice-4 intake player, per `(world_seed, club_id, season,
/// local_idx)` — mirrors `player_seed`'s "generated but consistent" pattern with a season
/// term folded in.
fn intake_player_seed(world_seed: u64, club_id: u64, season: u32, local_idx: u64) -> u64 {
    world_seed
        ^ club_id.rotate_left(21).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (season as u64)
            .rotate_left(31)
            .wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ local_idx
            .rotate_left(11)
            .wrapping_mul(0x1656_67B1_9E37_79F9)
}

/// Deterministic seed for a club's season-level "how many intake players this season"
/// roll — same seed family as `intake_player_seed`, but scoped to `(club_id, season)` only
/// (no `local_idx` term), so this count roll's RNG stream never entangles with any one
/// intake player's own identity seed.
fn intake_count_seed(world_seed: u64, club_id: u64, season: u32) -> u64 {
    world_seed
        ^ club_id.rotate_left(21).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (season as u64)
            .rotate_left(31)
            .wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ 0x494E_5441_4B45 // "INTAKE" salt — keeps this stream distinct from per-player seeds
}

/// Season-end youth academy replenishment for every club (Design round 3, Doc C §Slice
/// 4). Appends new SoA rows to `pop` in place — never removes or reorders existing rows;
/// retirement stays purely virtual, per `is_retired`. Rolls an independent uniform 1–4
/// intake count per club per season (no arithmetic tie to that season's actual retirement
/// count — Tùng explicitly rejected a rigid 1-in-1-out mechanic as unrealistic), skipped
/// entirely for a club whose active squad is already at/above `squad_size * 1.2`. Returns
/// the total number of players added, for logging/telemetry.
pub fn apply_youth_intake(
    pop: &mut Population,
    world: &WorldGenesis,
    world_seed: u64,
    season: u32,
) -> u32 {
    let elapsed_weeks = season * 52;
    let mut total_added = 0u32;

    for club in &world.clubs {
        let target = club.squad_size as u32;
        let ceiling = target + target / 5; // +20%
        let active = (0..pop.len())
            .filter(|&i| pop.club[i] as usize == club.id && !pop.is_retired(i, elapsed_weeks))
            .count() as u32;
        if active >= ceiling {
            continue;
        }

        let mut count_rng = GoatRng::new(intake_count_seed(world_seed, club.id as u64, season));
        let intake_count = count_rng.next_range_u32(1, 4);

        for local_idx in 0..intake_count {
            let pseed = intake_player_seed(world_seed, club.id as u64, season, local_idx as u64);
            let mut rng = GoatRng::new(pseed);

            let position = squad_position(local_idx as usize);
            // Slice 6's one call-site ripple into round-3's existing intake formula: an
            // academy-boosted club rolls potential against a higher anchor, no change to
            // `roll_potential_ovr`'s own signature or the outlier mechanism.
            let effective_strength = club.strength.saturating_add(club.academy_boost).min(99);
            let potential_ovr = roll_potential_ovr(&mut rng, effective_strength);

            pop.seed.push(pseed);
            pop.club.push(club.id as u16);
            pop.nation.push(club.nation as u8);
            pop.position.push(position);
            pop.birth_age_weeks.push(INTAKE_AGE_YEARS * 52);
            pop.potential_ovr.push(potential_ovr);
            pop.intake_week.push(elapsed_weeks);
            pop.career_goals.push(0);
            pop.career_apps.push(0);
            pop.career_titles.push(0);
            total_added += 1;
        }
    }

    total_added
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::WorldGenesis;

    #[test]
    fn genesis_is_full_and_columnar() {
        let world = WorldGenesis::generate(7);
        let pop = genesis(7, &world);
        let expected: usize = world.clubs.iter().map(|c| c.squad_size as usize).sum();
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
    fn genesis_headcount_matches_sum_of_squad_sizes() {
        let world = WorldGenesis::generate(23);
        let pop = genesis(23, &world);
        let expected: usize = world.clubs.iter().map(|c| c.squad_size as usize).sum();
        assert_eq!(pop.len(), expected);
    }

    #[test]
    fn genesis_is_deterministic() {
        let world = WorldGenesis::generate(42);
        assert_eq!(
            genesis(42, &world).fingerprint(),
            genesis(42, &world).fingerprint()
        );
        let world2 = WorldGenesis::generate(2);
        assert_ne!(
            genesis(1, &world).fingerprint(),
            genesis(2, &world2).fingerprint()
        );
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

    #[test]
    fn outlier_roll_breaks_the_weak_club_ceiling() {
        // strength <= 14 => the anchor branch alone always floors to exactly 30 (see the
        // TDD anchor for "Verified" ceiling math). Search for a seed whose outlier roll
        // breaks that ceiling.
        let weak_strength: u8 = 1;
        let found = (0u64..10_000).find_map(|seed| {
            let mut rng = GoatRng::new(seed);
            let v = roll_potential_ovr(&mut rng, weak_strength);
            (v > 30).then_some(v)
        });
        assert!(
            found.is_some(),
            "expected at least one outlier roll to exceed the flat-30 ceiling for a strength=1 club"
        );
    }

    #[test]
    fn outlier_rate_is_roughly_two_percent() {
        // For a weak club (strength <= 14) the anchor branch is deterministically exactly
        // 30 every time; any other value can only come from the outlier branch. Counting
        // "not exactly 30" over a large sample directly measures the outlier rate.
        let weak_strength: u8 = 5;
        let mut rng = GoatRng::new(0x00C0_FFEE);
        let n = 100_000;
        let outliers = (0..n)
            .filter(|_| roll_potential_ovr(&mut rng, weak_strength) != 30)
            .count();
        let rate_pct = outliers as f64 / n as f64 * 100.0;
        assert!(
            (0.5..4.0).contains(&rate_pct),
            "outlier rate {rate_pct}% should be roughly {OUTLIER_CHANCE_PCT}% (wide statistical tolerance)"
        );
    }

    #[test]
    fn anchor_branch_unchanged_when_no_outlier() {
        let strength: u8 = 40;
        for seed in 0u64..500 {
            // Replay the same rng stream independently to compute what the pre-Slice-2
            // anchor-only formula would have produced from the same draws.
            let mut probe = GoatRng::new(seed);
            let check = probe.next_range_u32(0, 99);
            if check < OUTLIER_CHANCE_PCT {
                continue; // this seed hits the outlier branch; not covered by this test
            }
            let variance = probe.next_range_u32(0, 30) as i32 - 15;
            let expected = (strength as i32 + variance).clamp(30, 99) as u8;

            let mut rng = GoatRng::new(seed);
            let actual = roll_potential_ovr(&mut rng, strength);
            assert_eq!(
                actual, expected,
                "seed {seed}: anchor branch must match the pre-Slice-2 formula bit-for-bit"
            );
        }
    }

    #[test]
    fn live_strength_matches_live_strength_from_squad() {
        let world = WorldGenesis::generate(31);
        let pop = genesis(31, &world);
        let club_id = pop.club[0] as usize;
        let elapsed = 5 * 52;
        let squad: Vec<usize> = (0..pop.len())
            .filter(|&i| pop.club[i] as usize == club_id && !pop.is_retired(i, elapsed))
            .collect();
        assert_eq!(
            pop.live_strength(club_id, elapsed),
            pop.live_strength_from_squad(&squad, elapsed)
        );
    }

    #[test]
    fn live_strength_excludes_retired_players() {
        let mut pop = Population::default();
        let mut push = |birth_age_weeks: u32, potential_ovr: u8| {
            pop.seed.push(1);
            pop.club.push(0);
            pop.nation.push(0);
            pop.position.push(0);
            pop.birth_age_weeks.push(birth_age_weeks);
            pop.potential_ovr.push(potential_ovr);
            pop.intake_week.push(0);
            pop.career_goals.push(0);
            pop.career_apps.push(0);
            pop.career_titles.push(0);
        };
        push(20 * 52, 60); // active, age 20
        push(25 * 52, 60); // active, age 25
        push(45 * 52, 99); // already past RETIRE_AGE_YEARS, a sky-high potential

        let elapsed = 0;
        let squad = vec![0, 1, 2];
        let naive_mean: u32 = squad
            .iter()
            .map(|&i| pop.current_ovr(i, elapsed) as u32)
            .sum::<u32>()
            / squad.len() as u32;
        let filtered = pop.live_strength_from_squad(&squad, elapsed);
        assert!(
            (filtered as u32) < naive_mean,
            "live_strength_from_squad ({filtered}) should be lower than the naive unfiltered \
             mean ({naive_mean}) once the high-potential retiree is excluded"
        );
    }

    #[test]
    fn live_strength_changes_as_roster_ages() {
        let world = WorldGenesis::generate(31);
        let pop = genesis(31, &world);
        let differs = world
            .clubs
            .iter()
            .any(|c| pop.live_strength(c.id, 0) != pop.live_strength(c.id, 20 * 52));
        assert!(
            differs,
            "at least one club's live strength should differ across a 20-season gap as its \
             roster ages"
        );
    }

    #[test]
    fn youth_intake_adds_players_deterministically() {
        let world = WorldGenesis::generate(41);
        let mut pop_a = genesis(41, &world);
        let mut pop_b = genesis(41, &world);
        let added_a = apply_youth_intake(&mut pop_a, &world, 41, 1);
        let added_b = apply_youth_intake(&mut pop_b, &world, 41, 1);
        assert_eq!(added_a, added_b);
        assert!(
            added_a > 0,
            "expected at least one club to receive intake in season 1"
        );
        assert_eq!(pop_a.fingerprint(), pop_b.fingerprint());
    }

    #[test]
    fn youth_intake_respects_ceiling() {
        let world = WorldGenesis::generate(9);
        let mut pop = genesis(9, &world);
        let club = &world.clubs[0];
        let season = 3u32;
        let elapsed_weeks = season * 52;
        let ceiling = club.squad_size as u32 + club.squad_size as u32 / 5;

        // Artificially inflate this club's active squad to at/above its ceiling.
        let current_active = (0..pop.len())
            .filter(|&i| pop.club[i] as usize == club.id && !pop.is_retired(i, elapsed_weeks))
            .count() as u32;
        let to_add = ceiling.saturating_sub(current_active) + 2;
        for extra in 0..to_add {
            pop.seed.push(90_000 + extra as u64);
            pop.club.push(club.id as u16);
            pop.nation.push(club.nation as u8);
            pop.position.push(0);
            pop.birth_age_weeks.push(20 * 52);
            pop.potential_ovr.push(50);
            pop.intake_week.push(0);
            pop.career_goals.push(0);
            pop.career_apps.push(0);
            pop.career_titles.push(0);
        }

        let len_before = pop.len();
        apply_youth_intake(&mut pop, &world, 9, season);
        let new_rows_for_club = (len_before..pop.len())
            .filter(|&i| pop.club[i] as usize == club.id)
            .count();
        assert_eq!(
            new_rows_for_club, 0,
            "a club already at/above its squad_size*1.2 ceiling must get zero intake this season"
        );
    }

    #[test]
    fn youth_intake_uses_shared_outlier_formula() {
        let world = WorldGenesis::generate(9);
        let mut pop = genesis(9, &world);
        let club = &world.clubs[0];
        let season = 5u32;
        apply_youth_intake(&mut pop, &world, 9, season);

        let mut local_idx = 0u64;
        for i in 0..pop.len() {
            if pop.club[i] as usize == club.id && pop.intake_week[i] == season * 52 {
                let pseed = intake_player_seed(9, club.id as u64, season, local_idx);
                assert_eq!(
                    pop.seed[i], pseed,
                    "intake player seed must match intake_player_seed"
                );
                let mut rng = GoatRng::new(pseed);
                let expected = roll_potential_ovr(&mut rng, club.strength);
                assert_eq!(
                    pop.potential_ovr[i], expected,
                    "intake potential_ovr must come from the shared roll_potential_ovr, not a \
                     re-derived formula"
                );
                local_idx += 1;
            }
        }
        assert!(
            local_idx > 0,
            "expected at least one intake player for club 0 this season"
        );
    }

    #[test]
    fn intake_player_age_is_correct_mid_career() {
        let world = WorldGenesis::generate(9);
        let mut pop = genesis(9, &world);
        let season = 6u32;
        apply_youth_intake(&mut pop, &world, 9, season);
        let elapsed_weeks = season * 52;

        let idx = (0..pop.len())
            .find(|&i| pop.intake_week[i] == elapsed_weeks)
            .expect("expected at least one intake player at this season");
        assert_eq!(
            pop.age_years_at(idx, elapsed_weeks),
            16,
            "an intake player must report age 16 at his own intake week"
        );
        // Regression guard: under the pre-4.4 birth_age_weeks-only formula (no intake
        // offset), this must NOT come out to 16 — proves the fix is load-bearing.
        let old_formula_age = (pop.birth_age_weeks[idx] + elapsed_weeks) / 52;
        assert_ne!(
            old_formula_age, 16,
            "the old formula (no intake_week offset) must get this wrong, or the fix isn't \
             load-bearing"
        );
    }

    #[test]
    fn fingerprint_changes_after_intake_but_is_still_deterministic() {
        let world = WorldGenesis::generate(9);
        let mut pop = genesis(9, &world);
        let fp_before = pop.fingerprint();
        apply_youth_intake(&mut pop, &world, 9, 2);
        let fp_after = pop.fingerprint();
        assert_ne!(
            fp_before, fp_after,
            "fingerprint must change once new identity data (intake players) exists"
        );

        let mut pop2 = genesis(9, &world);
        apply_youth_intake(&mut pop2, &world, 9, 2);
        assert_eq!(
            fp_after,
            pop2.fingerprint(),
            "two independent runs through the same intake must produce the same post-intake \
             fingerprint"
        );
    }

    #[test]
    fn promote_passes_club_tactical_identity() {
        use crate::world::{DivLevel, GeneratedNation, League};
        use goat_core::attrs::AttrId;
        use goat_core::roles::{RoleId, NUM_ROLES};
        use goat_core::tactical_identity::TacticalIdentity;

        let neutral = TacticalIdentity {
            role_weight: [Fixed::ONE; NUM_ROLES],
        };
        // Same lopsided identity verified in generation.rs's own
        // tactical_bias_shifts_technical_club_toward_technical_attrs test to raise Vision.
        let mut technical = TacticalIdentity {
            role_weight: [Fixed::ONE; NUM_ROLES],
        };
        for &r in &[
            RoleId::CentralMid,
            RoleId::AttackingMid,
            RoleId::Trequartista,
            RoleId::DefensiveMid,
            RoleId::Sweeper,
        ] {
            technical.role_weight[r as usize] = Fixed::raw(1_600);
        }
        for &r in &[RoleId::CentreBack, RoleId::FullBack] {
            technical.role_weight[r as usize] = Fixed::raw(400);
        }

        let make_club = |id: usize, tactical_identity: TacticalIdentity| crate::world::Club {
            id,
            name: format!("Club{id}"),
            nation: 0,
            strength: 60,
            squad_size: 20,
            tactical_identity,
            budget: 0,
            academy_boost: 0,
        };
        let world = WorldGenesis {
            nations: vec![GeneratedNation {
                id: 0,
                name: "Testland".into(),
                stature: 60,
                tactical_identity: neutral.clone(),
            }],
            leagues: vec![League {
                id: 0,
                nation: 0,
                tier: DivLevel::Top,
                name: "Test League".into(),
                clubs: vec![0, 1],
                max_clubs: 2,
            }],
            clubs: vec![make_club(0, neutral), make_club(1, technical)],
        };

        let mut pop = Population::default();
        let push = |pop: &mut Population, club_id: u16| {
            pop.seed.push(555);
            pop.club.push(club_id);
            pop.nation.push(0);
            pop.position.push(1); // Midfielder family -> CM primary position
            pop.birth_age_weeks.push(20 * 52);
            pop.potential_ovr.push(60);
            pop.intake_week.push(0);
            pop.career_goals.push(0);
            pop.career_apps.push(0);
            pop.career_titles.push(0);
        };
        push(&mut pop, 0);
        push(&mut pop, 1);

        // Same underlying seed, same everything else — only the club's tactical_identity
        // differs. A statistically detectable skew end-to-end (Design round 3, Doc C §5.5).
        let neutral_view = pop.promote(0, 0, "Neutral", &world).unwrap();
        let technical_view = pop.promote(1, 0, "Technical", &world).unwrap();
        assert!(
            technical_view.potential[AttrId::Vision as usize]
                > neutral_view.potential[AttrId::Vision as usize],
            "the technically-biased club's promoted player should have a higher Vision \
             potential than the neutral club's, same underlying seed"
        );
    }
}
