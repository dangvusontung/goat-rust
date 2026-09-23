//! Squad sheets — the 22 real individuals on the pitch (PA2 M2).
//!
//! Until M2 the engine only ever saw two `TacticalProfile` line scalars; every
//! teammate/opponent was anonymous. A `SquadSheet` hands the engine the actual
//! starting XI of each side — names, positions, full attribute sets — so a
//! contest can be resolved against a *specific* opponent (MATCH.md A.5) and the
//! commentary can name real people.
//!
//! The sheet is pure data: goat-match never derives names itself (that would
//! drag in goat-world — the layering stays engine ← data). The live game builds
//! real sheets from the population (`goat-tui`); harnesses and tests use
//! `SquadSheet::stub`, which synthesises a deterministic squad around a target
//! strength from a small built-in name pool.

use goat_core::attrs::NUM_ATTRS;
use goat_fixed::Fixed;
use goat_rng::{GoatRng, RngSource};

/// Position groups, matching the population convention (0=DEF, 1=MID, 2=FWD).
pub const POS_DEF: u8 = 0;
pub const POS_MID: u8 = 1;
pub const POS_FWD: u8 = 2;

/// One named starter with his full current attribute set.
#[derive(Debug, Clone)]
pub struct SquadPlayer {
    pub name: String,
    /// 0=Defender, 1=Midfielder, 2=Forward.
    pub position: u8,
    pub attrs: [Fixed; NUM_ATTRS],
    /// True for the PC's own entry in his team's sheet (excluded from
    /// teammate-name draws — he is "you", never "{scorer}").
    pub is_pc: bool,
    /// Population index when the sheet is built from the real world (PA2 M3) —
    /// the key goal credits are persisted against. `None` for stub sheets and
    /// for the PC (his stats live in WorldState, not the population).
    pub id: Option<u32>,
}

/// A starting XI, plus (for the live game) a few named bench players. The
/// engine only ever reads the XI; all picks are made through the match RNG so
/// a match stays a pure function of (setup, rng). The bench is used solely by
/// the opposition-substitution rule (PA2 M4 follow-up) — harnesses and the
/// golden match leave it empty, which disables that rule entirely.
#[derive(Debug, Clone)]
pub struct SquadSheet {
    pub players: Vec<SquadPlayer>,
    /// Named substitutes the opposition manager can throw on while chasing the
    /// game. Empty in every harness/test sheet.
    pub bench: Vec<SquadPlayer>,
}

/// Small built-in pool for `stub` — enough variety that synthetic commentary
/// doesn't repeat one name, no goat-world dependency.
const STUB_NAMES: [&str; 22] = [
    "R. Stone",
    "M. Kessler",
    "D. Varga",
    "T. Okafor",
    "S. Lindqvist",
    "P. Moreau",
    "J. Hartley",
    "A. Novak",
    "K. Tanaka",
    "L. Ferreira",
    "G. Walsh",
    "E. Duarte",
    "N. Petrov",
    "C. Ankersen",
    "B. Sylla",
    "F. Marchetti",
    "O. Riedel",
    "H. Bakker",
    "V. Costa",
    "I. Kowal",
    "U. Brandt",
    "Y. Haddad",
];

impl SquadSheet {
    /// Deterministic synthetic squad for harnesses/tests: `formation` outfield
    /// slots (D/M/F, e.g. (4,3,3)) + 1 abstract keeper slot filled by an extra
    /// defender-line entry. Attrs are `strength ± 12` seeded per player/attr.
    pub fn stub(strength: u8, seed: u64, formation: (usize, usize, usize)) -> Self {
        let mut players = Vec::with_capacity(11);
        let mut name_idx = 0usize;
        let mut push_group = |pos: u8, n: usize, players: &mut Vec<SquadPlayer>| {
            for _ in 0..n {
                let pseed = seed
                    .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    .wrapping_add(name_idx as u64 + 1);
                let mut rng = GoatRng::new(pseed);
                let mut attrs = [Fixed::ZERO; NUM_ATTRS];
                for a in attrs.iter_mut() {
                    let var = rng.next_range_u32(0, 24) as i32 - 12;
                    *a = Fixed::from_int((strength as i32 + var).clamp(20, 99));
                }
                players.push(SquadPlayer {
                    name: STUB_NAMES[name_idx % STUB_NAMES.len()].to_string(),
                    position: pos,
                    attrs,
                    is_pc: false,
                    id: None,
                });
                name_idx += 1;
            }
        };
        // The abstract keeper counts as a defensive-line entry (see formation
        // convention in `TacticalProfile::formation_slots`).
        push_group(POS_DEF, formation.0 + 1, &mut players);
        push_group(POS_MID, formation.1, &mut players);
        push_group(POS_FWD, formation.2, &mut players);
        Self {
            players,
            bench: Vec::new(),
        }
    }

    /// Indices of starters in one position group.
    pub fn group(&self, pos: u8) -> Vec<usize> {
        (0..self.players.len())
            .filter(|&i| self.players[i].position == pos)
            .collect()
    }

    /// Total attribute sum — the rough "how good is he right now" number used
    /// by the opposition-substitution rule.
    fn attr_sum(p: &SquadPlayer) -> i64 {
        p.attrs.iter().map(|a| a.to_int() as i64).sum()
    }

    /// Index of the weakest starter (lowest attribute sum) — the man a chasing
    /// manager takes off first.
    pub fn weakest_starter(&self) -> Option<usize> {
        (0..self.players.len()).min_by_key(|&i| Self::attr_sum(&self.players[i]))
    }

    /// Index of the best bench player in a position group, if any.
    pub fn best_bench_at(&self, pos: u8) -> Option<usize> {
        (0..self.bench.len())
            .filter(|&i| self.bench[i].position == pos)
            .max_by_key(|&i| Self::attr_sum(&self.bench[i]))
    }

    /// Position-weighted pick of a teammate (used for {scorer}/{assist} on the
    /// PC's side): forwards 3, midfielders 2, defenders 1; the PC is never drawn.
    pub fn pick_teammate(&self, rng: &mut impl RngSource) -> Option<&SquadPlayer> {
        let weight = |p: &SquadPlayer| {
            if p.is_pc {
                0
            } else {
                match p.position {
                    POS_FWD => 3u64,
                    POS_MID => 2,
                    _ => 1,
                }
            }
        };
        let total: u64 = self.players.iter().map(weight).sum();
        if total == 0 {
            return None;
        }
        let mut roll = rng.next_range_u64(0, total - 1);
        self.players.iter().find(|p| {
            let w = weight(p);
            if roll < w {
                true
            } else {
                roll -= w;
                false
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_is_deterministic_and_full_size() {
        let a = SquadSheet::stub(70, 42, (4, 3, 3));
        let b = SquadSheet::stub(70, 42, (4, 3, 3));
        assert_eq!(a.players.len(), 11);
        assert_eq!(b.players.len(), 11);
        for (x, y) in a.players.iter().zip(b.players.iter()) {
            assert_eq!(x.name, y.name);
            assert_eq!(x.attrs, y.attrs);
        }
        assert_eq!(a.group(POS_DEF).len(), 5, "keeper slot counts as defensive");
        assert_eq!(a.group(POS_MID).len(), 3);
        assert_eq!(a.group(POS_FWD).len(), 3);
    }

    #[test]
    fn stub_strength_tracks_target() {
        let weak = SquadSheet::stub(35, 7, (4, 3, 3));
        let strong = SquadSheet::stub(85, 7, (4, 3, 3));
        let mean = |s: &SquadSheet| {
            s.players
                .iter()
                .flat_map(|p| p.attrs.iter())
                .map(|a| a.to_int() as i64)
                .sum::<i64>()
        };
        assert!(mean(&strong) > mean(&weak));
    }

    #[test]
    fn pick_teammate_never_draws_pc_and_prefers_forwards() {
        let mut sheet = SquadSheet::stub(70, 5, (4, 3, 3));
        sheet.players[0].is_pc = true;
        let mut fwd = 0u32;
        for seed in 0..300u64 {
            let mut rng = GoatRng::new(seed);
            let p = sheet.pick_teammate(&mut rng).unwrap();
            assert!(!p.is_pc, "the PC is never a scorer slot");
            if p.position == POS_FWD {
                fwd += 1;
            }
        }
        // Forwards carry weight 3 of (5×1 + 3×2 + 3×3) = 20 → 9/20 = 45%.
        assert!(
            fwd > 110,
            "forwards should dominate scorer picks: {fwd}/300"
        );
    }
}
