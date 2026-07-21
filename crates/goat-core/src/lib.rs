#![forbid(unsafe_code)]

//! `goat-core` — headless deterministic simulation core.
//!
//! Public surface for renderers:
//! - `state::{WorldState, Intent, reduce}` — the only mutation point
//! - `generation::{CreationChoices, generate_player, STUB_*}` — player creation
//! - `positions::{PrimaryPosition}` — the 8 specific outfield positions creation picks from
//! - `player::{PlayerStore, PlayerView, PlayerId}` — storage types
//! - `derive::{derive_attrs, role_rating, ovr}` — derived display values
//! - `attrs::{AttrId, AttrFamily, AgeCurveArchetype, ATTR_NAMES, NUM_ATTRS}` — attribute metadata
//! - `roles::{RoleId, FamiliarityTier, ROLE_NAMES, NUM_ROLES}` — role metadata

pub mod attrs;
pub mod calendar_loop;
pub mod derive;
pub mod generation;
pub mod player;
pub mod positions;
pub mod roles;
pub mod state;
pub mod tuning;
pub mod week;
