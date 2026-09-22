//! Tactical profiles — per-club team stats that drive match situation generation.
//!
//! A club's profile is *derived*, never stored: `derive(strength, club_id, world_seed)`
//! is deterministic, so the same club in the same world always plays the same style
//! (tiny-saves rule — anything derivable from seed is recomputed, not persisted).
//!
//! The match engine (`goat-match`) uses the two profiles to weight which situations
//! arise (stat-driven picking instead of flat random) and to scale contest
//! difficulty by the opponent's relevant stat.

use crate::tuning::{
    NOISE_SALT, TACTICAL_JITTER, TACTICAL_STYLE_BASE, TACTICAL_STYLE_DOM_BOOST, TACTICAL_STYLE_SPAN,
};
use goat_rng::{GoatRng, RngSource};

/// The four team styles a profile mixes. Order is load-bearing (indexed draws).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum TacticalStyle {
    Pressing = 0,
    Possession = 1,
    Counter = 2,
    WingPlay = 3,
}

pub const NUM_STYLES: usize = 4;

/// A club's tactical identity: line strengths plus style weights (1–99 each).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TacticalProfile {
    pub attack: u8,
    pub midfield: u8,
    pub defense: u8,
    pub pressing: u8,
    pub possession: u8,
    pub counter: u8,
    pub wing_play: u8,
}

impl TacticalProfile {
    /// Derive a club's profile deterministically from its strength, id, and the
    /// world seed. Draw order is fixed (attack, midfield, defense, then the four
    /// style bases in enum order, then the dominant-style boost) — changing it
    /// changes every profile in the world.
    pub fn derive(strength: u8, club_id: u32, world_seed: u64) -> Self {
        let mut rng = GoatRng::new(world_seed ^ NOISE_SALT.wrapping_mul(club_id as u64 + 1));

        let mut line = || {
            let j =
                rng.next_range_u64(0, (2 * TACTICAL_JITTER) as u64) as i32 - TACTICAL_JITTER as i32;
            (strength as i32 + j).clamp(1, 99) as u8
        };
        let attack = line();
        let midfield = line();
        let defense = line();

        let mut styles = [0u8; NUM_STYLES];
        for s in &mut styles {
            *s = TACTICAL_STYLE_BASE + rng.next_range_u8(0, TACTICAL_STYLE_SPAN);
        }
        // The highest-rolled style becomes the club's identity (first max wins —
        // deterministic tie-break).
        let dominant = styles
            .iter()
            .enumerate()
            .max_by_key(|(_, w)| *w)
            .map(|(i, _)| i)
            .unwrap_or(0);
        styles[dominant] = styles[dominant]
            .saturating_add(TACTICAL_STYLE_DOM_BOOST)
            .min(99);

        Self {
            attack,
            midfield,
            defense,
            pressing: styles[TacticalStyle::Pressing as usize],
            possession: styles[TacticalStyle::Possession as usize],
            counter: styles[TacticalStyle::Counter as usize],
            wing_play: styles[TacticalStyle::WingPlay as usize],
        }
    }

    /// Weight of one style in this profile.
    pub fn style(&self, s: TacticalStyle) -> u8 {
        match s {
            TacticalStyle::Pressing => self.pressing,
            TacticalStyle::Possession => self.possession,
            TacticalStyle::Counter => self.counter,
            TacticalStyle::WingPlay => self.wing_play,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_is_deterministic() {
        let a = TacticalProfile::derive(75, 3, 42);
        let b = TacticalProfile::derive(75, 3, 42);
        assert_eq!(a, b);
    }

    #[test]
    fn derive_varies_by_club_and_seed() {
        let base = TacticalProfile::derive(75, 3, 42);
        assert_ne!(base, TacticalProfile::derive(75, 4, 42));
        assert_ne!(base, TacticalProfile::derive(75, 3, 43));
    }

    #[test]
    fn lines_stay_near_strength() {
        for seed in 0..20 {
            let p = TacticalProfile::derive(60, seed, 7);
            for line in [p.attack, p.midfield, p.defense] {
                let d = (line as i32 - 60).abs();
                assert!(
                    d <= TACTICAL_JITTER as i32,
                    "line {line} too far from strength"
                );
            }
        }
    }

    #[test]
    fn exactly_one_dominant_style() {
        let p = TacticalProfile::derive(50, 0, 1);
        let styles = [p.pressing, p.possession, p.counter, p.wing_play];
        let max = *styles.iter().max().unwrap();
        assert!(max >= TACTICAL_STYLE_BASE + TACTICAL_STYLE_DOM_BOOST.min(99));
        assert_eq!(styles.iter().filter(|&&w| w == max).count(), 1);
    }
}
