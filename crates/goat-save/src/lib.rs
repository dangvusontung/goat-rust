#![forbid(unsafe_code)]

//! `goat-save` — tiny-save serialisation.
//!
//! Persist only path-dependent state. Everything derivable from the world seed
//! is NOT saved and is recomputed on load.

pub mod save;
pub use save::{
    from_world_state, load_from_file, save_to_file, to_world_state, SaveData, SaveError,
};
