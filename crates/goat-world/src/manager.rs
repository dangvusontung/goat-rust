//! Manager profile + manager↔PC relationship model (PA2 M1.5).
//!
//! Every club has a manager *derived* from `(world_seed, club_id)` — personality,
//! nationality, age, name — never stored (tiny-saves rule: derivable ⇒ recomputed).
//! What IS stored (path-dependent, save v11) are the two relationship scalars in
//! `WorldState`: `pc_manager_trust` (professional judgement: output, training
//! attitude, experience) and `pc_manager_favor` (personal bias/politics:
//! nationality, marketability, lifestyle clash — independent of performance).
//!
//! This module holds only pure formulas; the TUI drives the updates through
//! `Intent::SetManagerRelation` / `Intent::ApplyManagerRelation` so goat-core
//! stays headless (it must not depend on goat-world).

use crate::world::NUM_NATIONS;
use goat_core::week::Intensity;
use goat_rng::{GoatRng, RngSource};

/// Domain salt for the manager RNG stream (distinct from player/tactical streams).
const MANAGER_SALT: u64 = 0x4D47_525F_5341_4C54; // "MGR_SALT"
/// Salt for the manager's name draw (same stream family, independent draw).
const NAME_SALT: u64 = 0x9E6D_31A7_42C5_19B3;

/// The manager's personality — modulates how trust/favor react to the PC.
/// Drawn from the club seed, in the spirit of `RefPersonality::from_rng`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagerPersonality {
    /// Disciplinarian: punishes poor output / skipped training harder, dislikes
    /// flashy lifestyles and over-hyped marketability, values professionals.
    Strict,
    /// Even-handed: no modifiers.
    Balanced,
    /// Ego-friendly: forgives poor output faster, loves marketable/flashy stars.
    StarLover,
}

impl ManagerPersonality {
    pub fn name(self) -> &'static str {
        match self {
            ManagerPersonality::Strict => "Strict",
            ManagerPersonality::Balanced => "Balanced",
            ManagerPersonality::StarLover => "StarLover",
        }
    }

    /// Substitution patience 0–100 (PA2 M4): how long the manager tolerates a
    /// misfiring starter before hooking him. Strict hooks early, a StarLover
    /// waits for his star to play himself into form.
    pub fn patience(self) -> i32 {
        match self {
            ManagerPersonality::Strict => 30,
            ManagerPersonality::Balanced => 55,
            ManagerPersonality::StarLover => 75,
        }
    }
}

/// A club's manager, fully derived from `(world_seed, club_id)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagerProfile {
    pub name: String,
    pub personality: ManagerPersonality,
    /// Nation index into `NATIONS` (60% the club's own nation).
    pub nation: u8,
    /// 38–60.
    pub age_years: u8,
}

/// Derive a club's manager deterministically. Draw order is load-bearing:
/// personality, nation roll, [nation if foreign], age.
pub fn manager_for_club(world_seed: u64, club_id: usize, club_nation: u8) -> ManagerProfile {
    let seed = world_seed ^ MANAGER_SALT.wrapping_mul(club_id as u64 + 1);
    let mut rng = GoatRng::new(seed);
    let personality = match rng.next_range_u32(0, 2) {
        0 => ManagerPersonality::Strict,
        1 => ManagerPersonality::Balanced,
        _ => ManagerPersonality::StarLover,
    };
    let nation = if rng.next_range_u32(0, 99) < 60 {
        club_nation
    } else {
        rng.next_range_u32(0, NUM_NATIONS as u32 - 1) as u8
    };
    let age_years = 38 + rng.next_range_u32(0, 22) as u8;
    ManagerProfile {
        name: crate::history::name_from_seed(seed ^ NAME_SALT),
        personality,
        nation,
        age_years,
    }
}

// ── Trust: professional judgement ─────────────────────────────────────────────

/// Trust baseline when the PC arrives at a club (new game / transfer).
/// Experience earns respect; a Strict manager distrusts kids, a StarLover
/// is charmed by them.
pub fn trust_base(mgr: &ManagerProfile, pc_age_years: u32) -> i32 {
    let mut t = 50;
    if pc_age_years + 12 >= mgr.age_years as u32 {
        t += 6; // veteran respect — the PC is close to the manager's own age
    }
    if pc_age_years <= 20 {
        match mgr.personality {
            ManagerPersonality::Strict => t -= 5,
            ManagerPersonality::StarLover => t += 3,
            ManagerPersonality::Balanced => {}
        }
    }
    t.clamp(0, 100)
}

/// Trust delta from one played match's output (0–100 scale). Personality
/// modulates the downside: Strict punishes harder, StarLover forgives.
pub fn trust_match_delta(mgr: &ManagerProfile, output: i32) -> i32 {
    let raw = ((output - 50) / 8).clamp(-4, 4);
    if raw >= 0 {
        return raw;
    }
    match mgr.personality {
        ManagerPersonality::Strict => raw * 3 / 2,
        ManagerPersonality::StarLover => raw / 2,
        ManagerPersonality::Balanced => raw,
    }
}

/// Trust delta from the week's training attitude (per round).
pub fn trust_training_delta(mgr: &ManagerProfile, trained: bool, intensity: Intensity) -> i32 {
    if !trained {
        return match mgr.personality {
            ManagerPersonality::Strict => -5,
            ManagerPersonality::Balanced => -3,
            ManagerPersonality::StarLover => -2,
        };
    }
    match intensity {
        Intensity::High => 2,
        Intensity::Medium => 1,
        Intensity::Low => 0,
    }
}

// ── Favor: personal bias / politics ───────────────────────────────────────────

/// Inputs for the favor baseline — all already exist in `WorldState` (no new
/// systems invented): nationality match, marketability, fan/character/discipline
/// reps, lifestyle. ("Same agent" was dropped — the game has no agent identity.)
#[derive(Debug, Clone, Copy)]
pub struct FavorInputs {
    /// PC and manager share a nationality.
    pub same_nation: bool,
    pub marketability: i32,
    pub fan_rep: i32,
    pub character_rep: i32,
    pub discipline_rep: i32,
    /// 0=Professional, 1=Balanced, 2=Flashy.
    pub lifestyle: u8,
}

/// Favor baseline for the PC under this manager — the drift target recomputed
/// each round from current state.
pub fn favor_base(mgr: &ManagerProfile, inputs: &FavorInputs) -> i32 {
    let mut f = 50;
    if inputs.same_nation {
        f += 8;
    }
    // Fame: a StarLover is charmed by it, a Strict manager resents it (sign flip).
    if inputs.marketability >= 70 {
        match mgr.personality {
            ManagerPersonality::StarLover => f += 8,
            ManagerPersonality::Strict => f -= 8,
            ManagerPersonality::Balanced => {}
        }
    }
    f += ((inputs.fan_rep - 50) / 10).clamp(-5, 5);
    f += ((inputs.character_rep - 50) / 10).clamp(-4, 4);
    if inputs.discipline_rep >= 70 {
        f -= 6; // troublemaker
    } else if inputs.discipline_rep <= 25 {
        f += 3; // model pro
    }
    match inputs.lifestyle {
        // Flashy clashes with Strict, charms StarLover.
        2 => match mgr.personality {
            ManagerPersonality::Strict => f -= 10,
            ManagerPersonality::StarLover => f += 5,
            ManagerPersonality::Balanced => {}
        },
        // A Strict manager appreciates a professional lifestyle.
        0 if mgr.personality == ManagerPersonality::Strict => f += 6,
        _ => {}
    }
    f.clamp(0, 100)
}

/// Sticky weekly drift: favor moves one point toward the recomputed base.
pub fn favor_drift(current: i32, base: i32) -> i32 {
    (base - current).clamp(-1, 1)
}

/// Extra favor shock when the PC answers the media defiantly after a red card.
pub fn media_defiant_favor_delta(mgr: &ManagerProfile) -> i32 {
    match mgr.personality {
        ManagerPersonality::Strict => -4,
        ManagerPersonality::Balanced => -2,
        ManagerPersonality::StarLover => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manager_derive_is_deterministic_and_bounded() {
        for club in 0..40usize {
            let a = manager_for_club(7, club, 0);
            let b = manager_for_club(7, club, 0);
            assert_eq!(a, b, "manager must be deterministic");
            assert!((38..=60).contains(&a.age_years), "age in 38..=60");
            assert!((a.nation as usize) < NUM_NATIONS);
            assert!(!a.name.is_empty());
        }
        // Different clubs (usually) get different managers; different seeds differ.
        assert_ne!(manager_for_club(1, 3, 0), manager_for_club(2, 3, 0));
    }

    #[test]
    fn manager_nationality_biased_to_club_nation() {
        let mut home = 0;
        let n = 500usize;
        for club in 0..n {
            if manager_for_club(99, club, 4).nation == 4 {
                home += 1;
            }
        }
        // 60% target: a 500-club sweep should land well within 45–75%.
        let pct = home * 100 / n;
        assert!((45..=75).contains(&pct), "home-nation share {pct}%");
    }

    #[test]
    fn trust_base_rewards_experience_and_punishes_kids_under_strict() {
        let strict = ManagerProfile {
            name: "S".into(),
            personality: ManagerPersonality::Strict,
            nation: 0,
            age_years: 50,
        };
        let star = ManagerProfile {
            personality: ManagerPersonality::StarLover,
            ..strict.clone()
        };
        assert_eq!(trust_base(&strict, 17), 45);
        assert_eq!(trust_base(&star, 17), 53);
        assert_eq!(trust_base(&strict, 40), 56, "veteran respect");
    }

    #[test]
    fn trust_match_delta_personality_modulates_downside_only() {
        let mk = |p: ManagerPersonality| ManagerProfile {
            name: "M".into(),
            personality: p,
            nation: 0,
            age_years: 45,
        };
        assert_eq!(trust_match_delta(&mk(ManagerPersonality::Balanced), 90), 4);
        assert_eq!(trust_match_delta(&mk(ManagerPersonality::Balanced), 10), -4);
        assert_eq!(trust_match_delta(&mk(ManagerPersonality::Strict), 10), -6);
        assert_eq!(
            trust_match_delta(&mk(ManagerPersonality::StarLover), 10),
            -2
        );
        // Upside is identical across personalities.
        assert_eq!(trust_match_delta(&mk(ManagerPersonality::Strict), 90), 4);
    }

    #[test]
    fn trust_training_delta_rewards_effort() {
        let mk = |p: ManagerPersonality| ManagerProfile {
            name: "M".into(),
            personality: p,
            nation: 0,
            age_years: 45,
        };
        let strict = mk(ManagerPersonality::Strict);
        assert_eq!(trust_training_delta(&strict, false, Intensity::High), -5);
        assert_eq!(trust_training_delta(&strict, true, Intensity::High), 2);
        assert_eq!(trust_training_delta(&strict, true, Intensity::Low), 0);
        assert_eq!(
            trust_training_delta(&mk(ManagerPersonality::StarLover), false, Intensity::High),
            -2
        );
    }

    #[test]
    fn favor_base_sign_flips_with_personality() {
        let mk = |p: ManagerPersonality| ManagerProfile {
            name: "M".into(),
            personality: p,
            nation: 0,
            age_years: 45,
        };
        let flashy_star = FavorInputs {
            same_nation: true,
            marketability: 85,
            fan_rep: 50,
            character_rep: 50,
            discipline_rep: 50,
            lifestyle: 2, // Flashy
        };
        let star = favor_base(&mk(ManagerPersonality::StarLover), &flashy_star);
        let strict = favor_base(&mk(ManagerPersonality::Strict), &flashy_star);
        assert_eq!(star, 50 + 8 + 8 + 5);
        assert_eq!(strict, 50 + 8 - 8 - 10);
        // Neutral input stays neutral for a Balanced manager.
        let neutral = FavorInputs {
            same_nation: false,
            marketability: 50,
            fan_rep: 50,
            character_rep: 50,
            discipline_rep: 50,
            lifestyle: 1,
        };
        assert_eq!(favor_base(&mk(ManagerPersonality::Balanced), &neutral), 50);
    }

    #[test]
    fn favor_drift_moves_one_point_toward_base() {
        assert_eq!(favor_drift(50, 70), 1);
        assert_eq!(favor_drift(50, 20), -1);
        assert_eq!(favor_drift(50, 50), 0);
    }
}
