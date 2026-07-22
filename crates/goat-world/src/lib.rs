#![forbid(unsafe_code)]

//! `goat-world` — mini-world data, fixture generation, and league table.
//!
//! This crate depends on `goat-core` but is NOT depended on by it.
//! The TUI links goat-world and goat-core together; goat-core stays headless.

pub mod academy;
pub mod batch_tick;
pub mod calendar;
pub mod continental;
pub mod domestic_cup;
pub mod economy;
pub mod fixtures;
pub mod history;
pub mod manager;
pub mod national_tournament;
pub mod population;
pub mod promotion;
pub mod rival;
pub mod scouting;
pub mod season;
pub mod transfers;
pub mod world;

pub use academy::{apply_academy_investment, decay_academy_boost, run_academy_investment_pass};
pub use calendar::{
    format_match_date, format_week_header, is_break_week, match_date, rest_weeks_after_round,
    round_to_week, week_day_offset, week_ends_after_round, week_to_rounds, BASE_CAREER_YEAR,
    SEASON_CALENDAR_WEEKS, WEEK_MATCH_COUNTS,
};
pub use continental::{
    continental_slots_for_nation, nation_ranks, qualify_clubs, simulate_continental,
    ContinentalSeasonResult, ContinentalSlots, ContinentalTier, GroupStanding, KnockoutRoundResult,
    KnockoutTie,
};
pub use domestic_cup::{simulate_domestic_cup, CupRoundResult, CupSeasonResult, CupTie};
pub use economy::{market_valuation, open_transfer_window, total_income};
pub use fixtures::{
    fixture_for_round, fixtures_for_club, generate_fixtures, round_fixtures, Fixture,
    ROUNDS_PER_SEASON,
};
pub use manager::{
    apply_manager_identity_shift, hire_replacement, should_fire, Manager, ManagerId, ManagerPool,
    MANAGER_FORM_WINDOW, MANAGER_POOL_SIZE,
};
pub use national_tournament::{
    is_continental_championship_season, is_world_cup_season, national_team_strength,
    simulate_national_tournament, NationStanding, NationalKnockoutRoundResult, NationalKnockoutTie,
    NationalTournamentResult, QualifyingCycleResult, QualifyingGroupResult,
};
pub use promotion::{apply_season_end, PromoRelegationEvent, ReplayCache, TransitionType};
pub use scouting::{
    candidates_by_position, gem_hunt_target, gem_targets_by_position, weakest_position_target,
};
pub use season::{sim_team_match, Table, TableEntry};
pub use transfers::{run_transfer_pass, TransferLane};
pub use world::{
    Club, ClubId, DivLevel, GeneratedNation, League, LeagueId, NationId, WorldGenesis,
    ACADEMY_BOOST_MAX, CLUBS_PER_DIV, NUM_CLUBS, NUM_DIVISIONS, NUM_NATIONS, PROMO_RELEGATION_N,
    TIERS_PER_NATION,
};
