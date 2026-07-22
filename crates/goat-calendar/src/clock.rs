//! Core time types: clock, season, windows, fixtures.
//!
//! All day values are **in-game epoch days** (0 = save start).
//! No wall-clock, no `std::time`, no `chrono`. Pure integer arithmetic.
//! A year is a fixed 365 days — no leap years, no weekday mapping (§2.3 / §9).

pub type SeasonId = u32;
pub type CompetitionId = u32;
/// Deterministic fixture id — hash(seed, competition, season, round, slot).
pub type FixtureId = u64;

/// The single in-game time axis. Only `tick_one_day` may mutate `epoch_day`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameClock {
    /// Days elapsed since save start. Epoch day 0 = first day of career.
    pub epoch_day: u32,
    pub current_season: SeasonId,
}

impl GameClock {
    pub fn new(start_season: SeasonId) -> Self {
        GameClock {
            epoch_day: 0,
            current_season: start_season,
        }
    }
}

/// Calendar window kinds (transfer windows, international breaks, off-season gaps).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKind {
    TransferSummer,
    TransferWinter,
    InternationalBreak,
    OffSeason,
}

/// A blocked/special interval within a season, expressed as inclusive epoch-day range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarWindow {
    pub kind: WindowKind,
    pub start_day: u32,
    pub end_day: u32,
}

impl CalendarWindow {
    pub fn contains(&self, day: u32) -> bool {
        day >= self.start_day && day <= self.end_day
    }
}

/// One football-season frame (e.g. Aug → May + windows + off-season gap).
///
/// A 365-day in-game year means `end_day = start_day + 364` for a full season.
/// Past seasons keep results only; fixtures are regenerated from seed (§9 — tiny saves).
#[derive(Debug, Clone)]
pub struct Season {
    pub id: SeasonId,
    /// Inclusive epoch-day range: [start_day, end_day].
    pub start_day: u32,
    pub end_day: u32,
    pub windows: Vec<CalendarWindow>,
    pub competition_ids: Vec<CompetitionId>,
}

impl Season {
    /// Total days in this season (inclusive both ends).
    pub fn days(&self) -> u32 {
        self.end_day.saturating_sub(self.start_day) + 1
    }

    /// Convert an epoch-day to a season-relative day (0-indexed from start_day).
    pub fn epoch_to_relative(&self, epoch_day: u32) -> u32 {
        epoch_day.saturating_sub(self.start_day)
    }

    /// Convert a season-relative day back to an epoch-day.
    pub fn relative_to_epoch(&self, relative_day: u32) -> u32 {
        self.start_day.saturating_add(relative_day)
    }
}

/// A competition instance (one league, one cup, one continental tier, etc.).
///
/// Phase 2 (this round): the entity fixtures hang off for conflict-resolution priority
/// and legacy-evidence bookkeeping. `id` is referenced by `Fixture::competition_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Competition {
    pub id: CompetitionId,
    pub kind: CompetitionKind,
    /// Conflict-resolution priority; higher wins a same-day clash (see `FixtureImportance`
    /// for the secondary, per-fixture tie-break).
    pub priority: i32,
    /// Relevant to the PC's club/nation this season.
    pub is_orbit: bool,
}

/// The shape of a competition. 7 kinds: the bible's original 4-way split
/// (`league | domesticCup | continental | international`) widened so continental club
/// competition (3 tiers) and international competition (2 distinct trophies) are each
/// individually distinguishable — needed for both conflict-resolution priority and
/// legacy-evidence purposes ("won a World Cup" and "won a continental championship" are
/// different trophies).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompetitionKind {
    League,
    DomesticCup,
    /// Champions-League-equivalent.
    ContinentalTier1,
    /// Europa-equivalent.
    ContinentalTier2,
    /// Conference-equivalent.
    ContinentalTier3,
    WorldCup,
    /// Euro/Copa América/Asian Cup/etc. — one enum value, many nations' instances.
    ContinentalChampionship,
}

/// Same-day conflict-resolution tie-break ladder, lowest to highest — used as the
/// secondary sort key after `Competition::priority` (see `resolveFixturesForDay`,
/// `docs/MAIN.md:1090-1117`). Declaration order IS the ladder order: derived `Ord`
/// compares by discriminant, so `DeadRubber < League < ... < ContinentalTier1Final`.
///
/// National-team fixtures (`WorldCup`/`ContinentalChampionship`) are deliberately absent
/// from this ladder — an international window excludes club fixtures from being
/// scheduled at all (a calendar-level rule), rather than winning a same-day priority
/// contest against them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FixtureImportance {
    DeadRubber,
    League,
    /// A league match, rivalry-flagged.
    Derby,
    ContinentalTier3,
    /// Rounds before the tier-1 entry round.
    DomesticCupEarly,
    ContinentalTier2,
    /// The tier-1-entry round onward, incl. semis.
    DomesticCupLate,
    ContinentalTier1,
    DomesticCupFinal,
    ContinentalTier1Final,
}

/// An orbit fixture: a match relevant to the player's club or nation.
///
/// `scheduled_day` may shift from `original_day` via same-day conflict resolution
/// (`CalendarEngine::resolve_conflicts_for_day`, `docs/MAIN.md:1090-1117`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixture {
    /// Deterministic id = hash(seed, competition, season, round, slot).
    pub id: FixtureId,
    pub competition_id: CompetitionId,
    /// Current scheduled epoch-day (may differ from original_day after reschedule).
    pub scheduled_day: u32,
    /// Original scheduled day — audit trail for conflict resolution.
    pub original_day: u32,
    /// True when this fixture is relevant to the player's club or nation.
    pub is_orbit: bool,
    /// Same-day conflict-resolution tie-break (see `FixtureImportance`).
    pub importance: FixtureImportance,
    /// For two-legged ties (continental knockout, some cup rounds): the other leg's
    /// fixture id. Two-legged ties reschedule together, never independently.
    pub leg_for_id: Option<FixtureId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_relative_day_roundtrips() {
        let season = Season {
            id: 1,
            start_day: 0,
            end_day: 364,
            windows: vec![],
            competition_ids: vec![],
        };
        for day in 0u32..=364 {
            let rel = season.epoch_to_relative(day);
            let back = season.relative_to_epoch(rel);
            assert_eq!(back, day, "round-trip failed at epoch_day {day}");
        }
    }

    #[test]
    fn season_days_is_365_for_full_year() {
        let season = Season {
            id: 1,
            start_day: 0,
            end_day: 364,
            windows: vec![],
            competition_ids: vec![],
        };
        assert_eq!(season.days(), 365);
    }

    #[test]
    fn calendar_window_contains_boundary_values() {
        let w = CalendarWindow {
            kind: WindowKind::TransferSummer,
            start_day: 10,
            end_day: 20,
        };
        assert!(!w.contains(9));
        assert!(w.contains(10));
        assert!(w.contains(15));
        assert!(w.contains(20));
        assert!(!w.contains(21));
    }

    #[test]
    fn sample_season_windows_do_not_overlap() {
        let windows = [
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
        ];
        for i in 0..windows.len() {
            for j in (i + 1)..windows.len() {
                let a = &windows[i];
                let b = &windows[j];
                let overlaps = a.start_day <= b.end_day && b.start_day <= a.end_day;
                assert!(!overlaps, "windows {i} and {j} overlap");
            }
        }
    }
}
