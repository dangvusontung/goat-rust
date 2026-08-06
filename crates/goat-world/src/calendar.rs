//! Season calendar — maps league rounds to calendar weeks and real dates.
//!
//! The season runs 45 calendar weeks: a 7-week pre-season (train / rest /
//! friendlies) then the 38-round competition grid. Day 0 of a career is Jul 1;
//! the grid is anchored to the first Monday on/after Jul 1 of the season year
//! (weeks run Mon–Sun), so fixtures still kick off mid-August.
//! Each grid week has 0, 1, or 2 league matches; 0-match weeks are breaks.
//! All data is static — nothing is stored, everything is recomputed.

use crate::fixtures::ROUNDS_PER_SEASON;

/// Total calendar weeks spanned by one season: 7 pre-season weeks + the 38-week
/// competition grid. Pre-season is real, ticked content (train / rest /
/// friendlies), not dead time — and 45 grid weeks + 7 off-season weeks keeps the
/// season-year at exactly 52 weeks, so the age model is untouched.
pub const SEASON_CALENDAR_WEEKS: usize = 45;

/// Pre-season lead: this many zero-match weeks at the front of every season's
/// grid (the "day 0 = Jul 1" career anchor — the fixtures themselves still kick
/// off mid-August, unchanged). Fixed at 7 per Tùng 2026-08-04; the opener drifts
/// Aug 17–29 depending on the year's weekday alignment, which is accepted.
pub const PRE_SEASON_WEEKS: usize = 7;

/// Career base year — season 1 starts in this year.
///
/// Fallback/harness default only (v19+): the live game reads the real year from
/// wall-clock ONCE at new-game (TUI/bridge/web outer layers) and persists it as
/// `WorldState::career_base_year`. This const remains the default for pre-v19
/// saves and for deterministic dev harnesses (career_sim).
pub const BASE_CAREER_YEAR: u32 = 2025;

/// Number of league matches per calendar week (0 = break / rest).
/// Must sum to exactly ROUNDS_PER_SEASON (38, at the confirmed 20-clubs/tier scale).
///
/// Layout (7 pre-season weeks + 3 break weeks kept for flavor — international
/// break, winter break, spring break — the rest of the grid carries 38 rounds):
///   wk 0–6  : Jul–mid-Aug pre-season (no league matches; friendlies live here)
///   wk 7–10 : Aug  (opener, early busy period)
///   wk 11   : Sep  break (international)
///   wk 12–27: Sep–Jan (steady run, Christmas/NY congestion)
///   wk 28   : Jan  winter break
///   wk 29–38: Jan–Mar (catch-up double-header + steady run)
///   wk 39   : Apr  break
///   wk 40–44: Apr–May (run-in / season tail)
pub const WEEK_MATCH_COUNTS: [u8; SEASON_CALENDAR_WEEKS] = [
    0, 0, 0, 0, 0, 0, 0, // pre-season (PRE_SEASON_WEEKS)
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

/// Days from Jul 1 of `season_year` to the season's week-grid anchor: the
/// first Monday on/after Jul 1. Weeks run Mon–Sun, so the MATCH_DAY_OFFSETS
/// land on real Tuesdays/Saturdays in every season year. Without this anchor
/// the grid starts on whatever weekday Jul 1 falls on and "Saturday" fixtures
/// would render as weekdays.
fn season_anchor_offset(season_year: u32) -> u32 {
    (7 - weekday_of_jul1(season_year) as u32) % 7
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
    advance_from_jul1(season_year, total_days)
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
    let (year, month, _) = advance_from_jul1(
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

fn advance_from_jul1(start_year: u32, offset_days: u32) -> (u32, u32, u32) {
    let mut year = start_year;
    let mut month = 7u32;
    let mut day = 1u32;
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

/// Returns the weekday of Jul 1 for a given year (0=Mon … 6=Sun).
/// Uses Tomohiko Sakamoto's algorithm (adjusted for Mon-origin).
fn weekday_of_jul1(year: u32) -> usize {
    // Standard algorithm gives 0=Sun…6=Sat; adjust to Mon=0.
    let dow_sun_origin = {
        let t = [0u32, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        let y = year; // month 7 >= 3, no year adjustment needed
        (y + y / 4 - y / 100 + y / 400 + t[6] + 1) % 7
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
        // Round 0 is in week 7 — the first week after the 7-week pre-season.
        assert_eq!(round_to_week(0), PRE_SEASON_WEEKS);
    }

    #[test]
    fn round_to_week_week1_rounds() {
        // Week 8 has 2 matches: rounds 1 and 2 (after week 7's 1 match)
        assert_eq!(round_to_week(1), 8);
        assert_eq!(round_to_week(2), 8);
    }

    #[test]
    fn week_to_rounds_preseason_is_empty() {
        assert_eq!(week_to_rounds(0), 0..0);
        assert!(is_break_week(0));
        assert_eq!(week_to_rounds(PRE_SEASON_WEEKS), 0..1);
    }

    #[test]
    fn week_to_rounds_week1() {
        assert_eq!(week_to_rounds(8), 1..3);
    }

    #[test]
    fn break_week_has_no_rounds() {
        assert!(is_break_week(11)); // week 11 is the first in-grid break
        assert_eq!(week_to_rounds(11), 5..5); // empty range
    }

    #[test]
    fn season_opener_date() {
        // Jul 1 2025 is a Tuesday → week grid anchors on Mon Jul 7.
        // Week 7 (first match week after the 7-week pre-season), 1 match →
        // Saturday: Jul 7 + 7*7 + 5 = Sat Aug 30.
        let (y, m, d) = match_date(2025, 7, 0);
        assert_eq!((y, m, d), (2025, 8, 30));
        assert_eq!(format_match_date(2025, 7, 0), "Sat 30 Aug 2025");
    }

    #[test]
    fn two_match_week_dates() {
        // Week 8 starts Mon Sep 1 (anchor Mon Jul 7 + 8*7 = Jul 7 + 56 days).
        // Tuesday = Sep 2, Saturday = Sep 6.
        let (_, m1, d1) = match_date(2025, 8, 0); // Tuesday
        let (_, m2, d2) = match_date(2025, 8, 1); // Saturday
        assert_eq!((m1, d1), (9, 2));
        assert_eq!((m2, d2), (9, 6));
        assert_eq!(format_match_date(2025, 8, 0), "Tue 2 Sep 2025");
        assert_eq!(format_match_date(2025, 8, 1), "Sat 6 Sep 2025");
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
        // Find a week that falls in November.
        // Jul 1 + 140 days = Nov 18 (approx).
        let (y, m, _) = advance_from_jul1(2025, 140);
        assert_eq!(y, 2025);
        assert_eq!(m, 11);
    }

    #[test]
    fn week_day_offset_matches_weekday_pattern() {
        // Week 7 (1 match): Saturday slot, offset 7*7 + 5 = 54.
        assert_eq!(week_day_offset(7, 0), 54);
        // Week 8 (2 matches): Tuesday then Saturday, offsets 1 and 5, plus the
        // week's own 7-day stride.
        assert_eq!(week_day_offset(8, 0), 56 + 1);
        assert_eq!(week_day_offset(8, 1), 56 + 5);
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
