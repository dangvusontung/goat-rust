//! `TrainingSubsystem` — implements the `Subsystem` trait from goat-calendar.
//!
//! On each day tick:
//!   - Match day   → silent, match subsystem owns load (out of scope)
//!   - Injured     → energy recovery, silent
//!   - Rest routine → energy recovery, silent
//!   - Training day → compute growth, spend energy, detect events, emit DayReport
//!
//! RNG: takes exactly 1 roll from the calendar's `rng` per day (unconditionally) to
//! seed a training-specific `GoatRng`. This keeps the calendar's RNG advancing by a
//! fixed 1 per subsystem per day regardless of the day type — predictable and
//! deterministic (§9). Training's internal variance never bleeds into other subsystems.

use goat_calendar::subsystem::{DayContext, DayReport, StopClass, Subsystem, SubsystemId};
use goat_core::attrs::{AttrId, ATTR_ARCHETYPES};
use goat_core::player::{PlayerId, PlayerStore, PlayerView};
use goat_fixed::Fixed;
use goat_rng::{GoatRng, RngSource};

use crate::growth::{compute_growth, detect_event};
use crate::models::{AttrTarget, DevelopmentInput, TrainingEvent, TrainingRoutine};
use crate::tuning::{
    ENERGY_COST_HARD, ENERGY_COST_LIGHT, ENERGY_COST_MODERATE, ENERGY_MAX, ENERGY_RECOVERY_MATCH,
    ENERGY_RECOVERY_REST, TRAINING_DOMAIN_HASH,
};

/// Day-by-day training subsystem for the orbit player.
///
/// Owns the orbit player's `PlayerStore` for Phase 1 (only orbit players store
/// per-day energy and attribute state; background players are formula-driven §7.1).
/// Phase 2 will lift this to shared state when multiple subsystems need the same data.
pub struct TrainingSubsystem {
    /// Orbit player's attribute + energy data. Directly mutated each day.
    pub players: PlayerStore,
    /// Which player in `players` is the orbit (PC) player.
    pub orbit_id: PlayerId,
    /// Current standing routine. Updated via `set_routine`.
    routine: TrainingRoutine,
    /// Facility/coach development multiplier (default 1.0; club model out of scope).
    pub facility_mult: Fixed,
}

impl TrainingSubsystem {
    /// Create the subsystem from an existing `PlayerView`.
    ///
    /// `facility_mult` should be 1.0 until the club model (Phase 2) provides a value.
    pub fn new(player: PlayerView, routine: TrainingRoutine, facility_mult: Fixed) -> Self {
        let mut players = PlayerStore::new();
        let orbit_id = players.push(player);
        Self {
            players,
            orbit_id,
            routine,
            facility_mult,
        }
    }

    /// Replace the standing routine.
    pub fn set_routine(&mut self, routine: TrainingRoutine) {
        self.routine = routine;
    }

    /// Read-only snapshot of the orbit player's current energy.
    pub fn energy(&self) -> Fixed {
        self.players.get_energy(self.orbit_id)
    }

    /// Read-only current value for a single attribute.
    pub fn get_current(&self, attr: AttrId) -> Fixed {
        self.players.get_current(self.orbit_id, attr as usize)
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    fn spend_energy(&mut self, intensity: crate::models::Intensity) {
        let cost = match intensity {
            crate::models::Intensity::Light => ENERGY_COST_LIGHT,
            crate::models::Intensity::Moderate => ENERGY_COST_MODERATE,
            crate::models::Intensity::Hard => ENERGY_COST_HARD,
        };
        let e = self.players.get_energy(self.orbit_id);
        self.players
            .set_energy(self.orbit_id, (e - cost).clamp(Fixed::ZERO, ENERGY_MAX));
    }

    fn recover_energy(&mut self, amount: Fixed) {
        let e = self.players.get_energy(self.orbit_id);
        self.players
            .set_energy(self.orbit_id, (e + amount).clamp(Fixed::ZERO, ENERGY_MAX));
    }
}

impl Subsystem for TrainingSubsystem {
    fn id(&self) -> SubsystemId {
        SubsystemId::Training
    }

    fn on_day(&mut self, ctx: &DayContext, rng: &mut dyn RngSource) -> DayReport {
        // Consume exactly 1 roll from the calendar's RNG per day to seed a
        // training-specific day RNG. This keeps the calendar stream advancing
        // by exactly 1 per subsystem per day, regardless of branch taken (§9).
        let day_base = rng.next_u64();
        let mut training_rng = GoatRng::new(day_base ^ TRAINING_DOMAIN_HASH);

        let orbit_id = self.orbit_id;

        // ── Match day: training yields nothing; match subsystem owns load ─────
        if !ctx.todays_fixtures.is_empty() {
            // Recover a small amount of energy (placeholder until match subsystem lands).
            self.recover_energy(ENERGY_RECOVERY_MATCH);
            return DayReport::silent(SubsystemId::Training);
        }

        // ── Injured: forced rest, energy recovery ─────────────────────────────
        let injury_weeks = self.players.get_injury_weeks(orbit_id);
        if injury_weeks > 0 {
            self.recover_energy(ENERGY_RECOVERY_REST);
            return DayReport::silent(SubsystemId::Training);
        }

        // ── Pure rest day: no routine target ─────────────────────────────────
        if self.routine.is_rest() {
            self.recover_energy(ENERGY_RECOVERY_REST);
            return DayReport::silent(SubsystemId::Training);
        }

        // ── Training day ──────────────────────────────────────────────────────
        let target_attr = match self.routine.target {
            Some(AttrTarget::Single(attr)) => attr,
            // AttrTarget::Family — Phase 2. Only Single is implemented.
            None => unreachable!("checked is_rest() above"),
        };

        let a = target_attr as usize;
        let archetype = ATTR_ARCHETYPES[a];
        let age_days = self.players.get_age_weeks(orbit_id) * 7;
        let current = self.players.get_current(orbit_id, a);
        let potential = self.players.get_potential(orbit_id, a);
        let energy = self.players.get_energy(orbit_id);

        let input = DevelopmentInput {
            attr_archetype: archetype,
            age_days,
            current,
            potential,
            intensity: self.routine.intensity,
            energy,
            facility_mult: self.facility_mult,
        };

        let delta = compute_growth(&input, &mut training_rng);

        // Apply attribute growth. Ceiling clamp via goat-core's set_current (§2.4):
        // set_current enforces current <= potential in debug builds (debug_assert).
        // We also clamp with Fixed::clamp to be safe in release builds.
        if delta > Fixed::ZERO {
            let new_val = (current + delta).clamp(Fixed::MIN_ATTR, potential);
            self.players.set_current(orbit_id, a, new_val);
        }

        // Detect event BEFORE spending energy (energy reflects start-of-day state).
        let after = current + delta;
        let event = detect_event(current, after, energy, self.routine.intensity);

        // Spend energy.
        self.spend_energy(self.routine.intensity);

        // Map event to stop class and payload.
        let stop_class = match event {
            Some(TrainingEvent::Breakthrough) | Some(TrainingEvent::Overtrained) => {
                StopClass::SoftFlashpoint
            }
            None => StopClass::Silent,
        };
        let payload = event.map(|e| match e {
            TrainingEvent::Breakthrough => format!(
                "Breakthrough: {} reached {}",
                goat_core::attrs::ATTR_NAMES[a],
                (current + delta).to_int()
            ),
            TrainingEvent::Overtrained => "Overtrained: energy critically low".into(),
        });

        DayReport {
            source: SubsystemId::Training,
            stop_class,
            payload,
            mutations: vec![],
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Intensity;
    use goat_core::attrs::AttrId;
    use goat_core::player::PlayerView;
    use goat_core::roles::{FamiliarityTier, NUM_ROLES};
    use goat_fixed::Fixed;
    use goat_rng::GoatRng;

    fn make_player(current: i32, potential: i32, energy: i32) -> PlayerView {
        let mut view = PlayerView::default();
        view.name = "TestPlayer".into();
        for a in 0..goat_core::attrs::NUM_ATTRS {
            view.current[a] = Fixed::from_int(current);
            view.potential[a] = Fixed::from_int(potential);
        }
        view.familiarity = [FamiliarityTier::Awkward; NUM_ROLES];
        view.age_weeks = 17 * 52; // 17-year-old
        view.energy = Fixed::from_int(energy);
        view.injury_weeks = 0;
        view
    }

    fn make_ctx(epoch_day: u32, is_match: bool) -> DayContext {
        use goat_calendar::clock::Fixture;
        DayContext {
            epoch_day,
            season_id: 1,
            todays_fixtures: if is_match {
                vec![Fixture {
                    id: 0,
                    competition_id: 1,
                    scheduled_day: epoch_day,
                    original_day: epoch_day,
                    is_orbit: true,
                }]
            } else {
                vec![]
            },
            active_windows: vec![],
            days_until_next_fixture: if is_match { 0 } else { 7 },
            congestion_score: 0,
        }
    }

    // AC-03: Energy decreases on training days.
    #[test]
    fn energy_decreases_on_training_day() {
        let player = make_player(60, 85, 100);
        let routine = TrainingRoutine {
            target: Some(AttrTarget::Single(AttrId::Finishing)),
            intensity: Intensity::Moderate,
        };
        let mut sub = TrainingSubsystem::new(player, routine, Fixed::ONE);
        let mut cal_rng = GoatRng::new(1);

        let before = sub.energy();
        sub.on_day(&make_ctx(0, false), &mut cal_rng);
        let after = sub.energy();

        assert!(
            after < before,
            "energy must decrease after training: {before:?} → {after:?}"
        );
    }

    // AC-03: Energy recovers on rest days.
    #[test]
    fn energy_recovers_on_rest_day() {
        let player = make_player(60, 85, 50); // start at 50 energy
        let routine = TrainingRoutine::rest();
        let mut sub = TrainingSubsystem::new(player, routine, Fixed::ONE);
        let mut cal_rng = GoatRng::new(1);

        let before = sub.energy();
        sub.on_day(&make_ctx(0, false), &mut cal_rng);
        let after = sub.energy();

        assert!(
            after > before,
            "energy must recover on rest: {before:?} → {after:?}"
        );
    }

    // AC-03: Energy stays in [0, 100] over many days.
    #[test]
    fn energy_stays_in_valid_range() {
        let player = make_player(30, 99, 90);
        let routine = TrainingRoutine {
            target: Some(AttrTarget::Single(AttrId::SprintSpeed)),
            intensity: Intensity::Hard,
        };
        let mut sub = TrainingSubsystem::new(player, routine, Fixed::ONE);
        let mut cal_rng = GoatRng::new(99);

        for day in 0u32..200 {
            sub.on_day(&make_ctx(day, false), &mut cal_rng);
            let e = sub.energy();
            assert!(e >= Fixed::ZERO, "energy below 0 on day {day}: {e:?}");
            assert!(
                e <= Fixed::from_int(100),
                "energy above 100 on day {day}: {e:?}"
            );
        }
    }

    // AC-01: current never exceeds potential after many days.
    #[test]
    fn current_never_exceeds_potential() {
        let player = make_player(60, 85, 90);
        let routine = TrainingRoutine {
            target: Some(AttrTarget::Single(AttrId::Finishing)),
            intensity: Intensity::Moderate,
        };
        let mut sub = TrainingSubsystem::new(player, routine, Fixed::ONE);
        let mut cal_rng = GoatRng::new(7);

        for day in 0u32..500 {
            sub.on_day(&make_ctx(day, false), &mut cal_rng);
        }
        for a in 0..goat_core::attrs::NUM_ATTRS {
            let cur = sub.players.get_current(sub.orbit_id, a);
            let pot = sub.players.get_potential(sub.orbit_id, a);
            assert!(
                cur <= pot,
                "current ({cur:?}) > potential ({pot:?}) for attr {a}"
            );
        }
    }

    // Match day: no growth, energy partially recovers.
    #[test]
    fn match_day_yields_no_growth() {
        let player = make_player(60, 85, 90);
        let routine = TrainingRoutine {
            target: Some(AttrTarget::Single(AttrId::Finishing)),
            intensity: Intensity::Hard,
        };
        let mut sub = TrainingSubsystem::new(player, routine, Fixed::ONE);
        let mut cal_rng = GoatRng::new(1);

        let before = sub.get_current(AttrId::Finishing);
        let report = sub.on_day(&make_ctx(0, true), &mut cal_rng);
        let after = sub.get_current(AttrId::Finishing);

        assert_eq!(report.stop_class, StopClass::Silent);
        assert_eq!(before, after, "no attr growth on match days");
    }

    // Growth halts once potential is reached.
    #[test]
    fn growth_halts_at_ceiling() {
        let mut player = make_player(84, 85, 100);
        // Set Finishing near its ceiling.
        player.current[AttrId::Finishing as usize] = Fixed::raw(84_900); // 84.9
        player.potential[AttrId::Finishing as usize] = Fixed::from_int(85);
        let routine = TrainingRoutine {
            target: Some(AttrTarget::Single(AttrId::Finishing)),
            intensity: Intensity::Hard,
        };
        let mut sub = TrainingSubsystem::new(player, routine, Fixed::ONE);
        let mut cal_rng = GoatRng::new(5);

        // Train many days — should pin at potential.
        for day in 0u32..100 {
            sub.on_day(&make_ctx(day, false), &mut cal_rng);
        }

        let final_val = sub.get_current(AttrId::Finishing);
        let potential = sub
            .players
            .get_potential(sub.orbit_id, AttrId::Finishing as usize);
        assert!(
            final_val <= potential,
            "Finishing ({final_val:?}) must not exceed potential ({potential:?})"
        );
    }
}
