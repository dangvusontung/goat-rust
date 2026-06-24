#![forbid(unsafe_code)]

//! `goat-world` — mini-world data, fixture generation, and league table.
//!
//! This crate depends on `goat-core` but is NOT depended on by it.
//! The TUI links goat-world and goat-core together; goat-core stays headless.

pub mod batch_tick;
pub mod calendar;
pub mod fixtures;
pub mod history;
pub mod population;
pub mod rival;
pub mod season;
pub mod world;

pub use calendar::{
    format_match_date, format_week_header, is_break_week, match_date, round_to_week,
    week_to_rounds, BASE_CAREER_YEAR, SEASON_CALENDAR_WEEKS, WEEK_MATCH_COUNTS,
};
pub use fixtures::{
    fixture_for_round, fixtures_for_club, generate_fixtures, round_fixtures, Fixture,
    ROUNDS_PER_SEASON,
};
pub use season::{sim_team_match, Table, TableEntry};
pub use world::{
    club_div_pos, club_division, clubs_for_nation, Club, ClubId, DivLevel, Nation, CLUBS,
    CLUBS_PER_DIV, DIV_BRA_SEC, DIV_BRA_TOP, DIV_CLUBS, DIV_ENG_SEC, DIV_ENG_TOP, DIV_LEVELS,
    DIV_NAMES, DIV_NATIONS, NUM_CLUBS, NUM_DIVISIONS,
};
