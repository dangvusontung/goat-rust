//! Referee personality and foul/card resolution.
//!
//! Cards emerge from beat choices and frustration — never a standalone dice roll.
//! `dirty_rep` (0–100, 50 = neutral) tightens officiating: a player with a dirty
//! reputation draws more cards because refs watch them more closely.

use crate::beats::DisciplineEvent;
use goat_rng::RngSource;

/// Referee personality for this match (generated at match-start from RNG).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefPersonality {
    Strict,
    Balanced,
    Lenient,
}

impl RefPersonality {
    /// Modifier to foul probability (in percentage points).
    pub fn foul_mod(self) -> i32 {
        match self {
            RefPersonality::Strict => 20,
            RefPersonality::Balanced => 0,
            RefPersonality::Lenient => -15,
        }
    }

    /// Roll ref personality from RNG — deterministic per match seed.
    pub fn from_rng(rng: &mut impl RngSource) -> Self {
        match rng.next_range_u64(0, 2) {
            0 => RefPersonality::Strict,
            1 => RefPersonality::Balanced,
            _ => RefPersonality::Lenient,
        }
    }
}

/// Foul risk carried by an action (authored in beats.json as `foul_chance` /
/// `foul_serious`).
///
/// `foul_chance` (0–100): base probability of a foul being awarded.
/// `serious`: true = DOGSO / cynical — escalates directly to red risk.
#[derive(Debug, Clone, Copy)]
pub struct FoulRisk {
    pub foul_chance: u8,
    pub serious: bool,
}

/// Resolve a foul risk into a card (or nothing).
///
/// Called from `advance_beat` when a foul-risk choice is made. The result
/// feeds into `ActiveMatchState.yellow_cards / red_card`.
pub fn resolve_card(
    risk: &FoulRisk,
    aggression_attr: u8,
    frustration: i32,
    dirty_rep: i32,
    ref_personality: RefPersonality,
    existing_yellow: u8,
    rng: &mut impl RngSource,
) -> Option<DisciplineEvent> {
    // Step 1: check if a foul is given at all.
    let rep_mod = (dirty_rep - 50) / 5; // ‒10..+10 depending on reputation
    let foul_prob =
        (risk.foul_chance as i32 + ref_personality.foul_mod() + rep_mod).clamp(5, 95) as u64;

    if rng.next_range_u64(0, 99) >= foul_prob {
        return None; // no foul — play on
    }

    // Step 2: card escalation.
    // Serious foul → red (DOGSO) if aggression or frustration is high.
    if risk.serious {
        let red_prob = (30
            + (aggression_attr as i32 - 50) / 3  // aggression contribution
            + frustration / 8                      // frustration contribution
            + ref_personality.foul_mod() / 2)
            .clamp(10, 80) as u64;
        if rng.next_range_u64(0, 99) < red_prob {
            return Some(DisciplineEvent::RedCard);
        }
    }

    // Step 3: yellow card chance.
    let yellow_prob = (40
        + (aggression_attr as i32 - 50) / 5
        + frustration / 10
        + ref_personality.foul_mod() / 2
        + existing_yellow as i32 * 5) // more likely if already booked
        .clamp(15, 90) as u64;

    if rng.next_range_u64(0, 99) < yellow_prob {
        // Second yellow = red.
        if existing_yellow >= 1 {
            return Some(DisciplineEvent::RedCard);
        }
        return Some(DisciplineEvent::YellowCard);
    }

    None // foul given but no card
}

/// Per-beat red-mist check when frustration is extreme (> 80).
///
/// Separate from foul choices — the player just snaps without a tactical option.
pub fn red_mist_roll(
    frustration: i32,
    aggression_attr: u8,
    dirty_rep: i32,
    ref_personality: RefPersonality,
    existing_yellow: u8,
    rng: &mut impl RngSource,
) -> Option<DisciplineEvent> {
    if frustration <= 80 {
        return None;
    }
    // Base 3% per beat at frustration 80; rises steeply toward 100.
    let prob = ((frustration - 80) * (aggression_attr as i32 / 20) / 3
        + (dirty_rep - 50).max(0) / 10
        + ref_personality.foul_mod() / 10)
        .clamp(0, 20) as u64;

    if rng.next_range_u64(0, 99) < prob {
        if existing_yellow >= 1 {
            return Some(DisciplineEvent::RedCard);
        }
        return Some(DisciplineEvent::YellowCard);
    }
    None
}
