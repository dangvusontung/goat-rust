#![forbid(unsafe_code)]

//! `goat-meta` — legacy axes, pantheon, reputation, awards, and pundits.
//!
//! All scoring and ranking is pure computation — same inputs → same outputs.
//! No RNG in the axes/ranking path. Awards use seeded stochastic AI competitors.

pub mod awards;
pub mod legacy;
pub mod pantheon;
pub mod pundits;
pub mod reputation;

pub use awards::{compute_golden_boot, compute_player_of_year, AwardResult};
pub use legacy::{compute_axes, LegacyAxes, LegacyEvidence, AXIS_NAMES};
pub use pantheon::{
    all_rankings, rank_in_canon, PastGreat, School, CANON, NUM_CANON, NUM_SCHOOLS, SCHOOLS,
};
pub use pundits::{pundit_comment, Pundit, PunditContext, NUM_PUNDITS, PUNDITS};
pub use reputation::{compute_reputation, update_club_fan_rep, update_sporting_rep, Reputation};
