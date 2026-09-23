//! Tactical profiles — per-club team stats that drive match situation generation.
//!
//! A club's profile is *derived*, never stored: `derive(strength, club_id, world_seed)`
//! is deterministic, so the same club in the same world always plays the same style
//! (tiny-saves rule — anything derivable from seed is recomputed, not persisted).
//!
//! The match engine (`goat-match`) uses the two profiles to weight which situations
//! arise (stat-driven picking instead of flat random) and to scale contest
//! difficulty by the opponent's relevant stat.

use crate::attrs::{DEFENDING_ATTRS, DRIBBLING_ATTRS, NUM_ATTRS, PASSING_ATTRS, SHOOTING_ATTRS};
use crate::tuning::{
    NOISE_SALT, TACTICAL_JITTER, TACTICAL_STYLE_BASE, TACTICAL_STYLE_DOM_BOOST, TACTICAL_STYLE_SPAN,
};
use goat_fixed::Fixed;
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

    /// Derive a profile from an actual lineup's attributes (PA2 M1): the lines are
    /// the mean of the starters' real attribute groups — attack from
    /// shooting/dribbling, midfield from passing, defense from defending — so a
    /// club's playing strength on the pitch reflects its roster, not a static
    /// scalar. Style weights still come from `derive` (club identity is
    /// seed-stable and strength-independent).
    pub fn from_squad(avg_attrs: &[Fixed; NUM_ATTRS], club_id: u32, world_seed: u64) -> Self {
        let mean = |groups: &[&[usize]]| -> u8 {
            let (sum, n) = groups.iter().fold((0i32, 0i32), |(s, n), g| {
                (
                    s + g.iter().map(|&a| avg_attrs[a].to_int()).sum::<i32>(),
                    n + g.len() as i32,
                )
            });
            (sum / n.max(1)).clamp(1, 99) as u8
        };
        let mut lines = Self::derive(50, club_id, world_seed); // styles only
        lines.attack = mean(&[SHOOTING_ATTRS, DRIBBLING_ATTRS]);
        lines.midfield = mean(&[PASSING_ATTRS]);
        lines.defense = mean(&[DEFENDING_ATTRS]);
        lines
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

    /// The dominant (highest-weighted) style — first max wins, matching `derive`.
    pub fn dominant_style(&self) -> TacticalStyle {
        [
            TacticalStyle::Pressing,
            TacticalStyle::Possession,
            TacticalStyle::Counter,
            TacticalStyle::WingPlay,
        ]
        .into_iter()
        .max_by_key(|&s| self.style(s))
        .unwrap_or(TacticalStyle::Pressing)
    }

    /// Outfield slot split (defenders, midfielders, forwards — sums to 11) implied
    /// by the club's dominant style (PA2 M1.5: formation follows club identity):
    /// Pressing 4-3-4 · Possession 4-5-2 · Counter 5-4-2 · WingPlay 4-4-3.
    pub fn formation_slots(&self) -> (usize, usize, usize) {
        match self.dominant_style() {
            TacticalStyle::Pressing => (4, 3, 4),
            TacticalStyle::Possession => (4, 5, 2),
            TacticalStyle::Counter => (5, 4, 2),
            TacticalStyle::WingPlay => (4, 4, 3),
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

    #[test]
    fn from_squad_lines_reflect_attrs() {
        let weak = [Fixed::from_int(30); NUM_ATTRS];
        let strong = [Fixed::from_int(90); NUM_ATTRS];
        let w = TacticalProfile::from_squad(&weak, 7, 42);
        let s = TacticalProfile::from_squad(&strong, 7, 42);
        assert!(s.attack > w.attack + 40, "attack should track roster attrs");
        assert!(s.midfield > w.midfield + 40);
        assert!(s.defense > w.defense + 40);
        // Styles are club identity — identical for the same club regardless of roster.
        assert_eq!(w.pressing, s.pressing);
        assert_eq!(w.possession, s.possession);
        assert_eq!(w.counter, s.counter);
        assert_eq!(w.wing_play, s.wing_play);
    }

    #[test]
    fn from_squad_is_deterministic_and_bounded() {
        let attrs = [Fixed::from_int(60); NUM_ATTRS];
        let a = TacticalProfile::from_squad(&attrs, 3, 42);
        let b = TacticalProfile::from_squad(&attrs, 3, 42);
        assert_eq!(a, b);
        for line in [a.attack, a.midfield, a.defense] {
            assert!((1..=99).contains(&line));
        }
        // Uniform 60s across all groups → every line lands on 60.
        assert_eq!((a.attack, a.midfield, a.defense), (60, 60, 60));
    }

    #[test]
    fn formation_slots_sum_to_eleven_and_follow_dominant_style() {
        for seed in 0..50 {
            let p = TacticalProfile::derive(50, seed, 7);
            let (d, m, f) = p.formation_slots();
            assert_eq!(d + m + f, 11, "formation must fill all 11 slots");
            assert!(d >= 3 && m >= 2 && f >= 1, "sane minimums per line");
            // The dominant style decides the shape: pressing loads the front line,
            // counter loads the back line.
            match p.dominant_style() {
                TacticalStyle::Pressing => assert_eq!((d, m, f), (4, 3, 4)),
                TacticalStyle::Possession => assert_eq!((d, m, f), (4, 5, 2)),
                TacticalStyle::Counter => assert_eq!((d, m, f), (5, 4, 2)),
                TacticalStyle::WingPlay => assert_eq!((d, m, f), (4, 4, 3)),
            }
        }
    }
}
