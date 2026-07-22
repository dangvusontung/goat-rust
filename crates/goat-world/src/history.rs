//! Seeded history backfill + canon (TASK-09A Slice 9A.4, bible §7 / feeds Phase-7 pantheon).
//!
//! Before the player's career starts there are decades of *internally consistent* past
//! seasons: league champions, an annual Ballon d'Or, and a canon of past greats whose
//! multiple awards form a believable career arc. It is a **pure function of `world_seed`**,
//! so nothing here needs persisting — it is recomputed on demand (tiny-saves §9: derive,
//! don't store). The same seed yields the same canon on every platform, pinned by
//! `History::fingerprint`.

use crate::world::WorldGenesis;
use goat_rng::{GoatRng, RngSource};

/// The "present" — backfilled seasons run in the years immediately before this.
pub const PRESENT_YEAR: u32 = 2025;
/// Number of past greats generated for the canon.
pub const NUM_GREATS: usize = 12;

const FIRST_NAMES: [&str; 16] = [
    "Marco", "Diego", "Luka", "Henrik", "Samuel", "Andre", "Paulo", "Viktor", "Tomas", "Rafael",
    "Karim", "Sergio", "Noah", "Mateus", "Felix", "Goran",
];
const LAST_NAMES: [&str; 16] = [
    "Costa",
    "Bauer",
    "Moreau",
    "Larsson",
    "Adeyemi",
    "Novak",
    "Ferreira",
    "Halonen",
    "Marchetti",
    "Okafor",
    "Vidic",
    "Romero",
    "Jensen",
    "Silva",
    "Andersson",
    "Petrov",
];

/// A backfilled past great. Owned data (not `&'static`) — generated, not handcrafted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricGreat {
    pub name: String,
    pub nationality: u8,
    /// First and last year of the player's top-level career.
    pub debut_year: u32,
    pub final_year: u32,
    pub peak_year: u32,
    pub peak_ovr: u8,
    /// Ballon d'Ors won across the backfill — the canon's headline accolade.
    pub ballon_dors: u32,
}

/// One backfilled season's canonical residue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeasonCanon {
    pub year: u32,
    /// Champion club id per division.
    pub champions: Vec<usize>,
    /// Index into `History::greats` of that year's Ballon d'Or winner.
    pub ballon_dor: usize,
}

/// The whole backfilled canon — pure-derivable from `world_seed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    pub seasons: Vec<SeasonCanon>,
    pub greats: Vec<HistoricGreat>,
}

fn great_seed(world_seed: u64, g: usize) -> u64 {
    world_seed ^ (g as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

fn make_name(rng: &mut impl RngSource) -> String {
    let f = FIRST_NAMES[rng.next_range_u32(0, 15) as usize];
    let l = LAST_NAMES[rng.next_range_u32(0, 15) as usize];
    format!("{f} {l}")
}

/// Deterministic display name from a player's seed (used for promoted/cohort players).
pub fn name_from_seed(seed: u64) -> String {
    make_name(&mut GoatRng::new(seed))
}

/// Backfill `num_seasons` of consistent past history from `world_seed`. Pure & deterministic.
pub fn backfill_history(world_seed: u64, num_seasons: u32, world: &WorldGenesis) -> History {
    let start_year = PRESENT_YEAR - num_seasons;
    let num_nations = world.nations.len() as u32;

    // 1. Generate the pool of greats, each with a career span inside the window.
    let mut greats: Vec<HistoricGreat> = Vec::with_capacity(NUM_GREATS);
    for g in 0..NUM_GREATS {
        let mut rng = GoatRng::new(great_seed(world_seed, g));
        let name = make_name(&mut rng);
        let nationality = rng.next_range_u32(0, num_nations.saturating_sub(1)) as u8;
        // Debut spread across the window; ~12–16 year careers, peak mid-career.
        let debut_year = start_year + rng.next_range_u32(0, num_seasons.saturating_sub(8).max(1));
        let career_len = rng.next_range_u32(12, 16);
        let final_year = (debut_year + career_len).min(PRESENT_YEAR - 1);
        let peak_year = debut_year + career_len / 2;
        let peak_ovr = rng.next_range_u32(82, 97) as u8;
        greats.push(HistoricGreat {
            name,
            nationality,
            debut_year,
            final_year,
            peak_year,
            peak_ovr,
            ballon_dors: 0,
        });
    }

    // 2. Resolve each season: champions per division + the Ballon d'Or winner.
    let mut seasons = Vec::with_capacity(num_seasons as usize);
    for year in start_year..PRESENT_YEAR {
        // Champions: strongest club per division + a per-(year,club) shake-up.
        let champions: Vec<usize> = world
            .leagues
            .iter()
            .map(|league| {
                *league
                    .clubs
                    .iter()
                    .max_by_key(|&&c| {
                        let mut rng = GoatRng::new(world_seed ^ (year as u64) ^ ((c as u64) << 16));
                        world.clubs[c].strength as i32 + rng.next_range_u32(0, 20) as i32
                    })
                    .unwrap()
            })
            .collect();

        // Ballon d'Or: among greats active this year, highest peak minus decline + noise.
        let mut best = 0usize;
        let mut best_score = i32::MIN;
        for (i, gr) in greats.iter().enumerate() {
            if year < gr.debut_year || year > gr.final_year {
                continue;
            }
            let decline = (year as i32 - gr.peak_year as i32).unsigned_abs() as i32 * 3;
            let mut rng = GoatRng::new(world_seed ^ (year as u64).rotate_left(7) ^ (i as u64));
            let score = gr.peak_ovr as i32 - decline + rng.next_range_u32(0, 8) as i32;
            if score > best_score {
                best_score = score;
                best = i;
            }
        }
        greats[best].ballon_dors += 1;

        seasons.push(SeasonCanon {
            year,
            champions,
            ballon_dor: best,
        });
    }

    History { seasons, greats }
}

impl History {
    /// The greats ranked by Ballon d'Ors (then peak OVR) — the canon the pantheon ranks.
    pub fn canon_ranked(&self) -> Vec<&HistoricGreat> {
        let mut v: Vec<&HistoricGreat> = self.greats.iter().collect();
        v.sort_by_key(|g| {
            (
                std::cmp::Reverse(g.ballon_dors),
                std::cmp::Reverse(g.peak_ovr),
            )
        });
        v
    }

    /// Deterministic fingerprint over the whole backfilled canon — the 9A.4 golden anchor.
    pub fn fingerprint(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let feed = |x: u64, h: &mut u64| {
            *h ^= x;
            *h = h.wrapping_mul(0x0000_0100_0000_01b3);
        };
        for s in &self.seasons {
            feed(s.year as u64, &mut h);
            feed(s.ballon_dor as u64, &mut h);
            for &c in &s.champions {
                feed(c as u64, &mut h);
            }
        }
        for g in &self.greats {
            feed(g.ballon_dors as u64, &mut h);
            feed(g.peak_ovr as u64, &mut h);
            feed(g.debut_year as u64, &mut h);
            feed(g.nationality as u64, &mut h);
        }
        h
    }
}

/// Nationality name for a historic great (for display / canon dump).
pub fn great_nation_name(nationality: u8, world: &WorldGenesis) -> &str {
    world.nation_name(nationality as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::WorldGenesis;

    #[test]
    fn history_is_deterministic() {
        let w7 = WorldGenesis::generate(7);
        assert_eq!(
            backfill_history(7, 30, &w7).fingerprint(),
            backfill_history(7, 30, &w7).fingerprint()
        );
        let w1 = WorldGenesis::generate(1);
        let w2 = WorldGenesis::generate(2);
        assert_ne!(
            backfill_history(1, 30, &w1).fingerprint(),
            backfill_history(2, 30, &w2).fingerprint()
        );
    }

    #[test]
    fn backfill_is_internally_consistent() {
        let world = WorldGenesis::generate(42);
        let h = backfill_history(42, 30, &world);
        assert_eq!(h.seasons.len(), 30);
        assert_eq!(h.greats.len(), NUM_GREATS);
        // Total Ballon d'Ors awarded == number of seasons (exactly one per year).
        let total_bd: u32 = h.greats.iter().map(|g| g.ballon_dors).sum();
        assert_eq!(total_bd, 30);
        // Every Ballon d'Or winner was active in the year he won.
        for s in &h.seasons {
            let g = &h.greats[s.ballon_dor];
            assert!(s.year >= g.debut_year && s.year <= g.final_year);
            // Champions are real clubs in their division.
            for (div, &champ) in s.champions.iter().enumerate() {
                assert!(world.leagues[div].clubs.contains(&champ));
            }
        }
        // The canon has a clear leader (someone won multiple awards over their arc).
        assert!(h.canon_ranked()[0].ballon_dors >= 2);
    }
}
