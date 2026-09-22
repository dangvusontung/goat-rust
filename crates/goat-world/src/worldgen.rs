//! Procedural world generation — World Scale-Up Phase A.
//!
//! Generates the full football world (50 nations, 3–4 divisions each, 16 clubs
//! per division) deterministically from `world_seed`. Nation list, powers and
//! division counts are fixed (see `nations.rs`); only club names and the
//! strength jitter draw from the seed.
//!
//! Strength shape (decision 5 in docs/WORLD-SCALE-UP.md):
//! ```text
//! club_strength = nation_power − DIV_DROP[level] + rank_bonus + jitter
//! ```
//! where `rank_bonus` is a quadratic elite skew inside the division (the best
//! club of a division stands well above its tail), clamped to `1..=99`.
//!
//! This module is ADDITIVE in Phase A milestone 1: the legacy const world in
//! `world.rs` still drives the game. Milestone 2 swaps consumers over.

use crate::nations::{patterns_for, NationDef, NATIONS};
use goat_rng::{GoatRng, RngSource};

/// Clubs per division — same as the legacy world so downstream shape
/// (fixtures, tables) can be reused in milestone 2.
pub const GEN_CLUBS_PER_DIV: usize = 16;

/// Total generated club count across all nations and divisions.
pub const GEN_NUM_CLUBS: usize = 2544; // 9 nations × 4 divs + 41 nations × 3 divs, ×16

/// Strength drop per division level below the nation's top division.
/// Top division anchors at `nation_power − 8` so even elite nations have a
/// spread inside their top flight (England top div ≈ 87–99, not a flat 95).
const DIV_DROP: [i32; 4] = [8, 28, 44, 58];
/// Peak rank bonus for the best club of each division level (quadratic skew
/// down to 0 for the last club).
const RANK_SPAN: [i32; 4] = [12, 8, 6, 5];
/// Deterministic jitter range, ±JITTER.
const JITTER: i32 = 2;

/// Division title suffixes by level.
const DIV_TITLES: [&str; 4] = [
    "Premier Division",
    "First Division",
    "Second Division",
    "Third Division",
];

/// One generated club.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenClub {
    /// Index into `GeneratedWorld::clubs`.
    pub id: usize,
    pub name: String,
    /// Index into `NATIONS`.
    pub nation: u8,
    /// 0 = top division of the nation.
    pub div_level: u8,
    /// 1–99 (post-clamp).
    pub strength: u8,
}

/// One generated division.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenDivision {
    pub nation: u8,
    pub level: u8,
    pub name: String,
    /// Club ids in strength order (rank 0 = strongest).
    pub clubs: Vec<usize>,
}

/// The full generated world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedWorld {
    pub divisions: Vec<GenDivision>,
    pub clubs: Vec<GenClub>,
}

impl GeneratedWorld {
    pub fn nation(&self, club_id: usize) -> &'static NationDef {
        &NATIONS[self.clubs[club_id].nation as usize]
    }

    /// Deterministic fingerprint over names + strengths (for golden tests).
    pub fn fingerprint(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for c in &self.clubs {
            for b in c.name.as_bytes() {
                h = (h ^ *b as u64).wrapping_mul(0x0000_0100_0000_01b3);
            }
            h = (h ^ c.strength as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }
}

/// Pyramid strength for one club slot. `rank` is the club's position inside
/// its division (0 = strongest slot).
pub fn pyramid_strength(nation_power: u8, level: u8, rank: usize, rng: &mut impl RngSource) -> u8 {
    let base = nation_power as i32 - DIV_DROP[level as usize];
    let rem = (GEN_CLUBS_PER_DIV - 1 - rank) as i32; // 15..=0
    let span = RANK_SPAN[level as usize];
    // Quadratic elite skew: rank 0 gets the full span, the tail gets ~0.
    let bonus =
        span * rem * rem / ((GEN_CLUBS_PER_DIV as i32 - 1) * (GEN_CLUBS_PER_DIV as i32 - 1));
    let jitter = rng.next_range_u32(0, (2 * JITTER) as u32) as i32 - JITTER;
    (base + bonus + jitter).clamp(1, 99) as u8
}

/// Pick a unique club name within one nation: random (city, pattern) start,
/// then a deterministic walk over the remaining combos on collision.
fn pick_name(nation: &NationDef, used: &mut Vec<String>, rng: &mut impl RngSource) -> String {
    let cities = nation.cities;
    let pats = patterns_for(nation.style);
    let city0 = rng.next_range_u32(0, cities.len() as u32 - 1) as usize;
    let pat0 = rng.next_range_u32(0, pats.len() as u32 - 1) as usize;
    let combos = cities.len() * pats.len();
    for attempt in 0..combos {
        let idx = (city0 * pats.len() + pat0 + attempt) % combos;
        let pat = pats[idx % pats.len()];
        let city = cities[idx / pats.len()];
        // Skip awkward doubles: the pattern's literal word already in the city
        // name ("Ho Chi Minh City City", "United United").
        let literal = pat.replace("{c}", "").trim().to_string();
        if !literal.is_empty() && city.contains(&literal) {
            continue;
        }
        let name = pat.replace("{c}", city);
        if !used.contains(&name) {
            used.push(name.clone());
            return name;
        }
    }
    // Unreachable given cities × patterns >= 64, but never panic at genesis.
    let fallback = format!("{} FC {}", cities[city0], used.len());
    used.push(fallback.clone());
    fallback
}

/// Generate the full world from a seed. Deterministic: same seed → identical
/// world (names, strengths, division rosters).
pub fn generate_world(world_seed: u64) -> GeneratedWorld {
    let mut rng = GoatRng::new(world_seed ^ 0x5EED_A0A1_D5C4_1E00);
    let mut clubs = Vec::with_capacity(GEN_NUM_CLUBS);
    let mut divisions = Vec::with_capacity(NATIONS.iter().map(|n| n.divisions as usize).sum());

    for (n_idx, nation) in NATIONS.iter().enumerate() {
        let mut used_names: Vec<String> = Vec::with_capacity(nation.divisions as usize * 16);
        for level in 0..nation.divisions {
            let mut div_club_ids = Vec::with_capacity(GEN_CLUBS_PER_DIV);
            for rank in 0..GEN_CLUBS_PER_DIV {
                let strength = pyramid_strength(nation.power, level, rank, &mut rng);
                let name = pick_name(nation, &mut used_names, &mut rng);
                let id = clubs.len();
                clubs.push(GenClub {
                    id,
                    name,
                    nation: n_idx as u8,
                    div_level: level,
                    strength,
                });
                div_club_ids.push(id);
            }
            divisions.push(GenDivision {
                nation: n_idx as u8,
                level,
                name: format!("{} {}", nation.name, DIV_TITLES[level as usize]),
                clubs: div_club_ids,
            });
        }
    }

    GeneratedWorld { divisions, clubs }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn worldgen_is_deterministic() {
        let a = generate_world(42);
        let b = generate_world(42);
        assert_eq!(a, b, "same seed must produce a bit-identical world");
        let c = generate_world(43);
        assert_ne!(
            a.fingerprint(),
            c.fingerprint(),
            "different seed must differ"
        );
    }

    #[test]
    fn world_size_matches_spec() {
        let w = generate_world(7);
        let expected_divs: usize = NATIONS.iter().map(|n| n.divisions as usize).sum();
        assert_eq!(w.divisions.len(), expected_divs);
        assert_eq!(w.clubs.len(), expected_divs * GEN_CLUBS_PER_DIV);
        for d in &w.divisions {
            assert_eq!(d.clubs.len(), GEN_CLUBS_PER_DIV);
        }
    }

    #[test]
    fn strength_bounds_hold() {
        let w = generate_world(99);
        for c in &w.clubs {
            assert!((1..=99).contains(&c.strength), "{} out of range", c.name);
        }
    }

    #[test]
    fn pyramid_shape_holds_within_nations() {
        let w = generate_world(7);
        for (n_idx, nation) in NATIONS.iter().enumerate() {
            let avg = |level: u8| -> i64 {
                let div = w
                    .divisions
                    .iter()
                    .find(|d| d.nation == n_idx as u8 && d.level == level)
                    .unwrap();
                div.clubs
                    .iter()
                    .map(|&c| w.clubs[c].strength as i64)
                    .sum::<i64>()
                    / GEN_CLUBS_PER_DIV as i64
            };
            for level in 0..nation.divisions - 1 {
                assert!(
                    avg(level) > avg(level + 1),
                    "{}: div {} avg {} must exceed div {} avg {}",
                    nation.name,
                    level,
                    avg(level),
                    level + 1,
                    avg(level + 1)
                );
            }
        }
    }

    #[test]
    fn nation_power_ordering_respected() {
        // UEFA-coefficient effect: a weak nation's BEST club must be weaker
        // than an elite nation's top-division average.
        let w = generate_world(7);
        let elite_avg: i64 = {
            let eng = w
                .divisions
                .iter()
                .find(|d| d.nation == 0 && d.level == 0)
                .unwrap();
            eng.clubs
                .iter()
                .map(|&c| w.clubs[c].strength as i64)
                .sum::<i64>()
                / GEN_CLUBS_PER_DIV as i64
        };
        for (n_idx, nation) in NATIONS.iter().enumerate().filter(|(_, n)| n.power <= 60) {
            let top = w
                .divisions
                .iter()
                .find(|d| d.nation == n_idx as u8 && d.level == 0)
                .unwrap();
            let best = top
                .clubs
                .iter()
                .map(|&c| w.clubs[c].strength)
                .max()
                .unwrap();
            assert!(
                best as i64 <= elite_avg,
                "{} best club ({}) must not exceed England top-div average ({})",
                nation.name,
                best,
                elite_avg
            );
        }
    }

    #[test]
    fn names_unique_within_nation() {
        let w = generate_world(7);
        for (n_idx, nation) in NATIONS.iter().enumerate() {
            let names: HashSet<&str> = w
                .clubs
                .iter()
                .filter(|c| c.nation == n_idx as u8)
                .map(|c| c.name.as_str())
                .collect();
            let total = nation.divisions as usize * GEN_CLUBS_PER_DIV;
            assert_eq!(
                names.len(),
                total,
                "{} has duplicate club names",
                nation.name
            );
        }
    }
}
