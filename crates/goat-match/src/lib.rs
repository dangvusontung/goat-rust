#![forbid(unsafe_code)]

//! `goat-match` — the beat engine, headspace, discipline, and match simulation.
//!
//! Public surface:
//! - `beats::{Beat, BeatChoice, Outcome, MatchPhase, ScoreEvent, HeadspaceDelta, DisciplineEvent}`
//! - `beats_data::RawBeatLibrary` — JSON-deserialised authoring data
//! - `library::{BEATS, NUM_BEATS, B_*}` — legacy static constants (reckless-challenge beat)
//! - `headspace::Headspace`
//! - `contest::{resolve_contest, auto_pick_choice}`
//! - `discipline::{RefPersonality, FoulRisk, resolve_card, red_mist_roll}`
//! - `sim::{BeatLibrary, MatchSetup, ActiveMatchState, MatchResult, start_match, advance_beat, auto_play_match}`

pub mod beats;
pub mod beats_data;
pub mod contest;
pub mod discipline;
pub mod headspace;
pub mod library;
pub mod sim;
