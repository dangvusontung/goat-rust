//! The calendar integration point: `impl Subsystem for Training`
//! (docs/TRAINING.md §Core Loop, appendix Step 3).
//!
//! Phase-1 realities (kept explicit, per lib.rs):
//! - The subsystem owns its orbit-player state (attrs/energy/age) — `DayContext`
//!   exposes no player access and `StateMutation` is a `NoOp` placeholder, so
//!   `mutations` stays empty until the wiring round.
//! - RNG comes from a stream forked ONCE at construction (`fork("training")`);
//!   the `rng` argument the engine passes (today: the calendar's own stream) is
//!   deliberately never drawn from.

use crate::growth::{compute_growth, recover_energy, spend_energy};
use crate::models::{AttrTarget, DevelopmentInput, TrainingEvent, TrainingRoutine};
use crate::tuning::*;
use goat_calendar::rng_stream::RngStream;
use goat_calendar::subsystem::{DayContext, DayReport, StopClass, Subsystem, SubsystemId};
use goat_core::attrs::{ATTR_ARCHETYPES, ATTR_NAMES, NUM_ATTRS};
use goat_fixed::Fixed;
use goat_rng::{GoatRng, RngSource};

/// The training subsystem. Phase 1: single-attribute routine, energy,
/// Breakthrough/Overtrained soft flashpoints.
pub struct Training {
    current: [Fixed; NUM_ATTRS],
    potential: [Fixed; NUM_ATTRS],
    age_days: u32,
    energy: Fixed,
    /// Injury days remaining — injured days rest and recover (no growth).
    injury_days: u32,
    /// `None` = standing rest instruction (a pure rest day when ticked).
    routine: Option<TrainingRoutine>,
    /// Club-model development multiplier; 1.0 until the club model exists (Phase 2).
    facility_mult: Fixed,
    /// This subsystem's own stream — forked once at construction, never shared.
    rng: GoatRng,
}

impl Training {
    /// `save_seed` is forked immediately into the training domain; the player's
    /// current/potential arrays are taken by value (Phase 1 owns its copy).
    pub fn new(
        save_seed: u64,
        current: [Fixed; NUM_ATTRS],
        potential: [Fixed; NUM_ATTRS],
        age_days: u32,
    ) -> Self {
        let rng = RngStream::new(save_seed).fork("training");
        Training {
            current,
            potential,
            age_days,
            energy: ENERGY_MAX,
            injury_days: 0,
            routine: Some(TrainingRoutine::default()),
            facility_mult: Fixed::ONE,
            rng,
        }
    }

    /// Set the standing routine; `None` means rest until told otherwise.
    pub fn set_routine(&mut self, routine: Option<TrainingRoutine>) {
        self.routine = routine;
    }

    pub fn current(&self, attr: goat_core::attrs::AttrId) -> Fixed {
        self.current[attr as usize]
    }

    pub fn energy(&self) -> Fixed {
        self.energy
    }

    pub fn age_days(&self) -> u32 {
        self.age_days
    }

    /// Test/wiring hook: put the player on the injury list for `days`.
    pub fn set_injured(&mut self, days: u32) {
        self.injury_days = days;
    }
}

/// Decide whether today produced a surfacing-worthy event (US-05). At most one
/// event per day; a warning (Overtrained) outranks good news (Breakthrough).
fn detect_event(before: Fixed, after: Fixed, energy_before: Fixed) -> Option<TrainingEvent> {
    if energy_before.to_raw() < OVERTRAINED_ENERGY_RAW {
        return Some(TrainingEvent::Overtrained);
    }
    let crossed =
        before.to_raw() / BREAKTHROUGH_MILESTONE_RAW < after.to_raw() / BREAKTHROUGH_MILESTONE_RAW;
    if crossed {
        return Some(TrainingEvent::Breakthrough);
    }
    None
}

impl Subsystem for Training {
    fn id(&self) -> SubsystemId {
        SubsystemId::Training
    }

    fn on_day(&mut self, ctx: &DayContext, _calendar_rng: &mut dyn RngSource) -> DayReport {
        // Age advances every day, whatever kind of day it is.
        self.age_days += 1;

        // Match day: training yields nothing — the match owns the day's load
        // (match-fatigue belongs to the match subsystem, later phase).
        if !ctx.todays_fixtures.is_empty() {
            return DayReport::silent(SubsystemId::Training);
        }

        // Injured: rest and recover only.
        if self.injury_days > 0 {
            self.injury_days -= 1;
            self.energy = recover_energy(self.energy);
            return DayReport::silent(SubsystemId::Training);
        }

        // Standing rest instruction: recover, no growth (AC-03).
        let routine = match self.routine {
            Some(r) => r,
            None => {
                self.energy = recover_energy(self.energy);
                return DayReport::silent(SubsystemId::Training);
            }
        };

        // Training day.
        let AttrTarget::Single(attr) = routine.target;
        let a = attr as usize;
        let energy_before = self.energy;
        let before = self.current[a];
        let input = DevelopmentInput {
            attr_archetype: ATTR_ARCHETYPES[a],
            age_days: self.age_days,
            current: before,
            potential: self.potential[a],
            intensity: routine.intensity,
            energy: energy_before,
            facility_mult: self.facility_mult,
        };
        let delta = compute_growth(&input, &mut self.rng);
        // Ceiling clamp via goat-core's exact idiom (§2.4 — never exceed potential).
        self.current[a] = (before + delta).clamp(Fixed::MIN_ATTR, self.potential[a]);
        self.energy = spend_energy(self.energy, routine.intensity);

        let event = detect_event(before, self.current[a], energy_before);
        match event {
            Some(TrainingEvent::Breakthrough) => DayReport {
                source: SubsystemId::Training,
                stop_class: StopClass::SoftFlashpoint,
                payload: Some(format!(
                    "Breakthrough! {} reaches {}.",
                    ATTR_NAMES[a],
                    self.current[a].to_int()
                )),
                mutations: vec![],
            },
            Some(TrainingEvent::Overtrained) => DayReport {
                source: SubsystemId::Training,
                stop_class: StopClass::SoftFlashpoint,
                payload: Some(format!(
                    "Overtrained — gains suffer at {}% energy.",
                    energy_before.to_int()
                )),
                mutations: vec![],
            },
            _ => DayReport::silent(SubsystemId::Training),
        }
    }
}
