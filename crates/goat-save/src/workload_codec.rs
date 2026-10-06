//! Save adapter for the shared, lossless journal codec.
use super::SaveError;
use goat_core::{history::NpcMatchLoad, state::NpcMatchCredit};
pub(super) fn write(out: &mut Vec<u8>, loads: &[NpcMatchLoad]) {
    goat_core::journal::write(out, loads);
}
pub(super) fn read(bytes: &[u8], cur: &mut usize) -> Result<Vec<NpcMatchLoad>, SaveError> {
    goat_core::journal::read(bytes, cur).map_err(|e| SaveError::Corrupt(e.0))
}
pub(super) fn write_credits(out: &mut Vec<u8>, credits: &[NpcMatchCredit]) {
    goat_core::journal::write_credits(out, credits);
}
pub(super) fn read_credits(
    bytes: &[u8],
    cur: &mut usize,
    count: u32,
) -> Result<Vec<NpcMatchCredit>, SaveError> {
    goat_core::journal::read_credits(bytes, cur, count).map_err(|e| SaveError::Corrupt(e.0))
}
