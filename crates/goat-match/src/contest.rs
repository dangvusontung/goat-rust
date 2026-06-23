//! Contest resolution: player attribute vs difficulty + headspace + stamina + RNG.

use crate::headspace::Headspace;
use goat_fixed::Fixed;
use goat_rng::RngSource;

/// Resolve a beat contest. Returns `true` on success.
///
/// Formula:
/// 1. attr_advantage = player_attr − difficulty  (‒98..+98)
/// 2. base_pct = 50 + attr_advantage × 50/99     (clamped 5..95)
/// 3. Apply headspace modifier                   (‒25..+25 pp)
/// 4. Apply stamina penalty when below 30        (down to ‒15 pp)
/// 5. Apply context modifier (desperation etc.)
/// 6. Roll d100 — success if roll < total_pct
pub fn resolve_contest(
    player_attr: Fixed,
    difficulty: u8,
    headspace: &Headspace,
    stamina: Fixed,
    context_mod: i32,
    rng: &mut impl RngSource,
) -> bool {
    let attr_advantage = player_attr.to_int() - difficulty as i32;
    let base_pct = (50 + attr_advantage * 50 / 99).clamp(5, 95);

    let hs_mod = headspace.contest_mod();
    let stam_mod = stamina_mod(stamina);

    let total_pct = (base_pct + hs_mod + stam_mod + context_mod).clamp(3, 97);

    rng.next_range_u64(0, 99) < total_pct as u64
}

fn stamina_mod(stamina: Fixed) -> i32 {
    let s = stamina.to_int().clamp(0, 100);
    if s >= 30 {
        0
    } else {
        (s - 30) / 2
    } // ‒1 pp per 2 stamina below 30; max ‒15
}

/// Auto-pick the best available choice (for skip-match or AI) by choosing the
/// option where (player_attr − difficulty) is highest (most favourable odds).
pub fn auto_pick_choice(
    choices: &[crate::beats::BeatChoice],
    player_attrs: &[Fixed; goat_core::attrs::NUM_ATTRS],
) -> usize {
    choices
        .iter()
        .enumerate()
        .max_by_key(|(_, c)| player_attrs[c.primary as usize].to_int() - c.difficulty as i32)
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Auto-pick for generated choices (same logic, owned types).
pub fn auto_pick_generated_choice(
    choices: &[crate::beats::GeneratedChoice],
    player_attrs: &[Fixed; goat_core::attrs::NUM_ATTRS],
) -> usize {
    choices
        .iter()
        .enumerate()
        .max_by_key(|(_, c)| player_attrs[c.primary as usize].to_int() - c.difficulty as i32)
        .map(|(i, _)| i)
        .unwrap_or(0)
}
