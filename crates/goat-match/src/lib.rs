#![forbid(unsafe_code)]

//! `goat-match` — the Match Flow engine: stat-driven situations, headspace,
//! discipline, and match simulation.
//!
//! Public surface:
//! - `beats::{Possession, ScoreEvent, HeadspaceDelta, DisciplineEvent, Generated*}`
//! - `beats_data::RawBeatLibrary` — JSON-deserialised authoring data
//! - `headspace::Headspace`
//! - `contest::{resolve_contest, auto_pick_generated_choice}`
//! - `discipline::{RefPersonality, FoulRisk, resolve_card, red_mist_roll}`
//! - `sim::{BeatLibrary, MatchSetup, ActiveMatchState, MatchResult, start_match, advance_beat, auto_play_match}`

pub mod beats;
pub mod beats_data;
pub mod contest;
pub mod discipline;
pub mod headspace;
pub mod sim;
