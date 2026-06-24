//! The week loop — training, energy, growth, familiarity XP, random events.
//!
//! `advance_week` is pure: same state + same RNG sequence = same universe.
//! No floats, no I/O, no logging in this path.

use goat_fixed::Fixed;
use goat_rng::RngSource;

use crate::attrs::{AgeCurveArchetype, AttrId, ATTR_ARCHETYPES};
use crate::player::{PlayerId, PlayerStore};
use crate::roles::{FamiliarityTier, RoleId, ROLE_WEIGHT_TABLE};
use crate::tuning::{
    BASE_DECAY_PER_WEEK, BASE_GROWTH_PER_WEEK, BASE_INJURY_PER_1000, BREAKTHROUGH_BONUS,
    BREAKTHROUGH_PER_1000, DECLINE_LIFESTYLE_BALANCED, DECLINE_LIFESTYLE_FLASHY,
    DECLINE_LIFESTYLE_PRO, ENERGY_AUTO_DOWNGRADE, ENERGY_COST_HIGH, ENERGY_COST_LOW,
    ENERGY_COST_MED, ENERGY_MAX, ENERGY_PASSIVE_RECOVERY, ENERGY_RECOVERY_INJURED, FAM_XP_AWKWARD,
    FAM_XP_COMPETENT, FAM_XP_IMP_PER_WEEK, FAM_XP_KEY_PER_WEEK, FAM_XP_UNCONVINCING,
    GROWTH_MULT_HIGH, GROWTH_MULT_LOW, GROWTH_MULT_MED, GROWTH_SINGLE_WEEK_CAP,
    GROWTH_VARIANCE_RAW, INJURY_LIFESTYLE_X10_BALANCED, INJURY_LIFESTYLE_X10_FLASHY,
    INJURY_LIFESTYLE_X10_PRO, INJURY_WEEKS_MAX, INJURY_WEEKS_MIN, LIFESTYLE_CEILING_BALANCED,
    LIFESTYLE_CEILING_FLASHY, LIFESTYLE_CEILING_PRO, W_IMP, W_KEY,
};

// ── Public types ──────────────────────────────────────────────────────────────

/// Weekly training intensity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intensity {
    Low,
    Medium,
    High,
}

impl Intensity {
    pub fn name(self) -> &'static str {
        match self {
            Intensity::Low => "Low",
            Intensity::Medium => "Medium",
            Intensity::High => "High",
        }
    }

    fn energy_cost(self) -> Fixed {
        match self {
            Intensity::Low => ENERGY_COST_LOW,
            Intensity::Medium => ENERGY_COST_MED,
            Intensity::High => ENERGY_COST_HIGH,
        }
    }

    fn growth_mult(self) -> Fixed {
        match self {
            Intensity::Low => GROWTH_MULT_LOW,
            Intensity::Medium => GROWTH_MULT_MED,
            Intensity::High => GROWTH_MULT_HIGH,
        }
    }
}

/// A player's weekly training plan.
#[derive(Debug, Clone)]
pub struct Routine {
    /// Attributes to focus on this week (up to 5 makes sense; no hard limit).
    pub focus_attrs: Vec<AttrId>,
    pub intensity: Intensity,
}

impl Default for Routine {
    /// Default routine: medium intensity, no focus attrs (recovery-only week).
    fn default() -> Self {
        Self {
            focus_attrs: Vec::new(),
            intensity: Intensity::Medium,
        }
    }
}

/// Noteworthy events produced by a week tick.
///
/// Quiet weeks return an empty list; the TUI only interrupts when this is non-empty.
#[derive(Debug, Clone)]
pub enum DevelopmentEvent {
    Injury {
        weeks: u32,
    },
    Illness {
        weeks: u32,
    },
    Breakthrough {
        attr: AttrId,
        bonus: Fixed,
    },
    FamiliarityUpgrade {
        role: RoleId,
        new_tier: FamiliarityTier,
    },
}

// ── Main entry point ──────────────────────────────────────────────────────────

/// Advance the simulation by one week, returning the updated store and any
/// noteworthy events. Called from `reduce(Intent::AdvanceWeek)`.
pub fn advance_week(
    players: &mut PlayerStore,
    pc_id: PlayerId,
    routine: &Routine,
    facilities_mult: Fixed,
    lifestyle: u8,
    rng: &mut impl RngSource,
) -> Vec<DevelopmentEvent> {
    let mut events: Vec<DevelopmentEvent> = Vec::new();

    // ── Age advancement (always) ──────────────────────────────────────────────
    let age_weeks = players.get_age_weeks(pc_id);
    players.set_age_weeks(pc_id, age_weeks + 1);
    let age_years = age_weeks / 52;

    // ── Injured? Rest and return early ────────────────────────────────────────
    let injury = players.get_injury_weeks(pc_id);
    if injury > 0 {
        players.set_injury_weeks(pc_id, injury - 1);
        let e =
            (players.get_energy(pc_id) + ENERGY_RECOVERY_INJURED).clamp(Fixed::ZERO, ENERGY_MAX);
        players.set_energy(pc_id, e);
        return events;
    }

    // ── Effective intensity (auto-downgrade if exhausted) ─────────────────────
    let energy = players.get_energy(pc_id);
    let intensity = if energy < ENERGY_AUTO_DOWNGRADE {
        Intensity::Low
    } else {
        routine.intensity
    };

    // ── Energy: cost + passive recovery ──────────────────────────────────────
    let new_energy =
        (energy - intensity.energy_cost() + ENERGY_PASSIVE_RECOVERY).clamp(Fixed::ZERO, ENERGY_MAX);
    players.set_energy(pc_id, new_energy);

    // ── Injury check ─────────────────────────────────────────────────────────
    let inj_prob = injury_prob(new_energy, intensity, age_years, lifestyle);
    if rng.next_range_u64(0, 999) < inj_prob as u64 {
        let dur = rng.next_range_u8(INJURY_WEEKS_MIN, INJURY_WEEKS_MAX) as u32;
        players.set_injury_weeks(pc_id, dur);
        events.push(DevelopmentEvent::Injury { weeks: dur });
        // Return early — injured this week, no training
        return events;
    }

    // Facilities multiplier is passed in directly (set from goat-world at club init).

    // ── Energy factor: full energy = 1.0, zero energy = 0.6 ─────────────────
    let energy_factor = energy_growth_factor(new_energy);

    // Snapshot current values before changes (for familiarity XP logic)
    let intensity_mult = intensity.growth_mult();

    // ── Per-attribute growth ──────────────────────────────────────────────────
    for &attr in &routine.focus_attrs {
        let a = attr as usize;
        let growth_rate = attr_growth_rate(ATTR_ARCHETYPES[a], age_years);
        if growth_rate == Fixed::ZERO {
            continue; // too old to improve this archetype
        }

        let base =
            BASE_GROWTH_PER_WEEK * growth_rate * intensity_mult * energy_factor * facilities_mult;

        // Random variance ±GROWTH_VARIANCE_RAW thousandths
        let variance_raw =
            rng.next_range_u64(0, (GROWTH_VARIANCE_RAW * 2) as u64) as i32 - GROWTH_VARIANCE_RAW;
        let growth = (base + Fixed::raw(variance_raw)).clamp(Fixed::ZERO, GROWTH_SINGLE_WEEK_CAP);

        let cur = players.get_current(pc_id, a);
        // Effective ceiling: a flashy lifestyle burns a little of the potential, so
        // the player never quite reaches the top (still ≤ potential — pillar §2.4).
        let pot = players.get_potential(pc_id, a) * lifestyle_ceiling(lifestyle);
        players.set_current(pc_id, a, (cur + growth).clamp(Fixed::MIN_ATTR, pot));
    }

    // ── Passive decay for unfocused physical/technical attrs in older players ─
    apply_passive_decay(players, pc_id, age_years, &routine.focus_attrs, lifestyle);

    // ── Familiarity XP and tier upgrades ─────────────────────────────────────
    let new_events = update_familiarity(players, pc_id, &routine.focus_attrs);
    events.extend(new_events);

    // ── Breakthrough check ────────────────────────────────────────────────────
    if !routine.focus_attrs.is_empty() && rng.next_range_u64(0, 999) < BREAKTHROUGH_PER_1000 as u64
    {
        let attr = routine.focus_attrs[0]; // reward the primary focus attr
        let a = attr as usize;
        let cur = players.get_current(pc_id, a);
        // Same effective ceiling as training growth — a breakthrough can't exceed it.
        let pot = players.get_potential(pc_id, a) * lifestyle_ceiling(lifestyle);
        let new_val = (cur + BREAKTHROUGH_BONUS).clamp(Fixed::MIN_ATTR, pot);
        players.set_current(pc_id, a, new_val);
        events.push(DevelopmentEvent::Breakthrough {
            attr,
            bonus: BREAKTHROUGH_BONUS,
        });
    }

    events
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Growth rate for an archetype at a given age (scales BASE_GROWTH_PER_WEEK).
///
/// Returns Fixed::ZERO when the player is too old to improve that archetype.
fn attr_growth_rate(archetype: AgeCurveArchetype, age_years: u32) -> Fixed {
    match archetype {
        AgeCurveArchetype::Physical => match age_years {
            ..=17 => Fixed::raw(1_000),
            18..=22 => Fixed::raw(700),
            23..=27 => Fixed::raw(350),
            _ => Fixed::ZERO, // too old to improve physically via training
        },
        AgeCurveArchetype::Technical => match age_years {
            ..=17 => Fixed::raw(700),
            18..=27 => Fixed::raw(1_000),
            28..=31 => Fixed::raw(600),
            32..=35 => Fixed::raw(300),
            _ => Fixed::raw(100),
        },
        AgeCurveArchetype::Mental => match age_years {
            ..=17 => Fixed::raw(500),
            18..=25 => Fixed::raw(700),
            26..=33 => Fixed::raw(900),
            34..=37 => Fixed::raw(500),
            _ => Fixed::raw(200),
        },
    }
}

/// Decay rate for an archetype at a given age when not actively trained.
///
/// Returns Fixed::ZERO when no decay applies.
fn attr_decay_rate(archetype: AgeCurveArchetype, age_years: u32) -> Fixed {
    match archetype {
        AgeCurveArchetype::Physical => match age_years {
            ..=27 => Fixed::ZERO,
            28..=31 => Fixed::raw(200),
            32..=35 => Fixed::raw(500),
            _ => Fixed::raw(800),
        },
        AgeCurveArchetype::Technical => match age_years {
            ..=31 => Fixed::ZERO,
            32..=35 => Fixed::raw(100),
            _ => Fixed::raw(250),
        },
        AgeCurveArchetype::Mental => match age_years {
            ..=35 => Fixed::ZERO,
            36..=39 => Fixed::raw(100),
            _ => Fixed::raw(200),
        },
    }
}

fn apply_passive_decay(
    players: &mut PlayerStore,
    pc_id: PlayerId,
    age_years: u32,
    _focus_attrs: &[AttrId],
    lifestyle: u8,
) {
    let decline_mult = lifestyle_decline(lifestyle);
    for (i, &archetype) in ATTR_ARCHETYPES.iter().enumerate() {
        let decay = attr_decay_rate(archetype, age_years);
        if decay == Fixed::ZERO {
            continue;
        }
        // Decay applies to all attrs regardless of training focus. Training adds
        // growth (in the loop above) but cannot stop age-related decline. For
        // Physical attrs, growth_rate is already ZERO after 27, so the two paths
        // are independent — focusing a Physical attr in the 30s does nothing but
        // does not slow its natural decline either.
        // Lifestyle bends the decline: pro burns out later/gentler, flashy earlier/steeper.
        let loss = BASE_DECAY_PER_WEEK * decay * decline_mult;
        let cur = players.get_current(pc_id, i);
        // Decay can never reduce below MIN_ATTR.
        players.current[i][pc_id] = (cur - loss).clamp(Fixed::MIN_ATTR, cur);
    }
}

/// Injury probability per 1 000 rolls.
fn injury_prob(energy: Fixed, intensity: Intensity, age_years: u32, lifestyle: u8) -> u32 {
    // Fatigue multiplier: 1.0 at full, 3.0 at 0 energy.
    let energy_pct = energy.to_int().clamp(0, 100) as u32;
    let fatigue_x10 = 10 + 20 * (100 - energy_pct) / 100;

    let intensity_x10 = match intensity {
        Intensity::Low => 5,
        Intensity::Medium => 10,
        Intensity::High => 15,
    };

    let age_x10 = match age_years {
        ..=20 => 8,
        21..=27 => 10,
        28..=32 => 13,
        _ => 16,
    };

    // Lifestyle multiplier (×10): a flashy lifestyle gets hurt more, a pro less.
    // Balanced is 10 (×1.0), so the divisor restores the pre-lifestyle baseline.
    let lifestyle_x10 = lifestyle_injury_x10(lifestyle);

    BASE_INJURY_PER_1000 * fatigue_x10 * intensity_x10 * age_x10 * lifestyle_x10 / 10_000
}

/// Injury-risk multiplier (×10) for a lifestyle (0=Pro, 2=Flashy, else Balanced).
fn lifestyle_injury_x10(lifestyle: u8) -> u32 {
    match lifestyle {
        0 => INJURY_LIFESTYLE_X10_PRO,
        2 => INJURY_LIFESTYLE_X10_FLASHY,
        _ => INJURY_LIFESTYLE_X10_BALANCED,
    }
}

/// Effective ceiling factor (fraction of potential reachable) for a lifestyle.
fn lifestyle_ceiling(lifestyle: u8) -> Fixed {
    match lifestyle {
        0 => LIFESTYLE_CEILING_PRO,
        2 => LIFESTYLE_CEILING_FLASHY,
        _ => LIFESTYLE_CEILING_BALANCED,
    }
}

/// Age-decline multiplier for a lifestyle: a pro declines gentler, a flashy steeper.
fn lifestyle_decline(lifestyle: u8) -> Fixed {
    match lifestyle {
        0 => DECLINE_LIFESTYLE_PRO,
        2 => DECLINE_LIFESTYLE_FLASHY,
        _ => DECLINE_LIFESTYLE_BALANCED,
    }
}

/// Energy factor on growth: 1.0 at full, 0.6 at empty.
fn energy_growth_factor(energy: Fixed) -> Fixed {
    let pct = energy / Fixed::from_int(100); // 0.0–1.0
    Fixed::raw(600) + Fixed::raw(400) * pct
}

/// Return the XP threshold to advance from the given tier.
fn fam_upgrade_threshold(tier: FamiliarityTier) -> Option<Fixed> {
    match tier {
        FamiliarityTier::Awkward => Some(FAM_XP_AWKWARD),
        FamiliarityTier::Unconvincing => Some(FAM_XP_UNCONVINCING),
        FamiliarityTier::Competent => Some(FAM_XP_COMPETENT),
        FamiliarityTier::Natural => None,
    }
}

/// Award familiarity XP for each role that overlaps with the focus attrs,
/// and process any tier upgrades.
fn update_familiarity(
    players: &mut PlayerStore,
    pc_id: PlayerId,
    focus_attrs: &[AttrId],
) -> Vec<DevelopmentEvent> {
    let mut events = Vec::new();
    for (r, weights) in ROLE_WEIGHT_TABLE.iter().enumerate() {
        let has_key = focus_attrs.iter().any(|&a| weights[a as usize] == W_KEY);
        let has_imp = focus_attrs.iter().any(|&a| weights[a as usize] == W_IMP);

        let xp_gain = if has_key {
            FAM_XP_KEY_PER_WEEK
        } else if has_imp {
            FAM_XP_IMP_PER_WEEK
        } else {
            Fixed::ZERO
        };

        if xp_gain == Fixed::ZERO {
            continue;
        }

        let current_xp = players.get_familiarity_xp(pc_id, r) + xp_gain;
        players.set_familiarity_xp(pc_id, r, current_xp);

        let tier = players.get_familiarity(pc_id, r);
        if let Some(threshold) = fam_upgrade_threshold(tier) {
            if current_xp >= threshold {
                if let Some(new_tier) = tier.upgrade() {
                    players.set_familiarity(pc_id, r, new_tier);
                    players.set_familiarity_xp(pc_id, r, Fixed::ZERO);
                    events.push(DevelopmentEvent::FamiliarityUpgrade {
                        role: RoleId::ALL[r],
                        new_tier,
                    });
                }
            }
        }
    }
    events
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attrs::NUM_ATTRS;
    use crate::generation::{generate_player, CreationChoices, Position};
    use crate::player::PlayerStore;
    use goat_rng::GoatRng;

    fn make_store_with_player(seed: u64, position: Position) -> (PlayerStore, PlayerId) {
        let choices = CreationChoices {
            name: "Test".into(),
            position,
            nationality: "English",
            club: "Riverside Town",
        };
        let view = generate_player(seed, &choices);
        let mut store = PlayerStore::new();
        let id = store.push(view);
        (store, id)
    }

    fn fwd_routine() -> Routine {
        Routine {
            focus_attrs: vec![AttrId::Finishing, AttrId::CloseControl, AttrId::BallControl],
            intensity: Intensity::Medium,
        }
    }

    #[test]
    fn energy_stays_in_bounds() {
        let (mut store, id) = make_store_with_player(1, Position::Forward);
        let routine = fwd_routine();
        let mut rng = GoatRng::new(42);
        for _ in 0..200 {
            advance_week(&mut store, id, &routine, Fixed::ONE, 1, &mut rng);
            let e = store.get_energy(id);
            assert!(e >= Fixed::ZERO, "energy below 0");
            assert!(e <= Fixed::from_int(100), "energy above 100");
        }
    }

    #[test]
    fn current_never_exceeds_potential() {
        let (mut store, id) = make_store_with_player(7, Position::Forward);
        let routine = fwd_routine();
        let mut rng = GoatRng::new(99);
        for _ in 0..500 {
            advance_week(&mut store, id, &routine, Fixed::ONE, 1, &mut rng);
        }
        for a in 0..NUM_ATTRS {
            assert!(
                store.get_current(id, a) <= store.get_potential(id, a),
                "current > potential for attr {a}"
            );
        }
    }

    #[test]
    fn attrs_stay_in_valid_range() {
        let (mut store, id) = make_store_with_player(42, Position::Defender);
        let routine = Routine {
            focus_attrs: vec![AttrId::StandingTackle, AttrId::Marking],
            intensity: Intensity::High,
        };
        let mut rng = GoatRng::new(1);
        for _ in 0..1_000 {
            advance_week(&mut store, id, &routine, Fixed::ONE, 1, &mut rng);
            for a in 0..NUM_ATTRS {
                let c = store.get_current(id, a);
                assert!(c >= Fixed::MIN_ATTR, "attr {a} below 1");
                assert!(c <= Fixed::MAX_ATTR, "attr {a} above 99");
            }
        }
    }

    #[test]
    fn resting_recovers_energy() {
        let (mut store, id) = make_store_with_player(3, Position::Midfielder);
        // Drain energy first
        let routine_high = Routine {
            focus_attrs: vec![AttrId::Vision],
            intensity: Intensity::High,
        };
        let mut rng = GoatRng::new(5);
        for _ in 0..20 {
            advance_week(&mut store, id, &routine_high, Fixed::ONE, 1, &mut rng);
        }
        let drained_energy = store.get_energy(id);

        // Now rest (no focus attrs, Low intensity)
        let routine_rest = Routine {
            focus_attrs: vec![],
            intensity: Intensity::Low,
        };
        for _ in 0..10 {
            advance_week(&mut store, id, &routine_rest, Fixed::ONE, 1, &mut rng);
        }
        let recovered_energy = store.get_energy(id);
        assert!(
            recovered_energy > drained_energy,
            "resting should recover energy: {drained_energy:?} → {recovered_energy:?}"
        );
    }

    #[test]
    fn old_player_physical_declines() {
        let (mut store, id) = make_store_with_player(10, Position::Forward);
        let routine = Routine {
            focus_attrs: vec![AttrId::Finishing], // not training physical attrs
            intensity: Intensity::Medium,
        };
        let mut rng = GoatRng::new(20);

        // Max out age to 30 (30*52 - starting 16*52 = 14*52 = 728 weeks)
        // Use a rest routine to age quickly without complicating growth
        let age_routine = Routine {
            focus_attrs: vec![],
            intensity: Intensity::Low,
        };
        for _ in 0..(14 * 52) {
            advance_week(&mut store, id, &age_routine, Fixed::ONE, 1, &mut rng);
        }
        let acc_at_30 = store.get_current(id, AttrId::Acceleration as usize);

        // Age another 8 years with physical unfocused
        for _ in 0..(8 * 52) {
            advance_week(&mut store, id, &routine, Fixed::ONE, 1, &mut rng);
        }
        let acc_at_38 = store.get_current(id, AttrId::Acceleration as usize);

        assert!(
            acc_at_38 <= acc_at_30,
            "physical attr should decline (or at worst stay) after 30: {acc_at_30:?} → {acc_at_38:?}"
        );
    }
}
