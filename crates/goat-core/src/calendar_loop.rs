//! Live calendar integration (TASK-3.5, Option A — golden-safe).
//!
//! The `CalendarEngine` drives day-tick progression and surfaces calendar flashpoints
//! (transfer windows, international breaks). It runs on its OWN forked RNG stream
//! (`CalendarEngine::calendar_rng`, seeded from `world_seed`), entirely independent of
//! the `week.rs` growth RNG — so attribute golden values are never perturbed. This is
//! what makes wiring the calendar into the live loop golden-safe: the week's growth /
//! injury / decay math is untouched; the calendar only progresses time and reports
//! windows.

use goat_calendar::{
    CalendarEngine, CalendarWindow, Competition, CompetitionId, CompetitionKind, DayContext,
    DayReport, Fixture, Season, StopClass, Subsystem, SubsystemId, WindowKind,
};
use goat_rng::RngSource;

/// Fixed-length in-game year (§2.3 — no leap years).
pub const SEASON_DAYS: u32 = 365;

/// The PC's league competition id. `goat-core` stays headless (no `goat-world`
/// dependency — see that crate's own doc comment), so this is the one fixed id the
/// bridge (the TUI, which links `goat-core` and `goat-world` together) must use when it
/// builds the season's `Fixture`s to hand into `Intent::StartSeason`.
pub const LEAGUE_COMPETITION_ID: CompetitionId = 1;

/// The competitions active this season. Slice 1: just the league; sibling slices append
/// domestic-cup/continental/national-team competitions to this same list.
fn standard_competitions() -> Vec<Competition> {
    vec![Competition {
        id: LEAGUE_COMPETITION_ID,
        kind: CompetitionKind::League,
        priority: 100,
        is_orbit: true,
    }]
}

/// A calendar flashpoint surfaced to the renderer (template + slot; the TUI formats it).
/// Produced when a calendar window opens during the week.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarFlashpoint {
    /// Season-relative epoch day the window opened on.
    pub day: u32,
    pub window: WindowKind,
}

/// The standard season window layout. Placeholder shape; the day ranges mirror the
/// frozen sample in `goat-calendar`'s own clock tests (international break, winter and
/// summer transfer windows). Tuning belongs to a later pass.
fn standard_season() -> Season {
    Season {
        id: 1,
        start_day: 0,
        end_day: SEASON_DAYS - 1,
        windows: vec![
            CalendarWindow {
                kind: WindowKind::InternationalBreak,
                start_day: 30,
                end_day: 44,
            },
            CalendarWindow {
                kind: WindowKind::TransferWinter,
                start_day: 160,
                end_day: 195,
            },
            CalendarWindow {
                kind: WindowKind::TransferSummer,
                start_day: 330,
                end_day: 364,
            },
        ],
        competition_ids: vec![LEAGUE_COMPETITION_ID],
    }
}

/// A minimal subsystem that reports a soft flashpoint on any day a calendar window is
/// active. Registered so each `tick_one_day` exercises the real subsystem ABI; the
/// structured flashpoint (which window, which day) is derived in `advance_calendar_week`
/// from the window's start day so it fires exactly once, on entry.
struct WindowWatch;

impl Subsystem for WindowWatch {
    fn on_day(&mut self, ctx: &DayContext, _rng: &mut dyn RngSource) -> DayReport {
        if ctx.active_windows.is_empty() {
            DayReport::silent(SubsystemId::Media)
        } else {
            DayReport {
                source: SubsystemId::Media,
                stop_class: StopClass::SoftFlashpoint,
                payload: None,
                mutations: vec![],
            }
        }
    }

    fn id(&self) -> SubsystemId {
        SubsystemId::Media
    }
}

/// Advance the calendar by 7 days (one week) from `epoch_day`, returning the new epoch
/// day and any window-entry flashpoints. `world_seed` seeds the calendar's own RNG —
/// it never touches the growth RNG, so this is golden-safe.
///
/// `orbit_fixtures` is the PC's current-season fixture list (league fixtures this
/// slice; cup/continental/national-team fixtures once sibling slices ship), built by the
/// TUI bridge (see `goat_calendar::Fixture`/`LEAGUE_COMPETITION_ID`) and threaded in via
/// `Intent::StartSeason` — `goat-core` never constructs these itself, since it stays
/// headless with respect to `goat-world`.
///
/// Note: this constructs a fresh, throwaway `CalendarEngine` every call (one per week),
/// seeded each time from the SAME immutable `orbit_fixtures` baseline. A conflict-
/// resolution reschedule that lands outside the current 7-day window is therefore not
/// carried forward to the next call — harmless for a single-competition (league-only)
/// season, since a club is never double-booked against itself, but something a sibling
/// slice threading a second orbit competition through here will need to address (see
/// the slice1 task doc's final report for detail).
pub fn advance_calendar_week(
    epoch_day: u32,
    world_seed: u64,
    orbit_fixtures: &[Fixture],
) -> (u32, Vec<CalendarFlashpoint>) {
    let season = standard_season();
    let windows = season.windows.clone();
    let start = epoch_day % SEASON_DAYS;

    let mut engine = CalendarEngine::new(
        world_seed,
        season,
        orbit_fixtures.to_vec(),
        standard_competitions(),
    );
    engine.clock.epoch_day = start;
    engine.register(Box::new(WindowWatch));

    let mut flashpoints = Vec::new();
    for i in 0..7u32 {
        // Season-relative day, wrapping at the year boundary (the engine clock itself
        // does not wrap; we own the canonical day so detection survives season rollover).
        let day = (start + i) % SEASON_DAYS;
        let _reports = engine.tick_one_day(); // drives the subsystem on the calendar's RNG
        if let Some(w) = windows.iter().find(|w| w.start_day == day) {
            flashpoints.push(CalendarFlashpoint {
                day,
                window: w.kind,
            });
        }
    }

    (epoch_day + 7, flashpoints)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_fire_once_on_entry_over_a_season() {
        let mut day = 0u32;
        let mut seen: Vec<(u32, WindowKind)> = Vec::new();
        // 53 weeks ≈ one full 365-day season plus a little, so each window opens once.
        for _ in 0..53 {
            let (next, fps) = advance_calendar_week(day, 42, &[]);
            for f in fps {
                seen.push((f.day, f.window));
            }
            day = next;
        }
        assert!(seen.contains(&(30, WindowKind::InternationalBreak)));
        assert!(seen.contains(&(160, WindowKind::TransferWinter)));
        assert!(seen.contains(&(330, WindowKind::TransferSummer)));
    }

    #[test]
    fn epoch_day_advances_seven_per_week() {
        let (next, _) = advance_calendar_week(100, 1, &[]);
        assert_eq!(next, 107);
    }
}
