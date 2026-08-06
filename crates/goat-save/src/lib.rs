#![forbid(unsafe_code)]

//! `goat-save` — tiny-save serialisation.
//!
//! Persist only path-dependent state. Everything derivable from the world seed
//! is NOT saved and is recomputed on load.

pub mod save;
pub use save::{
    from_world_state, list_slots, load_from_file, save_to_file, slot_path, to_world_state,
    SaveData, SaveError, SaveSlotSummary,
};
