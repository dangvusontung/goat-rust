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
use goat_core::generation::{generate_player_biased_with_ceiling, CreationChoices};
use goat_core::player::PlayerView;
use goat_core::positions::PrimaryPosition;
use goat_fixed::Fixed;
use goat_rng::{GoatRng, RngSource};
use std::cell::RefCell;

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

type DevelopmentCache = Option<(u32, [Fixed; goat_core::attrs::NUM_ATTRS], u8)>;

/// Background population as parallel columns. Index `i` identifies one player across all
/// columns — there is no per-player struct.
#[derive(Debug, Clone, Default)]
pub struct Population {
    dated_exposure: bool,
    sampled_health: bool,
    availability_cache: RefCell<Vec<Option<(u32, u32, u32)>>>,
    life_cache: RefCell<Vec<Option<(u32, crate::npc_life::NpcHealthState)>>>,
    exposures: crate::exposure::ExposureColumns,

    /// Empty for the frozen legacy model. New populations use shared development.
    shared_facilities: Vec<Fixed>,
    /// Derived cache only: discarded/rebuilt freely, never part of saved identity.
    shared_cache: RefCell<Vec<DevelopmentCache>>,
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
    /// Slow-moving individual form 0–100 (PA2 M3), fed ONLY by deep-simmed PC
    /// matches via the orbit overlay (EMA of synthetic match ratings). Players
    /// never touched by the orbit keep the neutral 50. Path-dependent — but not
    /// stored itself: it is replayed from `WorldState::orbit_records` on rebuild.
    /// Not part of `fingerprint` (identity columns only); the career residue is
    /// covered by `career_fingerprint` callers that also compare `form`.
    pub form: Vec<i16>,
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
            pop.form.push(50);
        }
    }

    pop
}

/// New-version population: preserve identity draws, share PC development laws.
pub fn genesis_shared(world_seed: u64, world: &WorldGenesis) -> Population {
    let mut pop = genesis(world_seed, world);
    pop.shared_facilities = pop
        .club
        .iter()
        .map(|&club| world.clubs[club as usize].facilities_mult())
        .collect();
    pop.shared_cache = RefCell::new(vec![None; pop.len()]);
    pop
}

/// Version 6: dated exposure and shared expected health. Version 3–5 factory remains frozen.
pub fn genesis_developed(world_seed: u64, world: &WorldGenesis) -> Population {
    let mut pop = genesis_shared(world_seed, world);
    pop.dated_exposure = true;
    pop.exposures.heads = vec![None; pop.len()];
    pop
}

/// Version 7: individual weekly training, energy and dated injuries after intake.
pub fn genesis_lived(world_seed: u64, world: &WorldGenesis) -> Population {
    let mut pop = genesis_developed(world_seed, world);
    pop.sampled_health = true;
    pop.life_cache = RefCell::new(vec![None; pop.len()]);
    pop.availability_cache = RefCell::new(vec![None; pop.len()]);
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
    /// Whether new opportunity/development rules should resolve this population.
    pub fn uses_shared_model(&self) -> bool {
        !self.shared_facilities.is_empty()
    }

    pub fn uses_dated_exposure(&self) -> bool {
        self.dated_exposure
    }

    /// Only changes at/after entry and in chronological order are accepted.
    /// All caches for this player are invalidated on revision, including same-date reads.
    pub fn record_exposure(&mut self, idx: usize, exposure: crate::exposure::NpcExposure) -> bool {
        if !self.dated_exposure
            || idx >= self.len()
            || !exposure.valid()
            || exposure.start_week < self.intake_week[idx]
        {
            return false;
        }
        let history = self.exposures.history(idx);
        if history
            .last()
            .is_some_and(|e| e.start_week > exposure.start_week)
        {
            return false;
        }
        let mut previous = history.last().copied().unwrap_or_else(|| {
            crate::exposure::NpcExposure::balanced(
                self.intake_week[idx],
                self.shared_facilities[idx],
            )
        });
        previous.start_week = exposure.start_week;
        if previous == exposure {
            return true;
        }
        self.exposures.push(idx, exposure);
        // A dated intervention cannot change any completed week before its start.
        let keep_prefix = exposure.start_week > self.intake_week[idx]
            && self.shared_cache.get_mut()[idx]
                .is_some_and(|(date, _, _)| date <= exposure.start_week);
        if !keep_prefix {
            self.shared_cache.get_mut()[idx] = None;
            if self.sampled_health {
                self.life_cache.get_mut()[idx] = None;
            }
        }
        if self.sampled_health {
            let keep_availability = self.availability_cache.get_mut()[idx]
                .is_some_and(|(date, _, _)| date <= exposure.start_week);
            if !keep_availability {
                self.availability_cache.get_mut()[idx] = None;
            }
        }
        true
    }

    pub fn exposure_at(&self, idx: usize, week: u32) -> crate::exposure::NpcExposure {
        self.exposures
            .history(idx)
            .into_iter()
            .rev()
            .find(|e| e.start_week <= week)
            .unwrap_or_else(|| {
                crate::exposure::NpcExposure::balanced(
                    self.intake_week[idx],
                    self.shared_facilities[idx],
                )
            })
    }

    pub fn expected_health_at(
        &self,
        idx: usize,
        week: u32,
    ) -> Option<crate::exposure::ExpectedHealth> {
        if !self.dated_exposure || idx >= self.len() || week < self.intake_week[idx] {
            return None;
        }
        let view = self.shared_view(idx, week);
        let mut e = self.exposure_at(idx, week);
        if self.sampled_health {
            e.energy = view.energy;
        }
        Some(crate::exposure::expected_health(
            e,
            view.age_weeks / 52,
            view.durability_x10,
        ))
    }

    /// Called by deterministic replay after a season. Appearance totals are only a
    /// workload proxy: no fabricated per-match energy or injury history.
    pub(crate) fn record_season_workload(
        &mut self,
        idx: usize,
        week: u32,
        apps: u32,
        facilities: Fixed,
    ) {
        if !self.dated_exposure {
            return;
        }
        let mut e = self.exposure_at(idx, week);
        e.start_week = week;
        e.workload_apps = apps.min(u16::MAX as u32) as u16;
        // First calibration assumption: a regular starter has 75 energy, fringe ~86.
        e.energy = Fixed::from_int((90 - apps.min(60) as i32 / 2).max(55));
        e.facilities = facilities;
        self.record_exposure(idx, e);
    }

    /// Sparse intervention history. Before the first entry, balanced intake conditions apply.
    pub fn exposure_history(&self, idx: usize) -> Vec<crate::exposure::NpcExposure> {
        if !self.dated_exposure || idx >= self.len() {
            return Vec::new();
        }
        self.exposures.history(idx)
    }

    pub fn exposure_segment_count(&self) -> usize {
        self.exposures.len()
    }

    /// Shared injury risk and expected recovery duration, recalculated at each age band.
    /// No seeded individual injury episodes are claimed for background NPCs.
    fn exposure_plan(
        e: crate::exposure::NpcExposure,
        age: u32,
        durability: u8,
    ) -> goat_core::development::DevelopmentPlan {
        use goat_core::{tuning::*, week::Intensity};
        let intensity = match e.intensity {
            0 => Intensity::Low,
            2 => Intensity::High,
            _ => Intensity::Medium,
        };
        let intensity = if e.energy < ENERGY_AUTO_DOWNGRADE {
            Intensity::Low
        } else {
            intensity
        };
        let healthy = crate::exposure::expected_health(e, age, durability).healthy_share;
        goat_core::development::DevelopmentPlan {
            intensity: match intensity {
                Intensity::Low => GROWTH_MULT_LOW,
                Intensity::High => GROWTH_MULT_HIGH,
                _ => GROWTH_MULT_MED,
            },
            energy: goat_core::week::energy_growth_factor(e.energy),
            facilities: e.facilities,
            focus_share: e.focus_share,
            healthy_share: healthy,
            ceiling: match e.intensity {
                0 => INTENSITY_CEILING_LOW,
                2 => INTENSITY_CEILING_HIGH,
                _ => INTENSITY_CEILING_MED,
            } * match e.lifestyle {
                0 => LIFESTYLE_CEILING_PRO,
                2 => LIFESTYLE_CEILING_FLASHY,
                _ => LIFESTYLE_CEILING_BALANCED,
            },
            // PC skips all development while injured, including age decline.
            decline: healthy
                * match e.lifestyle {
                    0 => DECLINE_LIFESTYLE_PRO,
                    2 => DECLINE_LIFESTYLE_FLASHY,
                    _ => DECLINE_LIFESTYLE_BALANCED,
                },
        }
    }

    pub fn uses_individual_health(&self) -> bool {
        self.sampled_health
    }

    fn lived_view(&self, idx: usize, week: u32, mut view: PlayerView) -> PlayerView {
        use crate::npc_life::{tick, NpcHealthState};
        use goat_core::attrs::ATTR_ARCHETYPES;
        let mut start = self.intake_week[idx];
        let mut health = NpcHealthState {
            energy: self.exposure_at(idx, start).energy,
            injury_weeks: 0,
            available_mask: 0,
            elapsed_weeks: 0,
        };
        let cached = self.shared_cache.borrow()[idx];
        let cached_health = self.life_cache.borrow()[idx];
        if let (Some((date, attrs, _)), Some((health_date, state))) = (cached, cached_health) {
            if date <= week && date == health_date {
                view.current = attrs;
                start = date;
                health = state;
            }
        }
        if start == self.intake_week[idx]
            && !cached.is_some_and(|(date, _, _)| {
                date == start && date <= week && cached_health.is_some_and(|(h, _)| h == date)
            })
        {
            // No fabricated pre-genesis medical history; pre-entry attributes keep the
            // earlier expected projection. Individual simulation starts at world entry.
            let e = crate::exposure::NpcExposure::balanced(
                self.intake_week[idx],
                self.shared_facilities[idx],
            );
            let mut age = 16 * 52;
            while age < self.birth_age_weeks[idx] {
                let end = self.birth_age_weeks[idx].min((age / 52 + 1) * 52);
                let plan = Self::exposure_plan(e, age / 52, view.durability_x10);
                for (a, kind) in ATTR_ARCHETYPES.iter().enumerate() {
                    view.current[a] = goat_core::development::project_attribute_interval(
                        view.current[a],
                        view.potential[a],
                        *kind,
                        age,
                        end,
                        plan,
                    );
                }
                age = end;
            }
        }
        let changes = self.exposure_history(idx);
        let mut e = self.exposure_at(idx, start);
        let mut next = changes.partition_point(|e| e.start_week <= start);
        let end = week.min(
            self.intake_week[idx]
                + (RETIRE_AGE_YEARS * 52).saturating_sub(self.birth_age_weeks[idx]),
        );
        for date in start..end {
            while next < changes.len() && changes[next].start_week <= date {
                e = changes[next];
                next += 1;
            }
            tick(
                self.seed[idx],
                date,
                (self.birth_age_weeks[idx] + date - self.intake_week[idx]) / 52,
                self.position[idx],
                view.durability_x10,
                e,
                &mut health,
                Some((&mut view.current, &view.potential)),
            );
        }
        view.age_weeks = self.birth_age_weeks[idx] + week.saturating_sub(self.intake_week[idx]);
        view.energy = health.energy;
        view.injury_weeks = health.injury_weeks;
        self.shared_cache.borrow_mut()[idx] = Some((
            week,
            view.current,
            goat_core::derive::ovr(&view.current, view.primary_position)
                .to_int()
                .clamp(1, 99) as u8,
        ));
        self.life_cache.borrow_mut()[idx] = Some((week, health));
        view
    }

    /// Factual simulation records, replayed on contact rather than stored per NPC.
    pub fn training_history(
        &self,
        idx: usize,
        from: u32,
        to: u32,
    ) -> Vec<crate::npc_life::NpcTrainingWeek> {
        if !self.sampled_health || idx >= self.len() || from >= to {
            return Vec::new();
        }
        let choices = CreationChoices {
            name: String::new(),
            primary_position: position_from_u8(self.position[idx]),
            nationality: String::new(),
            club: String::new(),
        };
        let view = generate_player_biased_with_ceiling(
            self.seed[idx],
            &choices,
            None,
            Some(self.potential_ovr[idx]),
        );
        let start = self.intake_week[idx];
        let mut health = crate::npc_life::NpcHealthState {
            energy: self.exposure_at(idx, start).energy,
            injury_weeks: 0,
            available_mask: 0,
            elapsed_weeks: 0,
        };
        let changes = self.exposure_history(idx);
        let mut e = self.exposure_at(idx, start);
        let mut next = changes.partition_point(|e| e.start_week <= start);
        let end = to.min(start + (RETIRE_AGE_YEARS * 52).saturating_sub(self.birth_age_weeks[idx]));
        let mut out = Vec::new();
        for date in start..end {
            while next < changes.len() && changes[next].start_week <= date {
                e = changes[next];
                next += 1;
            }
            let record = crate::npc_life::tick(
                self.seed[idx],
                date,
                (self.birth_age_weeks[idx] + date - start) / 52,
                self.position[idx],
                view.durability_x10,
                e,
                &mut health,
                None,
            );
            if date >= from {
                out.push(record);
            }
        }
        out
    }

    pub fn injury_history(
        &self,
        idx: usize,
        through: u32,
    ) -> Vec<crate::npc_life::NpcInjuryEpisode> {
        self.training_history(
            idx,
            self.intake_week.get(idx).copied().unwrap_or(0),
            through,
        )
        .into_iter()
        .filter(|w| w.new_injury)
        .map(|w| crate::npc_life::NpcInjuryEpisode {
            onset_week: w.week,
            expected_recovery_week: w.week + 1 + w.injury_after,
            recovered_week: (w.week + 1 + w.injury_after
                <= through.min(
                    self.intake_week[idx]
                        + (RETIRE_AGE_YEARS * 52).saturating_sub(self.birth_age_weeks[idx]),
                ))
            .then_some(w.week + 1 + w.injury_after),
            duration_weeks: w.injury_after,
        })
        .collect()
    }

    pub(crate) fn appearance_quota(&self, idx: usize, week: u32, quota: u32) -> u32 {
        if !self.sampled_health {
            return quota;
        }
        if let Some((date, healthy, total)) = self.availability_cache.borrow()[idx] {
            if date == week {
                return quota * healthy / total.max(1);
            }
        }
        let same_date = self.life_cache.borrow()[idx].is_some_and(|(date, _)| date == week);
        if !same_date {
            self.shared_view(idx, week);
        }
        let (_, health) = self.life_cache.borrow()[idx].unwrap();
        let total = health.elapsed_weeks.min(52);
        let healthy = health.available_mask.count_ones();
        self.availability_cache.borrow_mut()[idx] = Some((week, healthy, total));
        quota * healthy / total.max(1)
    }

    pub fn is_available(&self, idx: usize, week: u32) -> bool {
        if self.is_retired(idx, week) || week < self.intake_week[idx] {
            return false;
        }
        if !self.sampled_health {
            return true;
        }
        if let Some((date, health)) = self.life_cache.borrow()[idx] {
            if date == week {
                return health.injury_weeks == 0;
            }
        }
        self.shared_view(idx, week).injury_weeks == 0
    }

    fn shared_view(&self, idx: usize, elapsed_weeks: u32) -> PlayerView {
        use goat_core::attrs::ATTR_ARCHETYPES;
        use goat_core::development::{project_attribute, DevelopmentPlan};
        use goat_core::tuning::INTENSITY_CEILING_MED;
        let choices = CreationChoices {
            name: String::new(),
            primary_position: position_from_u8(self.position[idx]),
            nationality: String::new(),
            club: String::new(),
        };
        // Innate attributes derive from identity, never today's club philosophy.
        let mut view = generate_player_biased_with_ceiling(
            self.seed[idx],
            &choices,
            None,
            Some(self.potential_ovr[idx]),
        );
        let age = self.birth_age_weeks[idx] + elapsed_weeks.saturating_sub(self.intake_week[idx]);
        if self.sampled_health {
            return self.lived_view(idx, elapsed_weeks, view);
        }
        if self.dated_exposure {
            view.energy = self.exposure_at(idx, elapsed_weeks).energy;
        }
        let mut projected_from_age = 16 * 52;
        if let Some((week, attrs, _)) = &self.shared_cache.borrow()[idx] {
            if self.dated_exposure && *week < elapsed_weeks {
                view.current = *attrs;
                projected_from_age =
                    self.birth_age_weeks[idx] + week.saturating_sub(self.intake_week[idx]);
            }
            if *week == elapsed_weeks {
                view.current = *attrs;
                view.age_weeks = age;
                return view;
            }
        }
        let plan = DevelopmentPlan {
            intensity: Fixed::ONE,
            energy: Fixed::raw(900),
            facilities: self.shared_facilities[idx],
            focus_share: Fixed::raw(700),
            healthy_share: Fixed::raw(900),
            ceiling: INTENSITY_CEILING_MED,
            decline: Fixed::ONE,
        };
        if self.dated_exposure {
            use crate::exposure::NpcExposure;
            use goat_core::development::project_attribute_interval;
            let mut inputs = vec![(
                16 * 52,
                NpcExposure::balanced(self.intake_week[idx], self.shared_facilities[idx]),
            )];
            inputs.extend(
                self.exposures
                    .history(idx)
                    .into_iter()
                    .filter(|e| e.start_week <= elapsed_weeks)
                    .map(|e| {
                        (
                            self.birth_age_weeks[idx] + e.start_week - self.intake_week[idx],
                            e,
                        )
                    }),
            );
            for segment in 0..inputs.len() {
                let (mut start, exposure) = inputs[segment];
                let end = inputs
                    .get(segment + 1)
                    .map_or(age, |(start, _)| *start)
                    .min(age);
                start = start.max(projected_from_age);
                while start < end {
                    let band_end = end.min((start / 52 + 1) * 52);
                    let plan = Self::exposure_plan(exposure, start / 52, view.durability_x10);
                    for (a, archetype) in ATTR_ARCHETYPES.iter().enumerate() {
                        view.current[a] = project_attribute_interval(
                            view.current[a],
                            view.potential[a],
                            *archetype,
                            start,
                            band_end,
                            plan,
                        );
                    }
                    start = band_end;
                }
            }
        } else {
            for (a, archetype) in ATTR_ARCHETYPES.iter().enumerate() {
                view.current[a] =
                    project_attribute(view.current[a], view.potential[a], *archetype, age, plan);
            }
        }
        view.age_weeks = age;
        self.shared_cache.borrow_mut()[idx] = Some((
            elapsed_weeks,
            view.current,
            goat_core::derive::ovr(&view.current, view.primary_position)
                .to_int()
                .clamp(1, 99) as u8,
        ));
        view
    }
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
        if self.uses_shared_model() {
            if let Some((week, _, ovr)) = &self.shared_cache.borrow()[idx] {
                if *week == elapsed_weeks {
                    return *ovr;
                }
            }
            let view = self.shared_view(idx, elapsed_weeks);
            return goat_core::derive::ovr(&view.current, view.primary_position)
                .to_int()
                .clamp(1, 99) as u8;
        }
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
        if self.uses_shared_model() {
            let mut view = self.shared_view(idx, elapsed_weeks);
            view.name = name.into();
            return Some(view);
        }
        let club = &world.clubs[self.club[idx] as usize];
        let choices = CreationChoices {
            name: name.into(),
            primary_position: position_from_u8(self.position[idx]),
            nationality: world.nation_name(self.nation[idx] as usize).to_string(),
            club: club.name.clone(),
        };
        // generate_player_biased gives the realistic per-attribute potential + shape +
        // roles, nudged by the club's tactical identity (Design round 3, Doc C §Slice 5).
        // The ceiling is OVERRIDDEN with this player's stored `potential_ovr` — the batch
        // sim has aged him against that headline number since genesis/intake, so promotion
        // must not re-roll it (previously every promoted player silently got a 99 ceiling,
        // contradicting his own background curve). We then overwrite current to the
        // age-appropriate fraction of that potential.
        let mut view = generate_player_biased_with_ceiling(
            self.seed[idx],
            &choices,
            Some(&club.tactical_identity),
            Some(self.potential_ovr[idx]),
        );
        let frac = development_fraction(self.age_years_at(idx, elapsed_weeks));
        for a in 0..NUM_ATTRS {
            view.current[a] = (view.potential[a] * frac).clamp(Fixed::MIN_ATTR, view.potential[a]);
        }
        let weeks_since_intake = elapsed_weeks.saturating_sub(self.intake_week[idx]);
        view.age_weeks = self.birth_age_weeks[idx] + weeks_since_intake;
        Some(view)
    }

    // ── PA2: match-day lineups, squad profiles, PC selection ─────────────────

    /// Indices of a club's match-day lineup (PA2 M1): the top `count` available
    /// squad members by current OVR at `elapsed_weeks`, skipping the retired.
    /// Deterministic — ties broken by population index (insertion order).
    pub fn lineup_indices(&self, club_id: usize, elapsed_weeks: u32, count: usize) -> Vec<usize> {
        let mut squad: Vec<usize> = (0..self.len())
            .filter(|&i| self.club[i] as usize == club_id && self.is_available(i, elapsed_weeks))
            .collect();
        squad.sort_by_key(|&i| std::cmp::Reverse(self.current_ovr(i, elapsed_weeks)));
        squad.truncate(count);
        squad
    }

    /// Mean current attributes of a club's lineup (PA2 M1), optionally plus one
    /// external player (the PC occupies a starting slot, so his real attrs lift
    /// the team profile). Returns `None` if the club cannot field a full side.
    pub fn squad_avg_attrs(
        &self,
        club_id: usize,
        elapsed_weeks: u32,
        world: &WorldGenesis,
        extra: Option<&[Fixed; NUM_ATTRS]>,
    ) -> Option<[Fixed; NUM_ATTRS]> {
        let n_npc = 11 - usize::from(extra.is_some());
        let lineup = self.lineup_indices(club_id, elapsed_weeks, n_npc);
        if lineup.len() < n_npc {
            return None;
        }
        let mut sums = [0i64; NUM_ATTRS];
        let mut n = 0i64;
        for idx in lineup {
            let view = self.promote(
                idx,
                elapsed_weeks,
                crate::history::name_from_seed(self.seed[idx]),
                world,
            )?;
            for (a, s) in sums.iter_mut().enumerate() {
                *s += view.current[a].to_raw() as i64;
            }
            n += 1;
        }
        if let Some(attrs) = extra {
            for (a, s) in sums.iter_mut().enumerate() {
                *s += attrs[a].to_raw() as i64;
            }
            n += 1;
        }
        let mut avg = [Fixed::ZERO; NUM_ATTRS];
        for (a, v) in avg.iter_mut().enumerate() {
            *v = Fixed::raw((sums[a] / n) as i32);
        }
        Some(avg)
    }

    // ── PA2 M1.5: formation-aware lineup + PC selection ──────────────────────

    /// Blended selection score for an NPC (PA2 M3): current OVR + the real
    /// form pull — the same `(form − 50) × 3/10` weight the PC's own selection
    /// formula uses. Players untouched by the orbit sit at form 50 and score
    /// exactly their OVR (pre-M3 behaviour).
    fn npc_selection_score(&self, idx: usize, elapsed_weeks: u32) -> i32 {
        self.current_ovr(idx, elapsed_weeks) as i32 + (self.form[idx] as i32 - 50) * 3 / 10
    }

    /// Top `slots.{0,1,2}` available players within each position group (D/M/F),
    /// skipping the retired, ranked by the form-blended selection score (PA2 M3:
    /// a teammate in a hot streak holds his shirt; form 50 = pure OVR order).
    /// Deterministic and noise-free — this is the *profile* lineup (team
    /// strength), not the selection drama.
    ///
    /// `slots` counts OUTFIELD players only (real football convention: the
    /// formation's numbers sum to 10). The population has no goalkeeper entity,
    /// so the 11th man is an implicit abstract GK — see
    /// `TacticalProfile::formation_slots`.
    pub fn lineup_indices_formation(
        &self,
        club_id: usize,
        elapsed_weeks: u32,
        slots: (usize, usize, usize),
    ) -> Vec<usize> {
        let mut out = Vec::with_capacity(slots.0 + slots.1 + slots.2);
        for (pos, &n) in [slots.0, slots.1, slots.2].iter().enumerate() {
            let mut group: Vec<usize> = (0..self.len())
                .filter(|&i| {
                    self.club[i] as usize == club_id
                        && self.position[i] == pos as u8
                        && self.is_available(i, elapsed_weeks)
                })
                .collect();
            group.sort_by_key(|&i| std::cmp::Reverse(self.npc_selection_score(i, elapsed_weeks)));
            group.truncate(n);
            out.extend(group);
        }
        out
    }

    /// Formation-aware mean attributes of a club's lineup. When the PC starts,
    /// `pc` is `Some((position, attrs))` and he takes one slot in his position
    /// group (one fewer NPC is averaged there); when he is benched, `None`.
    /// Returns `None` if the club cannot field the full split.
    ///
    /// The mean is taken over the 10 outfield slots (plus the PC when he
    /// starts) — the abstract GK contributes nothing, and a mean stays
    /// comparable to the opponent's top-11-OVR mean regardless of n.
    pub fn squad_avg_attrs_formation(
        &self,
        club_id: usize,
        elapsed_weeks: u32,
        world: &WorldGenesis,
        slots: (usize, usize, usize),
        pc: Option<(u8, &[Fixed; NUM_ATTRS])>,
    ) -> Option<[Fixed; NUM_ATTRS]> {
        let npc_slots = match pc {
            Some((pos, _)) => {
                let mut s = slots;
                match pos {
                    0 => s.0 = s.0.saturating_sub(1),
                    1 => s.1 = s.1.saturating_sub(1),
                    _ => s.2 = s.2.saturating_sub(1),
                }
                s
            }
            None => slots,
        };
        let lineup = self.lineup_indices_formation(club_id, elapsed_weeks, npc_slots);
        let want = npc_slots.0 + npc_slots.1 + npc_slots.2;
        if lineup.len() < want {
            return None;
        }
        let mut sums = [0i64; NUM_ATTRS];
        let mut n = 0i64;
        for idx in lineup {
            let view = self.promote(
                idx,
                elapsed_weeks,
                crate::history::name_from_seed(self.seed[idx]),
                world,
            )?;
            for (a, s) in sums.iter_mut().enumerate() {
                *s += view.current[a].to_raw() as i64;
            }
            n += 1;
        }
        if let Some((_, attrs)) = pc {
            for (a, s) in sums.iter_mut().enumerate() {
                *s += attrs[a].to_raw() as i64;
            }
            n += 1;
        }
        let mut avg = [Fixed::ZERO; NUM_ATTRS];
        for (a, v) in avg.iter_mut().enumerate() {
            *v = Fixed::raw((sums[a] / n) as i32);
        }
        Some(avg)
    }

    /// Decide whether the PC starts or is benched this week (PA2 M1.5). The PC
    /// competes inside his position group for `group_slots` places; NPCs carry
    /// their REAL accumulated form (PA2 M3 — orbit overlay EMA, neutral 50 when
    /// untouched) plus a small seeded weekly noise (±5) and a ~3% availability
    /// exclusion, both ephemeral (nothing is stored). A PC just below the
    /// cutoff gets a seeded borderline roll where manager favor nudges the odds.
    pub fn select_pc(
        &self,
        club_id: usize,
        elapsed_weeks: u32,
        group_slots: usize,
        pc: &PcSelectionInput,
        week_seed: u64,
    ) -> SelectionOutcome {
        if pc.unavailable || group_slots == 0 {
            return SelectionOutcome::Benched;
        }
        let pc_score = pc_selection_score(pc);

        let mut cand: Vec<(i32, bool)> = Vec::new(); // (score, is_pc)
        for i in 0..self.len() {
            if self.club[i] as usize != club_id
                || self.position[i] != pc.position
                || !self.is_available(i, elapsed_weeks)
            {
                continue;
            }
            let mut rng = GoatRng::new(self.seed[i] ^ week_seed);
            if !self.sampled_health && rng.next_range_u32(0, NPC_UNAVAILABLE_DIV) == 0 {
                continue; // knocked/suspended this week — ephemeral abstraction
            }
            let noise = rng.next_range_u32(0, 2 * NPC_FORM_NOISE) as i32 - NPC_FORM_NOISE as i32;
            cand.push((self.npc_selection_score(i, elapsed_weeks) + noise, false));
        }
        cand.push((pc_score, true));
        // Stable sort, score descending: on ties the PC (pushed last) loses to NPCs.
        cand.sort_by_key(|&(score, _)| std::cmp::Reverse(score));

        let rank = cand.iter().position(|&(_, is_pc)| is_pc).unwrap();
        if rank < group_slots {
            return SelectionOutcome::Starts;
        }
        let cutoff = cand[group_slots - 1].0; // lowest selected score
        let diff = pc_score - cutoff;
        if diff < -(BORDERLINE_BAND as i32) {
            return SelectionOutcome::Benched;
        }
        // Borderline: the manager hesitates — favor nudges the odds (±10).
        let pct = (50 + diff * 10 + (pc.favor - 50) / 5).clamp(1, 99);
        let mut roll = GoatRng::new(week_seed ^ BORDERLINE_SALT);
        if roll.next_range_u32(1, 100) as i32 <= pct {
            SelectionOutcome::Starts
        } else {
            SelectionOutcome::Benched
        }
    }
}

/// NPC unavailability divisor: `next_range(0, NPC_UNAVAILABLE_DIV) == 0` ⇒ out
/// this week (~3%; knock/suspension abstraction, seeded per player-week, never stored).
pub const NPC_UNAVAILABLE_DIV: u32 = 33;
/// Weekly NPC form noise (±points on selection score). Halved at M3: the real
/// accumulated form now carries the signal, noise only keeps weeks alive.
pub const NPC_FORM_NOISE: u32 = 5;
/// Score band below the selection cutoff in which the borderline roll applies.
pub const BORDERLINE_BAND: u32 = 5;
/// Salt for the borderline roll stream (independent of availability/noise draws).
const BORDERLINE_SALT: u64 = 0xB0DE_21A4_7C3F_55E1;

/// Everything the selection formula needs from the PC — all sourced from existing
/// `WorldState`/`PlayerView` fields (no new systems). See the task doc
/// (`tasks/TASK-PA2-world-into-match.md`, M1.5) for the locked formula.
#[derive(Debug, Clone, Copy)]
pub struct PcSelectionInput {
    /// Primary position family: 0 = Defender, 1 = Midfielder, 2 = Forward.
    pub position: u8,
    /// Role rating at the PC's best role (familiarity multiplier already inside).
    pub role_rating: i32,
    /// Familiarity tier at that role (0=Awkward … 3=Natural) — tactical fit term.
    pub familiarity_tier: u8,
    pub form: i32,
    pub trust: i32,
    pub favor: i32,
    /// Academy hype — only counts in the PC's first season at the club.
    pub academy_hype: i32,
    pub first_season_at_club: bool,
    /// Annual wage (thousands) — top-earner proxy vs `club_strength * 3`.
    pub wage_annual: i64,
    pub club_strength: u8,
    pub fan_rep: i32,
    pub marketability: i32,
    pub power_ladder: u8,
    /// Energy 0–100.
    pub energy: i32,
    /// Hard exclusion: injured or suspended.
    pub unavailable: bool,
}

/// The PC's multi-factor selection score (locked formula, M1.5):
/// role rating + form/trust pulls + tactical fit + first-season hype +
/// top-earner status + fan/market appeal − power-ladder rebellion − fatigue.
pub fn pc_selection_score(pc: &PcSelectionInput) -> i32 {
    let fit = [-6, -2, 0, 4][pc.familiarity_tier.min(3) as usize];
    let hype = if pc.first_season_at_club {
        (pc.academy_hype / 10).clamp(-5, 5)
    } else {
        0
    };
    let wage = if pc.wage_annual >= pc.club_strength as i64 * 3 {
        5
    } else {
        0
    };
    let energy = if pc.energy < 40 {
        -10
    } else if pc.energy < 60 {
        -4
    } else {
        0
    };
    pc.role_rating
        + (pc.form - 50) * 3 / 10
        + (pc.trust - 50) * 2 / 5
        + fit
        + hype
        + wage
        + (pc.fan_rep - 50) / 10
        + if pc.marketability >= 70 { 3 } else { 0 }
        - 8 * pc.power_ladder as i32
        + energy
}

/// Whether the PC starts or watches from the bench this week.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionOutcome {
    Starts,
    Benched,
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
            pop.form.push(50);
            total_added += 1;
            if pop.uses_shared_model() {
                pop.shared_facilities.push(club.facilities_mult());
                pop.shared_cache.get_mut().push(None);
                if pop.dated_exposure {
                    pop.exposures.heads.push(None);
                    if pop.sampled_health {
                        pop.life_cache.get_mut().push(None);
                        pop.availability_cache.get_mut().push(None);
                    }
                }
            }
        }
    }

    total_added
}

#[cfg(test)]
mod tests {
    #[test]
    fn individual_history_replay_cache_and_availability_agree() {
        let world = WorldGenesis::generate(42);
        let mut pop = genesis_lived(42, &world);
        let idx = (0..pop.len())
            .find(|&i| pop.birth_age_weeks[i] == 16 * 52)
            .unwrap();
        let original = pop.shared_view(idx, 104);
        let episodes = pop.injury_history(idx, 104);
        assert!(!episodes.is_empty());
        assert_eq!(pop.injury_history(idx, 104), episodes);
        let records = pop.training_history(idx, 0, 104);
        assert_eq!(records.len(), 104);
        assert_eq!(records.last().unwrap().energy_after, original.energy);
        assert_eq!(records.last().unwrap().injury_after, original.injury_weeks);
        for episode in &episodes {
            let w = &records[episode.onset_week as usize];
            assert!(w.new_injury);
            assert_eq!(w.focus_mask, 0);
            assert!(!pop.is_available(idx, episode.onset_week + 1));
            if let Some(day) = episode.recovered_week {
                assert!(pop.is_available(idx, day));
            }
        }
        for date in [0, 52, 53, 104, 156, 500] {
            let hot = pop.shared_view(idx, date);
            let mut cold = pop.clone();
            cold.shared_cache.get_mut().fill(None);
            cold.life_cache.get_mut().fill(None);
            let reference = cold.shared_view(idx, date);
            assert_eq!(hot.current, reference.current);
            assert_eq!(hot.energy, reference.energy);
            assert_eq!(hot.injury_weeks, reference.injury_weeks);
            let weeks = pop.training_history(idx, date.saturating_sub(52), date);
            let healthy = weeks
                .iter()
                .filter(|w| w.injury_before == 0 && w.injury_after == 0)
                .count() as u32;
            assert_eq!(
                pop.appearance_quota(idx, date, 30),
                30 * healthy / (weeks.len() as u32).max(1)
            );
        }
        let old_past = pop.training_history(idx, 0, 104);
        pop.record_exposure(
            idx,
            crate::exposure::NpcExposure {
                workload_apps: 30,
                ..crate::exposure::NpcExposure::balanced(104, Fixed::raw(1600))
            },
        );
        assert_eq!(old_past, pop.training_history(idx, 0, 104));
        assert_eq!(pop.shared_view(idx, 104).current, original.current);
        assert_eq!(pop.shared_view(idx, 156).potential, original.potential);
        let future = pop.training_history(idx, 104, 156);
        assert!(future.iter().any(|w| w.effective_intensity == 0));
    }

    #[test]
    fn injured_npcs_are_not_selected_or_credited_full_season_quota() {
        let world = WorldGenesis::generate(42);
        let pop = genesis_lived(42, &world);
        let idx = (0..pop.len())
            .find(|&i| !pop.injury_history(i, 52).is_empty())
            .unwrap();
        let episode = pop.injury_history(idx, 52)[0].clone();
        let week = episode.onset_week + 1;
        assert!(!pop
            .lineup_indices(pop.club[idx] as usize, week, 50)
            .contains(&idx));
        assert!(!pop
            .lineup_indices_formation(pop.club[idx] as usize, week, (25, 25, 25))
            .contains(&idx));
        assert!(pop.appearance_quota(idx, 52, 30) < 30);
    }

    #[test]
    fn dated_interventions_preserve_past_and_invalidate_same_date_cache() {
        use crate::exposure::NpcExposure;
        let world = WorldGenesis::generate(42);
        let mut pop = genesis_developed(42, &world);
        let idx = (0..pop.len())
            .find(|&i| pop.birth_age_weeks[i] == 16 * 52)
            .unwrap();
        let before = pop.shared_view(idx, 104);
        let old_future = pop.shared_view(idx, 208);
        let innate = old_future.potential;
        let mut e = NpcExposure::balanced(104, Fixed::from_int(2));
        assert!(pop.record_exposure(idx, e));
        assert_eq!(pop.shared_view(idx, 104).current, before.current);
        let future = pop.shared_view(idx, 208);
        assert_ne!(future.current, old_future.current);
        assert_eq!(future.potential, innate);
        assert_eq!(
            pop.current_ovr(idx, 208),
            goat_core::derive::ovr(&future.current, future.primary_position).to_int() as u8
        );
        e.start_week = 103;
        assert!(!pop.record_exposure(idx, e));
        e.start_week = 104;
        e.focus_share = Fixed::ZERO;
        assert!(pop.record_exposure(idx, e));
        assert_ne!(pop.shared_view(idx, 208).current, future.current);
        assert_eq!(
            pop.shared_view(idx, 52).current,
            genesis_developed(42, &world).shared_view(idx, 52).current
        );
        for week in [105, 156, 400, 1000] {
            let hot = pop.shared_view(idx, week).current;
            let mut cold = pop.clone();
            cold.shared_cache.get_mut().fill(None);
            assert_eq!(hot, cold.shared_view(idx, week).current);
        }
        e.energy = Fixed::from_int(101);
        assert!(!pop.record_exposure(idx, e));
    }

    #[test]
    fn dated_projection_matches_weekly_reference_over_long_career() {
        use crate::exposure::NpcExposure;
        use goat_core::{
            attrs::ATTR_ARCHETYPES,
            development::{weekly_decay, weekly_growth},
        };
        let world = WorldGenesis::generate(42);
        let mut pop = genesis_developed(42, &world);
        let idx = (0..pop.len())
            .find(|&i| pop.birth_age_weeks[i] == 16 * 52)
            .unwrap();
        let initial = pop.shared_view(idx, 0);
        let changes = [
            (0, Fixed::raw(600)),
            (104, Fixed::raw(1800)),
            (300, Fixed::ONE),
        ];
        for (start, facility) in changes {
            assert!(pop.record_exposure(idx, NpcExposure::balanced(start, facility)));
        }
        let mut current = initial.current;
        for week in 0..24 * 52 {
            let plan = Population::exposure_plan(
                pop.exposure_at(idx, week),
                16 + week / 52,
                initial.durability_x10,
            );
            for a in 0..NUM_ATTRS {
                let growth = (weekly_growth(
                    ATTR_ARCHETYPES[a],
                    16 + week / 52,
                    plan.intensity,
                    plan.energy,
                    plan.facilities,
                ) * plan.focus_share
                    * plan.healthy_share)
                    .clamp(Fixed::ZERO, goat_core::tuning::GROWTH_SINGLE_WEEK_CAP);
                let decay = weekly_decay(ATTR_ARCHETYPES[a], 16 + week / 52, plan.decline);
                let ceiling = (initial.potential[a] * plan.ceiling).max(Fixed::MIN_ATTR);
                current[a] = ((current[a] + growth).min(ceiling) - decay).max(Fixed::MIN_ATTR);
            }
            if week % 52 == 51 || week == 104 || week == 300 {
                assert_eq!(
                    pop.shared_view(idx, week + 1).current,
                    current,
                    "week {}",
                    week + 1
                );
            }
        }
    }

    #[test]
    fn youth_exposure_uses_intake_epoch_and_expected_health_uses_shared_risk() {
        use crate::exposure::NpcExposure;
        let world = WorldGenesis::generate(42);
        let mut pop = genesis_developed(42, &world);
        let first = pop.len();
        apply_youth_intake(&mut pop, &world, 42, 2);
        assert!(pop.len() > first);
        assert!(pop.exposure_history(first).is_empty());
        assert!(!pop.record_exposure(first, NpcExposure::balanced(103, Fixed::ONE)));
        assert!(pop.record_exposure(first, NpcExposure::balanced(104, Fixed::ONE)));
        assert_eq!(pop.shared_view(first, 104).age_weeks, 16 * 52);
        assert_eq!(pop.shared_view(first, 156).age_weeks, 17 * 52);
        let mut e = NpcExposure::balanced(0, Fixed::ONE);
        let fit = Population::exposure_plan(e, 25, 12);
        e.energy = Fixed::from_int(30);
        let tired = Population::exposure_plan(e, 25, 8);
        assert!(fit.healthy_share > tired.healthy_share);
        e.injury_duration_pct = 1500;
        assert!(Population::exposure_plan(e, 25, 8).healthy_share < tired.healthy_share);
    }

    use super::*;
    use crate::world::WorldGenesis;

    #[test]
    fn shared_ranking_promotion_and_cache_agree_without_rerolling_talent() {
        let world = WorldGenesis::generate(42);
        let mut pop = genesis_shared(42, &world);
        for i in 0..64 {
            let rating = pop.current_ovr(i, 52);
            assert_eq!(rating, pop.current_ovr(i, 52));
            let before = pop.promote(i, 52, "NPC", &world).unwrap();
            assert_eq!(
                rating,
                goat_core::derive::ovr(&before.current, before.primary_position)
                    .to_int()
                    .clamp(1, 99) as u8
            );
            assert!(before
                .current
                .iter()
                .zip(before.potential)
                .all(|(c, p)| *c <= p));
            pop.club[i] = (pop.club[i] + 1) % world.clubs.len() as u16;
            let after = pop.promote(i, 52, "NPC", &world).unwrap();
            assert_eq!(before.potential, after.potential);
            assert_eq!(before.current, after.current);
        }
        let rebuilt = genesis_shared(42, &world);
        assert_eq!(pop.current_ovr(0, 520), rebuilt.current_ovr(0, 520));
    }

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
    fn promoted_player_matches_stored_background_potential() {
        // Regression for the promote-ceiling leak: promotion must honour the headline
        // `potential_ovr` the batch sim has aged this player against since genesis —
        // not re-roll a fresh ceiling (which was silently 99 for everyone).
        for seed in [7u64, 9, 42, 1234] {
            let world = WorldGenesis::generate(seed);
            let pop = genesis(seed, &world);
            for idx in [0usize, 50, 500] {
                let povr = pop.potential_ovr[idx] as i32;
                // Retired-by-then is a legitimate outcome at this horizon, not a
                // ceiling-override bug — skip rather than treat it as a failure.
                let Some(view) = pop.promote(idx, 8 * 52, "Prospect", &world) else {
                    continue;
                };
                let max_pot = (0..NUM_ATTRS)
                    .map(|i| view.potential[i].to_int())
                    .max()
                    .unwrap();
                assert!(
                    max_pot <= povr,
                    "seed {seed} idx {idx}: promoted max potential {max_pot} inflates past \
                     stored background potential_ovr {povr}"
                );
                // And it must not collapse either: with the ceiling overridden to povr,
                // the player's best attribute sits within the key-tier band of it.
                assert!(
                    max_pot >= povr * 6 / 10,
                    "seed {seed} idx {idx}: promoted max potential {max_pot} collapsed far \
                     below stored potential_ovr {povr}"
                );
            }
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
            pop.form.push(50);
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
            pop.form.push(50);
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
            pop.form.push(50);
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

    // ── PA2 lineup / selection tests (local line, ported to WorldGenesis) ─────

    #[test]
    fn lineup_picks_highest_ovr_and_skips_retired() {
        let world = WorldGenesis::generate(7);
        let pop = genesis(7, &world);
        let lineup = pop.lineup_indices(3, 260, 11);
        assert_eq!(lineup.len(), 11);
        // Every picked player outranks every unpicked squad mate.
        let min_picked = lineup
            .iter()
            .map(|&i| pop.current_ovr(i, 260))
            .min()
            .unwrap();
        for i in 0..pop.len() {
            if pop.club[i] as usize == 3 && !lineup.contains(&i) && !pop.is_retired(i, 260) {
                assert!(pop.current_ovr(i, 260) <= min_picked);
            }
        }
        // Late enough that the oldest squad members are retired → they can't be picked.
        let late = pop.lineup_indices(3, 22 * 52, 11);
        for &i in &late {
            assert!(!pop.is_retired(i, 22 * 52));
        }
    }

    #[test]
    fn squad_avg_attrs_deterministic_and_pc_lifts_profile() {
        let world = WorldGenesis::generate(5);
        let pop = genesis(5, &world);
        let a = pop.squad_avg_attrs(2, 260, &world, None).unwrap();
        let b = pop.squad_avg_attrs(2, 260, &world, None).unwrap();
        assert_eq!(a, b, "lineup attrs must be deterministic");
        // A superstar PC in the side raises every group mean.
        let star = [Fixed::from_int(95); NUM_ATTRS];
        let with_pc = pop.squad_avg_attrs(2, 260, &world, Some(&star)).unwrap();
        assert!(with_pc[2] > a[2], "PC attrs must lift the squad mean");
    }

    fn pc_input(position: u8, role_rating: i32) -> PcSelectionInput {
        PcSelectionInput {
            position,
            role_rating,
            familiarity_tier: 3, // Natural
            form: 50,
            trust: 50,
            favor: 50,
            academy_hype: 0,
            first_season_at_club: false,
            wage_annual: 0,
            club_strength: 50,
            fan_rep: 50,
            marketability: 50,
            power_ladder: 0,
            energy: 100,
            unavailable: false,
        }
    }

    #[test]
    fn formation_lineup_respects_slots() {
        let world = WorldGenesis::generate(7);
        let pop = genesis(7, &world);
        let slots = (5, 4, 1);
        let lineup = pop.lineup_indices_formation(3, 260, slots);
        assert_eq!(lineup.len(), 10, "10 outfield slots; the GK is abstract");
        for pos in 0..3u8 {
            let want = [slots.0, slots.1, slots.2][pos as usize];
            let got = lineup.iter().filter(|&&i| pop.position[i] == pos).count();
            assert_eq!(got, want, "position {pos} must fill exactly its slots");
        }
        // Deterministic.
        assert_eq!(lineup, pop.lineup_indices_formation(3, 260, slots));
    }

    #[test]
    fn formation_avg_pc_occupies_his_group_slot() {
        let world = WorldGenesis::generate(5);
        let pop = genesis(5, &world);
        let slots = (4, 3, 3);
        let without = pop
            .squad_avg_attrs_formation(2, 260, &world, slots, None)
            .unwrap();
        let star = [Fixed::from_int(95); NUM_ATTRS];
        let with_pc = pop
            .squad_avg_attrs_formation(2, 260, &world, slots, Some((2, &star)))
            .unwrap();
        assert!(
            with_pc[2] > without[2],
            "a starting PC still lifts the squad mean under formation lineups"
        );
    }

    #[test]
    fn star_pc_always_starts_at_weak_club() {
        let world = WorldGenesis::generate(7);
        let pop = genesis(7, &world);
        // Club 0's players cap near its strength; a 95-rated PC clears them all.
        let pc = pc_input(2, 95);
        for week in 0..40u64 {
            let outcome = pop.select_pc(0, week as u32 * 7, 3, &pc, 0xA11CE ^ week);
            assert_eq!(outcome, SelectionOutcome::Starts, "week {week}");
        }
    }

    #[test]
    fn hopeless_pc_is_benched_at_strong_club() {
        let world = WorldGenesis::generate(7);
        let pop = genesis(7, &world);
        // A 20-rated PC at the strongest club should essentially never start.
        let strong_club = (0..world.clubs.len())
            .max_by_key(|&c| world.clubs[c].strength)
            .unwrap();
        let pc = pc_input(1, 20);
        let mut starts = 0;
        for week in 0..60u64 {
            if pop.select_pc(strong_club, week as u32 * 7, 4, &pc, 0xA11CE ^ week)
                == SelectionOutcome::Starts
            {
                starts += 1;
            }
        }
        assert!(
            starts <= 6,
            "20-rated PC started {starts}/60 at an elite club"
        );
    }

    #[test]
    fn selection_is_deterministic_and_unavailable_is_hard() {
        let world = WorldGenesis::generate(3);
        let pop = genesis(3, &world);
        let pc = pc_input(0, 60);
        let a = pop.select_pc(4, 260, 4, &pc, 0xBEEF);
        let b = pop.select_pc(4, 260, 4, &pc, 0xBEEF);
        assert_eq!(a, b, "same week ⇒ same decision");
        let mut injured = pc;
        injured.unavailable = true;
        assert_eq!(
            pop.select_pc(4, 260, 4, &injured, 0xBEEF),
            SelectionOutcome::Benched,
            "injury/suspension is a hard exclusion"
        );
    }

    #[test]
    fn npc_weekly_unavailability_is_rare() {
        // ~1-in-33 per player-week: over 25 players × 200 weeks the outage count
        // must land in a sane band around 150 (3%).
        let world = WorldGenesis::generate(7);
        let pop = genesis(7, &world);
        let mut out = 0u32;
        let mut total = 0u32;
        for week in 0..200u64 {
            let week_seed = 0xA11CE ^ week;
            for i in 0..pop.len() {
                if pop.club[i] as usize != 3 {
                    continue;
                }
                total += 1;
                let mut rng = GoatRng::new(pop.seed[i] ^ week_seed);
                if rng.next_range_u32(0, NPC_UNAVAILABLE_DIV) == 0 {
                    out += 1;
                }
            }
        }
        let pct = out * 100 / total;
        assert!((1..=6).contains(&pct), "NPC outage {pct}% (target ~3%)");
    }
}
