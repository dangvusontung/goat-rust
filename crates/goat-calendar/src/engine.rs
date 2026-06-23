//! CalendarEngine — the time-orchestration spine (§2.2, §2.3, §9).
//!
//! Single mutation point: `tick_one_day`. Every advance mode composes it.
//! Determinism rules enforced here:
//!   - No wall-clock reads.
//!   - RNG only via the injected `calendar_rng` (domain-forked from root seed).
//!   - Subsystems polled in a fixed `Vec` order — never a HashMap iteration.
//!   - `epoch_day` mutated in exactly one place.

use crate::clock::{Fixture, GameClock, Season, SeasonId, WindowKind};
use crate::rng_stream::RngStream;
use crate::subsystem::{DayContext, DayReport, StopClass, Subsystem};
use crate::tuning::SOFT_FLUSH_THRESHOLD;
use goat_rng::GoatRng;

/// Returned to the renderer whenever the advance loop halts.
#[derive(Debug)]
pub struct StopResult {
    /// The epoch-day on which the event fired (pre-tick; this is "today" for the player).
    pub day: u32,
    /// Reports that caused the halt (HardStops or flushed SoftFlashpoints).
    pub stops: Vec<DayReport>,
    /// Buffered SoftFlashpoints flushed alongside a HardStop.
    pub pending: Vec<DayReport>,
}

/// Owns the clock, current season, orbit fixtures, and subsystem registry.
///
/// The renderer creates this engine, registers subsystems, then drives it
/// by calling one of the advance modes. Core never touches a renderer type.
pub struct CalendarEngine {
    pub clock: GameClock,
    pub season: Season,
    /// Current-season orbit fixtures, sorted by scheduled_day for deterministic iteration.
    pub fixtures: Vec<Fixture>,
    /// Registered subsystems in ABI-locked order (SIM_VERSION).
    /// Use a `Vec`, never a `HashMap` — iteration order is load-bearing.
    subsystems: Vec<Box<dyn Subsystem>>,
    /// Calendar's own RNG stream, forked once at construction from the root seed.
    /// Independent of match, transfer, and injury streams (§9).
    calendar_rng: GoatRng,
}

impl CalendarEngine {
    /// Create the engine. `root_seed` is the save seed; the calendar immediately forks
    /// its independent stream from it so subsequent forks (match, transfer, …) don't
    /// affect the calendar's randomness.
    pub fn new(root_seed: u64, season: Season, mut fixtures: Vec<Fixture>) -> Self {
        let mut root = RngStream::new(root_seed);
        let calendar_rng = root.fork("calendar");
        // Sort by scheduled_day so iteration order is deterministic.
        fixtures.sort_by_key(|f| f.scheduled_day);
        let season_id = season.id;
        CalendarEngine {
            clock: GameClock::new(season_id),
            season,
            fixtures,
            subsystems: vec![],
            calendar_rng,
        }
    }

    /// Register a subsystem. Must be called in the ABI-locked order from SIM_VERSION.
    /// Appended to the Vec; do not insert at arbitrary positions.
    pub fn register(&mut self, subsystem: Box<dyn Subsystem>) {
        self.subsystems.push(subsystem);
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    fn fixtures_for_day(&self, day: u32) -> Vec<Fixture> {
        // Iterate Vec (sorted), never HashMap — order is deterministic.
        self.fixtures
            .iter()
            .filter(|f| f.is_orbit && f.scheduled_day == day)
            .cloned()
            .collect()
    }

    fn days_until_next_fixture(&self, from_day: u32) -> u32 {
        self.fixtures
            .iter()
            .filter(|f| f.is_orbit && f.scheduled_day > from_day)
            .map(|f| f.scheduled_day - from_day)
            .min()
            .unwrap_or(u32::MAX)
    }

    /// Count of orbit fixtures in the 10-day window starting at `day`.
    /// Named `congestion_score` in the spec (typed double there; u32 here — no floats).
    fn congestion_score(&self, day: u32) -> u32 {
        self.fixtures
            .iter()
            .filter(|f| f.is_orbit && f.scheduled_day >= day && f.scheduled_day <= day + 10)
            .count() as u32
    }

    fn active_windows(&self, day: u32) -> Vec<WindowKind> {
        self.season
            .windows
            .iter()
            .filter(|w| w.contains(day))
            .map(|w| w.kind)
            .collect()
    }

    fn build_context(&self, day: u32) -> DayContext {
        DayContext {
            epoch_day: day,
            season_id: self.clock.current_season,
            todays_fixtures: self.fixtures_for_day(day),
            active_windows: self.active_windows(day),
            days_until_next_fixture: self.days_until_next_fixture(day),
            congestion_score: self.congestion_score(day),
        }
    }

    /// Season-boundary pipeline — Phase 1 stub.
    ///
    /// Phase 3 will implement the full ordered pipeline (order is load-bearing):
    ///   1. settleSeason  2. awardCeremonies  3. ageTickPopulation
    ///   4. batchTickOuterWorld  5. promoteRelegateClubs  6. openWindow(transferSummer)
    ///   7. genesisFixtures  8. discardFixtures
    fn on_season_boundary(&mut self) {}

    // ── The single clock-mutation point ───────────────────────────────────────

    /// Advance one day: build context, poll subsystems in ABI order, apply mutations,
    /// increment `epoch_day`. This is the ONLY function permitted to mutate `epoch_day`.
    pub fn tick_one_day(&mut self) -> Vec<DayReport> {
        let day = self.clock.epoch_day;
        let ctx = self.build_context(day);

        // Poll subsystems in fixed registration order (Vec, not HashMap).
        // Apply each report's mutations immediately in registration order.
        let mut reports = Vec::with_capacity(self.subsystems.len());
        for sys in &mut self.subsystems {
            let report = sys.on_day(&ctx, &mut self.calendar_rng);
            // Phase 1: StateMutation::NoOp — nothing to apply.
            // Phase 2+: match mutations here and mutate shared sim state.
            reports.push(report);
        }

        // Decrement match-scoped suspension counters for matches played today.
        // Phase 1 stub: SuspensionLedger (per-competition, counts by match not day) added in Phase 2.

        // The ONLY increment of epoch_day.
        self.clock.epoch_day += 1;

        if self.clock.epoch_day > self.season.end_day {
            self.on_season_boundary();
        }

        reports
    }

    // ── Advance modes ─────────────────────────────────────────────────────────

    /// Advance until the first HardStop or until buffered soft flashpoints warrant flushing.
    /// This is the heart of manage-by-exception (§2.2).
    ///
    /// Silent days loop automatically. The player is only woken at moments that matter.
    pub fn advance_until_flashpoint(&mut self) -> StopResult {
        let mut soft_buffer: Vec<DayReport> = Vec::new();
        loop {
            // Guard: stop if we've passed the season end (no more days to tick).
            if self.clock.epoch_day > self.season.end_day {
                return StopResult {
                    day: self.clock.epoch_day,
                    stops: core::mem::take(&mut soft_buffer),
                    pending: vec![],
                };
            }

            let event_day = self.clock.epoch_day;
            let reports = self.tick_one_day();

            let hard: Vec<DayReport> = reports
                .iter()
                .filter(|r| r.stop_class == StopClass::HardStop)
                .cloned()
                .collect();
            let soft: Vec<DayReport> = reports
                .iter()
                .filter(|r| r.stop_class == StopClass::SoftFlashpoint)
                .cloned()
                .collect();

            soft_buffer.extend(soft);

            if !hard.is_empty() {
                // Stop immediately. Flush any buffered softs alongside the hard stops.
                return StopResult {
                    day: event_day,
                    stops: hard,
                    pending: core::mem::take(&mut soft_buffer),
                };
            }

            if should_flush_soft(&soft_buffer, &self.clock) {
                return StopResult {
                    day: event_day,
                    stops: core::mem::take(&mut soft_buffer),
                    pending: vec![],
                };
            }

            // Silent day — loop continues.
        }
    }

    /// Advance up to `max_days`, breaking early on a HardStop when `allow_break = true`.
    /// Used for rest periods, injury layoffs, and bounded skips (§2.2 US-03).
    pub fn advance_bounded(&mut self, max_days: u32, allow_break: bool) -> StopResult {
        let target = self.clock.epoch_day.saturating_add(max_days);
        while self.clock.epoch_day < target {
            let event_day = self.clock.epoch_day;
            let reports = self.tick_one_day();
            if allow_break {
                let hard: Vec<DayReport> = reports
                    .into_iter()
                    .filter(|r| r.stop_class == StopClass::HardStop)
                    .collect();
                if !hard.is_empty() {
                    return StopResult {
                        day: event_day,
                        stops: hard,
                        pending: vec![],
                    };
                }
            }
        }
        StopResult {
            day: self.clock.epoch_day,
            stops: vec![],
            pending: vec![],
        }
    }

    /// Fast-forward a full season without stopping for any flashpoints.
    /// Used by tests and the outer-world batch-tick (§7.1).
    ///
    /// MUST use the same code path as live play — only `auto_resolve` differs.
    /// Two separate code paths for "sim" vs "live" would diverge and make tests meaningless.
    pub fn sim_season_headless(&mut self, season_id: SeasonId) {
        while !self.season_ended(season_id) {
            let reports = self.tick_one_day();
            for r in reports {
                if r.stop_class != StopClass::Silent {
                    auto_resolve(&r);
                }
            }
        }
    }

    fn season_ended(&self, season_id: SeasonId) -> bool {
        self.clock.current_season != season_id || self.clock.epoch_day > self.season.end_day
    }
}

/// Decide whether the buffered soft flashpoints are worth surfacing now.
///
/// TUNABLE — this is the most-iterated function in the system; the real policy is a
/// feel question, not a correctness question (see §2.2 open decisions). Phase 1 uses
/// a simple count threshold (`SOFT_FLUSH_THRESHOLD`). Do not finalise the rule here.
pub fn should_flush_soft(buffer: &[DayReport], _clock: &GameClock) -> bool {
    buffer.len() >= SOFT_FLUSH_THRESHOLD
}

/// Default auto-resolution policy for headless sim: skip matches, keep routines, decline offers.
/// Phase 1: no-op. Phase 2+: dispatch on `r.source` and apply default choices.
fn auto_resolve(_report: &DayReport) {}
