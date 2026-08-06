//! Season calendar — maps league rounds to calendar weeks and real dates.
//!
//! The season runs 38 calendar weeks, the grid anchored to the first Monday
//! on/after 15 August of the season year (weeks run Mon–Sun).
//! Each week has 0, 1, or 2 league matches; 0-match weeks are breaks.
//! All data is static — nothing is stored, everything is recomputed.

use crate::fixtures::ROUNDS_PER_SEASON;

/// Total calendar weeks spanned by one season.
pub const SEASON_CALENDAR_WEEKS: usize = 38;

/// Career base year — season 1 starts in this year.
///
/// Fallback/harness default only (v19+): the live game reads the real year from
/// wall-clock ONCE at new-game (TUI/bridge/web outer layers) and persists it as
/// `WorldState::career_base_year`. This const remains the default for pre-v19
/// saves and for deterministic dev harnesses (career_sim).
pub const BASE_CAREER_YEAR: u32 = 2025;

/// Number of league matches per calendar week (0 = break / rest).
/// Must sum to exactly ROUNDS_PER_SEASON (38, at the confirmed 20-clubs-per-tier scale).
///
/// Layout (3 break weeks kept for flavor — international break, winter break, spring
/// break — the rest of the 38-week grid is filled to carry 38 rounds):
///   wk 0–3  : Aug  (opener, early busy period)
///   wk 4    : Sep  break (international)
///   wk 5–20 : Sep–Jan (steady run, Christmas/NY congestion)
///   wk 21   : Jan  winter break
///   wk 22–31: Jan–Mar (catch-up double-header + steady run)
///   wk 32   : Apr  break
///   wk 33–37: Apr–May (run-in / season tail)
pub const WEEK_MATCH_COUNTS: [u8; SEASON_CALENDAR_WEEKS] = [
    1, 2, 1, 1, 0, 1, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 2, 1, 1, 1, 1, 1, 1, 1, 1,
    0, 1, 1, 1, 1, 1,
];

// Compile-time assertion: WEEK_MATCH_COUNTS must sum to ROUNDS_PER_SEASON.
const _CALENDAR_SUM_CHECK: () = {
    let mut sum = 0usize;
    let mut i = 0;
    while i < SEASON_CALENDAR_WEEKS {
        sum += WEEK_MATCH_COUNTS[i] as usize;
        i += 1;
    }
    assert!(
        sum == ROUNDS_PER_SEASON,
        "WEEK_MATCH_COUNTS must sum to ROUNDS_PER_SEASON"
    );
};

/// Day-of-week offset from Monday for each match slot within a week.
/// Index by match_count then slot index.
///   1 match  → [5]        Saturday only
///   2 matches→ [1, 5]     Tuesday + Saturday
const MATCH_DAY_OFFSETS: [[u8; 2]; 3] = [
    [0, 0], // 0 matches — unused
    [5, 0], // 1 match : Saturday
    [1, 5], // 2 matches: Tuesday + Saturday
];

/// Days from Aug 15 of `season_year` to the season's week-grid anchor: the
/// first Monday on/after Aug 15. Weeks run Mon–Sun, so the MATCH_DAY_OFFSETS
/// land on real Tuesdays/Saturdays in every season year. Without this anchor
/// the grid starts on whatever weekday Aug 15 falls on (a Friday in 2025) and
/// "Saturday" fixtures render as Wednesdays.
fn season_anchor_offset(season_year: u32) -> u32 {
    (7 - weekday_of_aug15(season_year) as u32) % 7
}

// ── Navigation helpers ────────────────────────────────────────────────────────

/// Returns the calendar week (0-indexed) that contains the given round index.
pub fn round_to_week(round: usize) -> usize {
    let mut cumulative: usize = 0;
    for (w, &count) in WEEK_MATCH_COUNTS.iter().enumerate() {
        cumulative += count as usize;
        if cumulative > round {
            return w;
        }
    }
    SEASON_CALENDAR_WEEKS.saturating_sub(1)
}

/// Returns the half-open range of round indices that fall in `week`.
pub fn week_to_rounds(week: usize) -> std::ops::Range<usize> {
    let start: usize = WEEK_MATCH_COUNTS[..week].iter().map(|&c| c as usize).sum();
    let count = WEEK_MATCH_COUNTS.get(week).copied().unwrap_or(0) as usize;
    start..start + count
}

/// True if `week` has no matches (break / rest week).
pub fn is_break_week(week: usize) -> bool {
    WEEK_MATCH_COUNTS.get(week).copied().unwrap_or(0) == 0
}

/// Break/rest calendar weeks skipped when the season advances from `round` to
/// `round + 1` — 0 when the next round is in the same or the adjacent week.
/// Feed this to `Intent::ApplyRoundResult { rest_weeks }` so the player clock
/// tracks the season calendar across break weeks.
pub fn rest_weeks_after_round(round: usize) -> u32 {
    if round + 1 >= ROUNDS_PER_SEASON {
        return 0; // trailing weeks are absorbed by the off-season back-fill
    }
    let w0 = round_to_week(round);
    let w1 = round_to_week(round + 1);
    w1.saturating_sub(w0).saturating_sub(1) as u32
}

/// Day offset from the season's Monday-anchored week 0 (0-indexed, independent of the
/// real-world calendar year — that anchor is a display-only concern layered on top by
/// `match_date`). Exposed standalone so callers that need a season-relative day number
/// (e.g. `goat-calendar`'s `Fixture::scheduled_day`) don't have to duplicate
/// `MATCH_DAY_OFFSETS`.
pub fn week_day_offset(week_offset: usize, slot: usize) -> u32 {
    let count = WEEK_MATCH_COUNTS.get(week_offset).copied().unwrap_or(0) as usize;
    let day_offset = MATCH_DAY_OFFSETS[count.min(2)][slot.min(1)];
    week_offset as u32 * 7 + day_offset as u32
}

/// True when `round` is the last round of its calendar week — false means a
/// second fixture follows in the same week (double-fixture weeks). Feed this
/// to `Intent::ApplyRoundResult { week_ends }` so the same calendar week never
/// ticks twice.
pub fn week_ends_after_round(round: usize) -> bool {
    round + 1 >= ROUNDS_PER_SEASON || round_to_week(round + 1) != round_to_week(round)
}

// ── Date helpers ──────────────────────────────────────────────────────────────

/// Compute the real calendar date for a match.
///
/// `season_year`      — the year the season starts (e.g. 2025 for 2025/26).
/// `week_offset`      — 0-indexed calendar week within the season.
/// `slot`             — which match within the week (0 or 1).
pub fn match_date(season_year: u32, week_offset: usize, slot: usize) -> (u32, u32, u32) {
    let count = WEEK_MATCH_COUNTS.get(week_offset).copied().unwrap_or(0) as usize;
    let day_offset = MATCH_DAY_OFFSETS[count.min(2)][slot.min(1)];
    let total_days = season_anchor_offset(season_year) + week_offset as u32 * 7 + day_offset as u32;
    advance_from_aug15(season_year, total_days)
}

/// Format a date as "Sat 15 Aug 2025".
pub fn format_match_date(season_year: u32, week_offset: usize, slot: usize) -> String {
    let (year, month, day) = match_date(season_year, week_offset, slot);
    // The week grid is Monday-anchored (see season_anchor_offset), so the
    // day offset IS the weekday (0=Mon … 6=Sun).
    let count = WEEK_MATCH_COUNTS.get(week_offset).copied().unwrap_or(0) as usize;
    let day_offset = MATCH_DAY_OFFSETS[count.min(2)][slot.min(1)];
    let weekday = day_offset as usize % 7;
    let day_names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let month_names = [
        "", "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!(
        "{} {} {} {}",
        day_names[weekday],
        day,
        month_names.get(month as usize).unwrap_or(&""),
        year
    )
}

/// Format a week label for the header: "Game Week 5 · Aug 2025".
pub fn format_week_header(season_year: u32, week_offset: usize) -> String {
    let (year, month, _) = advance_from_aug15(
        season_year,
        season_anchor_offset(season_year) + week_offset as u32 * 7,
    );
    let month_names = [
        "", "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!(
        "Game Week {} · {} {}",
        week_offset + 1,
        month_names.get(month as usize).unwrap_or(&""),
        year
    )
}

// ── Internal date arithmetic ──────────────────────────────────────────────────

fn advance_from_aug15(start_year: u32, offset_days: u32) -> (u32, u32, u32) {
    let mut year = start_year;
    let mut month = 8u32;
    let mut day = 15u32;
    let mut remaining = offset_days;
    while remaining > 0 {
        let in_month = days_in_month(year, month);
        let left = in_month - day;
        if remaining <= left {
            day += remaining;
            break;
        }
        remaining -= left + 1;
        day = 1;
        month += 1;
        if month > 12 {
            month = 1;
            year += 1;
        }
    }
    (year, month, day)
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn is_leap_year(year: u32) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

/// Returns the weekday of Aug 15 for a given year (0=Mon … 6=Sun).
/// Uses Tomohiko Sakamoto's algorithm (adjusted for Mon-origin).
fn weekday_of_aug15(year: u32) -> usize {
    // Standard algorithm gives 0=Sun…6=Sat; adjust to Mon=0.
    let dow_sun_origin = {
        let t = [0u32, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        let y = year; // month 8 >= 3, no year adjustment needed
        (y + y / 4 - y / 100 + y / 400 + t[7] + 15) % 7
    };
    // Convert Sun=0 → Mon=0: Sun becomes 6, Mon stays 0, Tue=1, …
    ((dow_sun_origin + 6) % 7) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_sum_to_rounds_per_season() {
        let sum: usize = WEEK_MATCH_COUNTS.iter().map(|&c| c as usize).sum();
        assert_eq!(sum, ROUNDS_PER_SEASON);
    }

    #[test]
    fn round_to_week_first_round() {
        assert_eq!(round_to_week(0), 0); // first round is in week 0
    }

    #[test]
    fn round_to_week_week1_rounds() {
        // Week 1 has 2 matches: rounds 1 and 2 (after week 0's 1 match)
        assert_eq!(round_to_week(1), 1);
        assert_eq!(round_to_week(2), 1);
    }

    #[test]
    fn week_to_rounds_week0() {
        assert_eq!(week_to_rounds(0), 0..1);
    }

    #[test]
    fn week_to_rounds_week1() {
        assert_eq!(week_to_rounds(1), 1..3);
    }

    #[test]
    fn break_week_has_no_rounds() {
        assert!(is_break_week(4)); // week 4 is a break
        assert_eq!(week_to_rounds(4), 5..5); // empty range
    }

    #[test]
    fn season_opener_date() {
        // Aug 15 2025 is a Friday → week grid anchors on Mon Aug 18.
        // Week 0, 1 match → Saturday: Aug 18 + 5 = Sat Aug 23.
        let (y, m, d) = match_date(2025, 0, 0);
        assert_eq!((y, m, d), (2025, 8, 23));
        assert_eq!(format_match_date(2025, 0, 0), "Sat 23 Aug 2025");
    }

    #[test]
    fn two_match_week_dates() {
        // Week 1 starts Mon Aug 25 (anchor Mon Aug 18 + 7).
        // Tuesday = Aug 26, Saturday = Aug 30.
        let (_, m1, d1) = match_date(2025, 1, 0); // Tuesday
        let (_, m2, d2) = match_date(2025, 1, 1); // Saturday
        assert_eq!((m1, d1), (8, 26));
        assert_eq!((m2, d2), (8, 30));
        assert_eq!(format_match_date(2025, 1, 0), "Tue 26 Aug 2025");
        assert_eq!(format_match_date(2025, 1, 1), "Sat 30 Aug 2025");
    }

    /// Independent weekday check (Sakamoto, 0=Sun … 6=Sat) — deliberately NOT
    /// using the module's Monday-anchored shortcut, so it catches anchor bugs.
    fn true_weekday(y: u32, m: u32, d: u32) -> u32 {
        let t = [0u32, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        let y = if m < 3 { y - 1 } else { y };
        (y + y / 4 - y / 100 + y / 400 + t[(m - 1) as usize] + d) % 7
    }

    #[test]
    fn match_weekdays_correct_every_season_year() {
        // The anchor must hold for any year: 1-match weeks land on real
        // Saturdays, 2-match weeks on real Tuesday + Saturday.
        const SAT: u32 = 6;
        const TUE: u32 = 2;
        for year in 2025..2045 {
            for (week, &count) in WEEK_MATCH_COUNTS.iter().enumerate() {
                for slot in 0..count as usize {
                    let (y, m, d) = match_date(year, week, slot);
                    let expected = if count == 1 || slot == 1 { SAT } else { TUE };
                    assert_eq!(
                        true_weekday(y, m, d),
                        expected,
                        "season {year} week {week} slot {slot}: {y}-{m:02}-{d:02} \
                         is not on the expected weekday ({})",
                        format_match_date(year, week, slot),
                    );
                }
            }
        }
    }

    #[test]
    fn date_crosses_year_boundary() {
        // Find a week that falls in January.
        // Aug 15 + 20 weeks = Aug 15 + 140 days = Jan 2 (approx).
        let (y, m, _) = advance_from_aug15(2025, 140);
        assert_eq!(y, 2026);
        assert_eq!(m, 1);
    }

    #[test]
    fn week_day_offset_matches_weekday_pattern() {
        // Week 0 (1 match): Saturday slot, offset 5.
        assert_eq!(week_day_offset(0, 0), 5);
        // Week 1 (2 matches): Tuesday then Saturday, offsets 1 and 5, plus the week's
        // own 7-day stride.
        assert_eq!(week_day_offset(1, 0), 7 + 1);
        assert_eq!(week_day_offset(1, 1), 7 + 5);
    }

    #[test]
    fn week_day_offset_strictly_increases_with_week() {
        // Every match in week N+1 must fall after every match in week N, so fixtures
        // built from consecutive rounds never land on the same or an earlier day.
        let mut last_max = 0u32;
        for (week, &raw_count) in WEEK_MATCH_COUNTS.iter().enumerate() {
            let count = raw_count as usize;
            if count == 0 {
                continue;
            }
            let first = week_day_offset(week, 0);
            assert!(first > last_max || week == 0, "week {week} regressed");
            for slot in 0..count {
                last_max = last_max.max(week_day_offset(week, slot));
            }
        }
    }
}
