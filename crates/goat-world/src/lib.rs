#![forbid(unsafe_code)]

//! `goat-world` — mini-world data, fixture generation, and league table.
//!
//! This crate depends on `goat-core` but is NOT depended on by it.
//! The TUI links goat-world and goat-core together; goat-core stays headless.

pub mod batch_tick;
pub mod calendar;
pub mod fixtures;
pub mod history;
pub mod manager;
pub mod nations;
pub mod orbit;
pub mod population;
pub mod rival;
pub mod scout;
pub mod season;
pub mod staff;
pub mod world;
pub mod worldgen;

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
    club_div_pos, club_division, div_clubs, div_index, facilities_mult, nation_name, ClubId,
    NationId, CLUBS_PER_DIV, NATION_BRAZIL, NATION_ENGLAND, NUM_CLUBS, NUM_DIVISIONS, NUM_NATIONS,
};
