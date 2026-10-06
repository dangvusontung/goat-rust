//! Deterministic Gregorian dates on one July-1 career epoch. No wall clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CivilDate {
    pub year: u32,
    pub month: u8,
    pub day: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeasonFrame {
    pub id: u32,
    pub preparation_start: u32,
    pub start_day: u32,
    pub end_day: u32,
    pub next_preparation_start: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chronology {
    pub base_year: u32,
}
pub fn is_leap_year(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}
fn month_days(year: u32, month: u8) -> u32 {
    match month {
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}
fn year_start(year: u32) -> u64 {
    let y = year as u64 - 1;
    y * 365 + y / 4 - y / 100 + y / 400
}
fn serial(date: CivilDate) -> u64 {
    year_start(date.year)
        + (1..date.month)
            .map(|m| month_days(date.year, m) as u64)
            .sum::<u64>()
        + date.day as u64
        - 1
}
impl Chronology {
    pub fn new(base_year: u32) -> Self {
        assert!(base_year > 0);
        Self { base_year }
    }
    pub fn epoch_day(self, date: CivilDate) -> Option<u32> {
        if date.year == 0
            || !(1..=12).contains(&date.month)
            || date.day == 0
            || date.day as u32 > month_days(date.year, date.month)
        {
            return None;
        }
        let origin = serial(CivilDate {
            year: self.base_year,
            month: 7,
            day: 1,
        });
        serial(date)
            .checked_sub(origin)
            .and_then(|d| u32::try_from(d).ok())
    }
    pub fn date(self, day: u32) -> CivilDate {
        let n = serial(CivilDate {
            year: self.base_year,
            month: 7,
            day: 1,
        }) + day as u64;
        let mut year = (n / 366 + 1) as u32;
        while year_start(year + 1) <= n {
            year += 1;
        }
        let mut offset = n - year_start(year);
        let mut month = 1;
        while offset >= month_days(year, month) as u64 {
            offset -= month_days(year, month) as u64;
            month += 1;
        }
        CivilDate {
            year,
            month,
            day: offset as u8 + 1,
        }
    }
    pub fn frame(self, season: u32) -> SeasonFrame {
        assert!(season > 0);
        let year = self.base_year + season - 1;
        let day = |year, month, day| self.epoch_day(CivilDate { year, month, day }).unwrap();
        SeasonFrame {
            id: season,
            preparation_start: day(year, 7, 1),
            start_day: day(year, 8, 15),
            end_day: day(year + 1, 6, 30),
            next_preparation_start: day(year + 1, 7, 1),
        }
    }
    /// Summer belongs to the upcoming planning cycle, not an active club season.
    pub fn planning_season(self, day: u32) -> u32 {
        let d = self.date(day);
        d.year - self.base_year + u32::from(d.month >= 7)
    }
    pub fn active_season(self, day: u32) -> Option<u32> {
        let s = self.planning_season(day);
        let f = self.frame(s);
        (day >= f.start_day && day <= f.end_day).then_some(s)
    }
    /// Monday=0 ... Sunday=6, Gregorian 0001-01-01 was Monday.
    pub fn weekday(self, day: u32) -> u32 {
        ((serial(CivilDate {
            year: self.base_year,
            month: 7,
            day: 1,
        }) + day as u64)
            % 7) as u32
    }
    /// Calendar anniversaries, rather than an assumed 52-week year.
    pub fn age_years(self, initial_years: u32, entry: u32, now: u32) -> u32 {
        let e = self.date(entry);
        let n = self.date(now.max(entry));
        let anniversary_day = e.day.min(month_days(n.year, e.month) as u8);
        initial_years + n.year - e.year - u32::from((n.month, n.day) < (e.month, anniversary_day))
    }
    pub fn display_age_weeks(self, initial_years: u32, entry: u32, now: u32) -> u32 {
        let age = self.age_years(initial_years, entry, now);
        let e = self.date(entry);
        let anniversary_year = e.year + age - initial_years;
        let anniversary = self
            .epoch_day(CivilDate {
                year: anniversary_year,
                month: e.month,
                day: e.day.min(month_days(anniversary_year, e.month) as u8),
            })
            .unwrap();

        age * 52 + ((now.saturating_sub(anniversary)) / 7).min(51)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundaries_leap_days_and_twenty_seasons_do_not_drift() {
        let c = Chronology::new(2023);
        let f = c.frame(1);
        assert_eq!(f.start_day, 45);
        assert_eq!(f.end_day - f.start_day + 1, 321);
        assert_eq!(c.active_season(44), None);
        assert_eq!(c.active_season(45), Some(1));
        assert_eq!(c.active_season(f.end_day), Some(1));
        assert_eq!(c.active_season(f.end_day + 1), None);
        for s in 1..=20 {
            let f = c.frame(s);
            assert_eq!(
                c.date(f.start_day),
                CivilDate {
                    year: 2022 + s,
                    month: 8,
                    day: 15
                }
            );
            assert_eq!(
                c.date(f.end_day),
                CivilDate {
                    year: 2023 + s,
                    month: 6,
                    day: 30
                }
            );
            assert_eq!(c.frame(s + 1).start_day - f.next_preparation_start, 45);
            assert_eq!(c.planning_season(f.next_preparation_start), s + 1);
        }
        for day in 0..8000 {
            assert_eq!(c.epoch_day(c.date(day)), Some(day));
        }
        assert_eq!(
            c.epoch_day(CivilDate {
                year: 2024,
                month: 2,
                day: 29
            })
            .map(|d| c.date(d)),
            Some(CivilDate {
                year: 2024,
                month: 2,
                day: 29
            })
        );
        assert_eq!(
            c.epoch_day(CivilDate {
                year: 2025,
                month: 2,
                day: 29
            }),
            None
        );
        let c = Chronology::new(2025);
        assert_eq!(c.weekday(0), 1);
        let birthday = c.frame(21).preparation_start;
        assert_eq!(c.age_years(16, 0, birthday - 1), 35);
        assert_eq!(c.age_years(16, 0, birthday), 36);
    }
}
