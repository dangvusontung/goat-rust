#![forbid(unsafe_code)]
//! Core trait types: mastery tiers, trait identities, per-player state.
//!
//! Implements §A.1 (traits ≠ attributes), §A.2 (five engine hook types),
//! §A.3 (three acquisition paths), §A.4 (hidden ceilings).

use goat_rng::RngSource;

use crate::tuning::{
    CEIL_MASTERY_PER_100, CEIL_PROFICIENT_PER_100, CEIL_RAW_PER_100, START_RAW_IF_MASTERY_CEIL,
};

// ── Mastery tier ──────────────────────────────────────────────────────────────

/// Mastery tier for a single trait.
///
/// Mirrors the familiarity ladder (§5.2) — four tiers, each opening new
/// capabilities rather than just improving existing ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum MasteryTier {
    /// Player cannot use this trait at all.
    #[default]
    None,
    /// Trait is active but rough — the relevant branch appears at a contest penalty.
    Raw,
    /// Contest is balanced; trait is a real, viable tool.
    Proficient,
    /// Strong buff; new sub-choices and combinations unlock that lower tiers lack.
    Mastery,
}

impl MasteryTier {
    pub const ALL: [MasteryTier; 4] = [Self::None, Self::Raw, Self::Proficient, Self::Mastery];

    pub fn upgrade(self) -> Option<Self> {
        match self {
            Self::None => Some(Self::Raw),
            Self::Raw => Some(Self::Proficient),
            Self::Proficient => Some(Self::Mastery),
            Self::Mastery => None,
        }
    }

    pub fn is_active(self) -> bool {
        self != Self::None
    }

    /// Beat-summoner bonus as a percentage added to the base bias weight.
    ///
    /// None = 0%, Raw = +25%, Proficient = +75%, Mastery = +150%.
    /// TUNABLE — these are placeholder feel-first values (§A.2, §11).
    pub fn beat_summoner_bonus_pct(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Raw => 25,
            Self::Proficient => 75,
            Self::Mastery => 150,
        }
    }

    pub fn as_u8(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Raw => 1,
            Self::Proficient => 2,
            Self::Mastery => 3,
        }
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Raw,
            2 => Self::Proficient,
            3 => Self::Mastery,
            _ => Self::None,
        }
    }
}

// ── Trait identity ─────────────────────────────────────────────────────────────

/// How many traits exist in Phase 1.
pub const NUM_TRAITS: usize = 10;

/// A player's special skill identity (§A.5 catalogue, illustrative selection).
///
/// Phase 1 implements six beat-summoner traits with full hook wiring, plus four
/// trait identities whose hooks are scaffolded but not yet active (Phase 2+).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraitId {
    // ── Beat-summoner traits (§A.2 hook type 2) — fully wired in Phase 1 ──────
    /// Drift into space, arrive unmarked in the box. Experiential. (§A.5 Ghost)
    Ghost,
    /// Loose-ball-in-the-six-yard-box beats fire more. Trainable. (§A.5 Poacher)
    Poacher,
    /// Read the attack, cut lanes, cover. Experiential. (§A.5 Reader)
    Reader,
    /// Receive in midfield, open a chance. Trainable. (§A.5 Playmaker)
    Playmaker,
    /// Aerial-duel beats fire in both boxes. Physical/trainable. (§A.5 Aerial Dominator)
    AerialDominator,
    /// Tackle/interception beats fire more. Trainable. (§A.5 Ball-Winner)
    BallWinner,

    // ── Other hook types — types exist, hooks scaffolded for Phase 2 ──────────
    /// Unlocks the curled finish on edge-of-box chances. Trainable. (§A.5, hook type 1)
    FinesseMerchant,
    /// Resists Nerves; lifts Flow baseline in finals/derbies. Experiential. (§A.5, hook type 4)
    BigGamePlayer,
    /// Stamina drains slowly; holds tempo through the full 90. Physical. (§A.5, hook type 5)
    Engine,
    /// Reduces headspace dependence when finishing. Trainable. (§A.5, hook type 5)
    Clinical,
}

impl TraitId {
    pub const ALL: [TraitId; NUM_TRAITS] = [
        TraitId::Ghost,
        TraitId::Poacher,
        TraitId::Reader,
        TraitId::Playmaker,
        TraitId::AerialDominator,
        TraitId::BallWinner,
        TraitId::FinesseMerchant,
        TraitId::BigGamePlayer,
        TraitId::Engine,
        TraitId::Clinical,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Ghost => "Ghost (Raumdeuter)",
            Self::Poacher => "Poacher",
            Self::Reader => "Reader",
            Self::Playmaker => "Playmaker",
            Self::AerialDominator => "Aerial Dominator",
            Self::BallWinner => "Ball-Winner",
            Self::FinesseMerchant => "Finesse Merchant",
            Self::BigGamePlayer => "Big-Game Player",
            Self::Engine => "Engine",
            Self::Clinical => "Clinical",
        }
    }

    /// Which beat-situation tags this trait boosts via the beat-summoner hook (§A.2).
    ///
    /// Returns an empty slice for traits whose hook is not beat-summoner — those
    /// traits are real but their engine hooks land in Phase 2+.
    pub fn beat_summoner_tags(self) -> &'static [&'static str] {
        match self {
            Self::Ghost => &["positioning", "attack"],
            Self::Poacher => &["shooting"],
            Self::Reader => &["reading", "defend"],
            Self::Playmaker => &["passing", "attack"],
            Self::AerialDominator => &["aerial"],
            Self::BallWinner => &["tackle", "defend"],
            // Non-beat-summoner — hook is a different type (Phase 2+)
            Self::FinesseMerchant | Self::BigGamePlayer | Self::Engine | Self::Clinical => &[],
        }
    }
}

// ── Per-player trait state ─────────────────────────────────────────────────────

/// Complete trait state for one player.
///
/// `ceiling` is the maximum tier this player can ever reach (rolled at creation,
/// never shown to the player — §A.4). `current` is where they are now (earned over
/// the career). `current[i] <= ceiling[i]` is an invariant.
#[derive(Debug, Clone, Copy)]
pub struct PlayerTraits {
    pub current: [MasteryTier; NUM_TRAITS],
    pub ceiling: [MasteryTier; NUM_TRAITS],
}

impl Default for PlayerTraits {
    fn default() -> Self {
        Self {
            current: [MasteryTier::None; NUM_TRAITS],
            ceiling: [MasteryTier::None; NUM_TRAITS],
        }
    }
}

impl PlayerTraits {
    /// Roll trait ceilings from a creation seed (§A.3 aptitude roll).
    ///
    /// The seed must be the player's creation seed so trait aptitude is part of
    /// the same fate-roll as attribute potential (§5.3, §2.4). XOR with the
    /// domain constant isolates trait rolls from attribute rolls on the same seed.
    pub fn roll_from_seed(seed: u64, rng: &mut impl RngSource) -> Self {
        let mut ceiling = [MasteryTier::None; NUM_TRAITS];
        let mut current = [MasteryTier::None; NUM_TRAITS];

        for i in 0..NUM_TRAITS {
            // Roll ceiling (the aptitude — §A.3).
            let roll = rng.next_range_u64(0, 99) as u32;
            ceiling[i] = if roll < CEIL_MASTERY_PER_100 {
                MasteryTier::Mastery
            } else if roll < CEIL_MASTERY_PER_100 + CEIL_PROFICIENT_PER_100 {
                MasteryTier::Proficient
            } else if roll < CEIL_MASTERY_PER_100 + CEIL_PROFICIENT_PER_100 + CEIL_RAW_PER_100 {
                MasteryTier::Raw
            } else {
                MasteryTier::None
            };

            // A player with Mastery ceiling has a small chance to arrive already at Raw
            // ("raised on it from childhood" — §A.3). Prodigies, not the norm.
            if ceiling[i] == MasteryTier::Mastery
                && rng.next_range_u64(0, 99) < START_RAW_IF_MASTERY_CEIL as u64
            {
                current[i] = MasteryTier::Raw;
            }
        }

        let _ = seed; // seed already consumed by caller to construct rng
        Self { current, ceiling }
    }

    /// Trait name + tier strings for display (for a simple traits screen).
    pub fn display_lines(&self) -> impl Iterator<Item = (&'static str, MasteryTier)> + '_ {
        TraitId::ALL.iter().enumerate().filter_map(|(i, &t)| {
            if self.current[i].is_active() {
                Some((t.name(), self.current[i]))
            } else {
                None
            }
        })
    }

    /// Compute the beat-summoner bias bonus for a given set of situation tags.
    ///
    /// Returns the total additive percentage bonus to apply to the situation's
    /// base bias weight. Multiple traits can stack additively.
    pub fn beat_summoner_bonus_pct(&self, tags: &[String]) -> u32 {
        let mut total = 0u32;
        for (i, trait_id) in TraitId::ALL.iter().enumerate() {
            let tier = self.current[i];
            if !tier.is_active() {
                continue;
            }
            let boost_tags = trait_id.beat_summoner_tags();
            if boost_tags.is_empty() {
                continue;
            }
            if tags.iter().any(|t| boost_tags.contains(&t.as_str())) {
                total = total.saturating_add(tier.beat_summoner_bonus_pct());
            }
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use goat_rng::GoatRng;

    #[test]
    fn tier_ordering_is_sane() {
        assert!(MasteryTier::None < MasteryTier::Raw);
        assert!(MasteryTier::Raw < MasteryTier::Proficient);
        assert!(MasteryTier::Proficient < MasteryTier::Mastery);
    }

    #[test]
    fn upgrade_chain_is_complete() {
        let mut t = MasteryTier::None;
        t = t.upgrade().unwrap();
        assert_eq!(t, MasteryTier::Raw);
        t = t.upgrade().unwrap();
        assert_eq!(t, MasteryTier::Proficient);
        t = t.upgrade().unwrap();
        assert_eq!(t, MasteryTier::Mastery);
        assert!(t.upgrade().is_none());
    }

    #[test]
    fn beat_summoner_bonus_zero_when_none() {
        let traits = PlayerTraits::default();
        let tags = vec!["positioning".to_string(), "attack".to_string()];
        assert_eq!(traits.beat_summoner_bonus_pct(&tags), 0);
    }

    #[test]
    fn ghost_proficient_boosts_positioning_beats() {
        let mut traits = PlayerTraits::default();
        traits.current[TraitId::ALL
            .iter()
            .position(|&t| t == TraitId::Ghost)
            .unwrap()] = MasteryTier::Proficient;
        let tags = vec!["positioning".to_string()];
        assert_eq!(traits.beat_summoner_bonus_pct(&tags), 75);
    }

    #[test]
    fn roundtrip_tier_as_u8() {
        for tier in MasteryTier::ALL {
            assert_eq!(MasteryTier::from_u8(tier.as_u8()), tier);
        }
    }

    #[test]
    fn roll_respects_ceiling_invariant() {
        let mut rng = GoatRng::new(42);
        let traits = PlayerTraits::roll_from_seed(42, &mut rng);
        for i in 0..NUM_TRAITS {
            assert!(
                traits.current[i] <= traits.ceiling[i],
                "current[{i}]={:?} > ceiling[{i}]={:?}",
                traits.current[i],
                traits.ceiling[i]
            );
        }
    }

    #[test]
    fn non_beat_summoner_traits_boost_nothing() {
        let mut traits = PlayerTraits::default();
        // Set FinesseMerchant to Mastery — should not affect any beat tags
        let fm_idx = TraitId::ALL
            .iter()
            .position(|&t| t == TraitId::FinesseMerchant)
            .unwrap();
        traits.current[fm_idx] = MasteryTier::Mastery;
        // All possible tags from beats.json
        let all_tags: Vec<String> = vec![
            "attack",
            "defend",
            "setpiece",
            "positioning",
            "key",
            "aerial",
            "delivery",
            "discipline",
            "dribble",
            "passing",
            "physical",
            "pressing",
            "reading",
            "shooting",
            "tackle",
            "wide",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(traits.beat_summoner_bonus_pct(&all_tags), 0);
    }
}
