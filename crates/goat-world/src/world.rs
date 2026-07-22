//! Generated world data: ~20 nations, 3 tiers each, 20 clubs per tier — all seed-derived.
//!
//! Nation/club/league identity is generated once per `world_seed` at genesis time and held
//! as a `WorldGenesis` value for the session (never a compile-time const, never persisted —
//! recomputed from `world_seed` on load, same "seed is the universe" pattern as `History`).

use goat_core::tactical_identity::TacticalIdentity;
use goat_fixed::Fixed;
use goat_rng::{GoatRng, RngSource};

/// Index into `WorldGenesis::clubs`.
pub type ClubId = usize;
/// Index into `WorldGenesis::nations`.
pub type NationId = usize;
/// Index into `WorldGenesis::leagues`.
pub type LeagueId = usize;

/// Division tier within a nation. Every nation gets exactly 3 tiers this round
/// (uniform-depth pyramids — variable pyramid depth is a flagged future enhancement).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DivLevel {
    Top = 0,
    Second = 1,
    Third = 2,
}

impl DivLevel {
    pub const ALL: [DivLevel; TIERS_PER_NATION] = [DivLevel::Top, DivLevel::Second, DivLevel::Third];

    pub fn from_idx(i: usize) -> Self {
        match i {
            0 => DivLevel::Top,
            1 => DivLevel::Second,
            _ => DivLevel::Third,
        }
    }
}

/// A generated nation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedNation {
    pub id: NationId,
    pub name: String,
    /// Quality band 1–99 — shifts the mean/spread of this nation's generated club
    /// strengths (a powerhouse nation's floor is respectable; a minnow's ceiling is
    /// capped), preserving bible §4.1's powerhouse↔minnow dial once real club strength
    /// numbers are no longer hand-tuned per nation.
    pub stature: u8,
    /// The national team's style bias over the 14 outfield roles (Design round 2, Doc B
    /// §B.2) — doubles as "the national team" for tactical-fit purposes; no separate
    /// roster is persisted (call-ups are resolved fresh each international window from
    /// the eligible-by-nationality population, per B.2's recommendation).
    pub tactical_identity: TacticalIdentity,
}

/// A club in the generated world. Owned data (not `&'static`) — generated, not handcrafted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Club {
    pub id: ClubId,
    pub name: String,
    pub nation: NationId,
    /// Aggregate team strength 1–99.
    pub strength: u8,
    /// The club's style bias over the 14 outfield roles (Design round 2, Doc B §B.2) —
    /// one `TacticalIdentity` generated per club at genesis, seed-derived.
    pub tactical_identity: TacticalIdentity,
}

impl Club {
    /// Facilities development multiplier (stronger clubs invest more in youth).
    pub fn facilities_mult(&self) -> Fixed {
        let pct = 700 + (self.strength as i32 - 50) * 10; // 0.700 at str 50, scales ±
        Fixed::raw(pct.clamp(500, 1_300))
    }
}

/// One division: a specific tier within a specific nation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct League {
    pub id: LeagueId,
    pub nation: NationId,
    pub tier: DivLevel,
    pub name: String,
    /// Club ids currently in this league, in table order. `Vec`, not a fixed array —
    /// promotion/relegation mutates membership season to season (a club is removed from
    /// one league's `clubs` and inserted into another's).
    pub clubs: Vec<ClubId>,
    /// Per-league capacity. Held as data (not baked into the type) so a future round can
    /// give different league shapes (conferences, smaller domestic leagues, etc.) without
    /// a new type per shape. This round: uniformly 20 for every league.
    pub max_clubs: u8,
}

// ── Confirmed scale numbers (2026-07-22) ────────────────────────────────────────
pub const NUM_NATIONS: usize = 20;
pub const TIERS_PER_NATION: usize = 3;
/// Clubs per tier — uniform across every league this round (confirmed by Tùng: 20, not 16).
pub const CLUBS_PER_DIV: usize = 20;
pub const NUM_DIVISIONS: usize = NUM_NATIONS * TIERS_PER_NATION; // 60
pub const NUM_CLUBS: usize = NUM_DIVISIONS * CLUBS_PER_DIV; // 1,200
/// Promotion/relegation cut size (top-N up, bottom-N down) — Design's recommended N=3.
pub const PROMO_RELEGATION_N: usize = 3;

// ── Word banks (hand-authored fictional fragments; no real place/club names) ────

/// Fictional country-name fragments, combined prefix+suffix — a large hand-authored bank
/// (per A2.2's recommendation) rather than a syllable-combinator, to avoid repetitive
/// invented names at 20-nation scale.
const NATION_PREFIX: [&str; 24] = [
    "Vestra", "Kordo", "Ambar", "Solen", "Thurin", "Mirova", "Casta", "Nordar", "Velen", "Ostro",
    "Brenna", "Ithor", "Marveld", "Corvan", "Sundal", "Perano", "Adrasi", "Wintra", "Golveth",
    "Halmar", "Rosvik", "Tamora", "Delvan", "Estria",
];
const NATION_SUFFIX: [&str; 20] = [
    "via", "land", "stan", "aro", "mark", "gard", "sia", "burg", "nesia", "vale", "dor", "wick",
    "moor", "heim", "rica", "thia", "ford", "wen", "grad", "ain",
];

/// Fictional place-name stems for club naming, English-flavored register (Style A) —
/// combined with `CLUB_SUFFIX_A` (e.g. "Ashford United").
const CITY_STEM: [&str; 24] = [
    "Ashford", "Brackwell", "Solmoor", "Kingsmere", "Redcliffe", "Hallowgate", "Wynstead",
    "Marlow", "Thornbury", "Oakhaven", "Cresthill", "Fenwick", "Draymoor", "Hartfield",
    "Ellsworth", "Stonebridge", "Greymarsh", "Ravensdale", "Wintershaw", "Foxleigh", "Braemoor",
    "Duskvale", "Aldermere", "Wrenford",
];
const CLUB_SUFFIX_A: [&str; 8] = [
    "United", "Athletic", "Rovers", "Town", "City", "Wanderers", "Albion", "Rangers",
];

/// Single-word invented club identities, South-American-flavored register (Style B) — mirrors
/// how the old hardcoded data used single-word Brazilian club names (Flamengo, Corinthians).
const CLUB_WORD_B: [&str; 32] = [
    "Volcanza", "Marejada", "Estrelar", "Cruzeta", "Fluvente", "Andaria", "Pampero", "Litorena",
    "Tropicó", "Cordillar", "Solaço", "Barranca", "Vermelhas", "Selvana", "Riacho", "Tucanaço",
    "Cerraço", "Guaranti", "Aurinegro", "Costeira", "Manguera", "Pantana", "Sertana", "Bravante",
    "Corrente", "Marisco", "Delfina", "Ipanera", "Tijuqua", "Sambara", "Coralina", "Litorio",
];

fn seed_mix(world_seed: u64, salt: u64, idx: u64) -> u64 {
    world_seed
        ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (idx.rotate_left(23)).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
}

fn nation_seed(world_seed: u64, n: usize) -> u64 {
    seed_mix(world_seed, 0xA1, n as u64)
}

fn club_seed(world_seed: u64, c: usize) -> u64 {
    seed_mix(world_seed, 0xB2, c as u64)
}

/// Style A (English-flavored "Stem + Suffix") vs Style B (South-American-flavored single
/// invented word) — chosen once per nation from its own seed, giving the world a mix of
/// naming registers rather than one homogenized bank (preserves the "this reads as a
/// distinct footballing culture" flavor the old hardcoded England/Brazil split had).
fn nation_naming_style(world_seed: u64, nation_id: NationId) -> u8 {
    let mut rng = GoatRng::new(seed_mix(world_seed, 0xC3, nation_id as u64));
    rng.next_range_u32(0, 1) as u8
}

fn generate_nation_name(rng: &mut impl RngSource) -> String {
    let p = NATION_PREFIX[rng.next_range_u32(0, NATION_PREFIX.len() as u32 - 1) as usize];
    let s = NATION_SUFFIX[rng.next_range_u32(0, NATION_SUFFIX.len() as u32 - 1) as usize];
    format!("{p}{s}")
}

fn generate_club_name(rng: &mut impl RngSource, style: u8) -> String {
    if style == 0 {
        let stem = CITY_STEM[rng.next_range_u32(0, CITY_STEM.len() as u32 - 1) as usize];
        let suf = CLUB_SUFFIX_A[rng.next_range_u32(0, CLUB_SUFFIX_A.len() as u32 - 1) as usize];
        format!("{stem} {suf}")
    } else {
        CLUB_WORD_B[rng.next_range_u32(0, CLUB_WORD_B.len() as u32 - 1) as usize].to_string()
    }
}

fn league_name(nation_name: &str, tier: DivLevel) -> String {
    match tier {
        DivLevel::Top => format!("{nation_name} Premier League"),
        DivLevel::Second => format!("{nation_name} Division Two"),
        DivLevel::Third => format!("{nation_name} Division Three"),
    }
}

/// The whole generated world — nations, leagues, clubs. Built once per `world_seed` at
/// genesis time (or on save load, replaying the same pure function), held for the session.
/// Not persisted directly: `SaveData` stores `world_seed` and the PC's club/nation *index*,
/// and this whole structure is regenerated deterministically from that seed on load —
/// exactly like `History` already is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldGenesis {
    pub nations: Vec<GeneratedNation>,
    pub leagues: Vec<League>,
    pub clubs: Vec<Club>,
}

impl WorldGenesis {
    /// Generate the whole world deterministically from `world_seed`. Pure & reproducible.
    pub fn generate(world_seed: u64) -> Self {
        let mut nations = Vec::with_capacity(NUM_NATIONS);
        for n in 0..NUM_NATIONS {
            let mut rng = GoatRng::new(nation_seed(world_seed, n));
            // Retry-on-collision keeps the 20 nation names distinct (cheap: 24×20 = 480
            // possible combinations against only 20 draws).
            let mut name = generate_nation_name(&mut rng);
            let mut guard = 0;
            while nations.iter().any(|existing: &GeneratedNation| existing.name == name) && guard < 32 {
                name = generate_nation_name(&mut rng);
                guard += 1;
            }
            let stature = rng.next_range_u32(25, 95) as u8;
            let tactical_identity = TacticalIdentity::generate(seed_mix(world_seed, 0xD4, n as u64));
            nations.push(GeneratedNation {
                id: n,
                name,
                stature,
                tactical_identity,
            });
        }

        let mut leagues = Vec::with_capacity(NUM_DIVISIONS);
        let mut clubs = Vec::with_capacity(NUM_CLUBS);

        for nation in &nations {
            let style = nation_naming_style(world_seed, nation.id);
            for (tier_idx, &tier) in DivLevel::ALL.iter().enumerate() {
                let league_id = leagues.len();
                let mut league_clubs = Vec::with_capacity(CLUBS_PER_DIV);

                // Base strength for this tier: decays with tier depth from the nation's
                // stature (a minnow's tier-1 clubs are still stronger than its tier-3 ones).
                let tier_base = (nation.stature as i32 - tier_idx as i32 * 15).max(10);

                for rank in 0..CLUBS_PER_DIV {
                    let club_id = clubs.len();
                    let mut rng = GoatRng::new(club_seed(world_seed, club_id));
                    let name = generate_club_name(&mut rng, style);
                    let rank_decay = (rank as i32 * 2) / 3;
                    let noise = rng.next_range_u32(0, 10) as i32 - 5;
                    let strength = (tier_base - rank_decay + noise).clamp(1, 99) as u8;
                    let tactical_identity =
                        TacticalIdentity::generate(seed_mix(world_seed, 0xE5, club_id as u64));
                    clubs.push(Club {
                        id: club_id,
                        name,
                        nation: nation.id,
                        strength,
                        tactical_identity,
                    });
                    league_clubs.push(club_id);
                }

                leagues.push(League {
                    id: league_id,
                    nation: nation.id,
                    tier,
                    name: league_name(&nation.name, tier),
                    clubs: league_clubs,
                    max_clubs: CLUBS_PER_DIV as u8,
                });
            }
        }

        WorldGenesis { nations, leagues, clubs }
    }

    pub fn nation_name(&self, id: NationId) -> &str {
        self.nations
            .get(id)
            .map(|n| n.name.as_str())
            .unwrap_or("Unknown")
    }

    /// Return which league a club currently belongs to (scans membership — cheap at
    /// ~1,200 clubs / 60 leagues, same "iterate, don't hardcode indices" shape the old
    /// `club_division` had).
    pub fn club_league(&self, club_id: ClubId) -> LeagueId {
        for league in &self.leagues {
            if league.clubs.contains(&club_id) {
                return league.id;
            }
        }
        panic!("club_id {club_id} not in any league");
    }

    /// Return the index of a club within its league's table order (0-based).
    pub fn club_league_pos(&self, club_id: ClubId) -> usize {
        let league = self.club_league(club_id);
        self.leagues[league]
            .clubs
            .iter()
            .position(|&c| c == club_id)
            .unwrap()
    }

    /// Clubs belonging to a nation, across all its tiers.
    pub fn clubs_for_nation(&self, nation: NationId) -> impl Iterator<Item = &Club> {
        self.clubs.iter().filter(move |c| c.nation == nation)
    }

    /// Genesis-static per-league membership (`league_clubs[id]` == `leagues[id].clubs`),
    /// for callers that don't need promotion/relegation-advanced membership (e.g. flavor
    /// screens that replay the cohort without needing real league-drift accuracy).
    pub fn static_league_clubs(&self) -> Vec<Vec<ClubId>> {
        self.leagues.iter().map(|l| l.clubs.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn club_names_deterministic_per_seed() {
        let a = WorldGenesis::generate(7);
        let b = WorldGenesis::generate(7);
        assert_eq!(a.clubs, b.clubs);
    }

    #[test]
    fn club_names_differ_across_seeds() {
        let a = WorldGenesis::generate(1);
        let b = WorldGenesis::generate(2);
        assert!(a.clubs.iter().zip(b.clubs.iter()).any(|(x, y)| x.name != y.name));
    }

    #[test]
    fn world_deterministic_per_seed() {
        let a = WorldGenesis::generate(42);
        let b = WorldGenesis::generate(42);
        assert_eq!(a, b);
    }

    #[test]
    fn world_differs_across_seeds() {
        let a = WorldGenesis::generate(1);
        let b = WorldGenesis::generate(2);
        assert_ne!(a.nations, b.nations);
    }

    #[test]
    fn world_matches_agreed_shape() {
        let w = WorldGenesis::generate(99);
        assert_eq!(w.nations.len(), NUM_NATIONS);
        assert_eq!(w.leagues.len(), NUM_DIVISIONS);
        assert_eq!(w.clubs.len(), NUM_CLUBS);
        for league in &w.leagues {
            assert_eq!(league.clubs.len(), CLUBS_PER_DIV);
            assert_eq!(league.max_clubs as usize, CLUBS_PER_DIV);
        }
        // Every club is in exactly the nation its league says it belongs to.
        for league in &w.leagues {
            for &cid in &league.clubs {
                assert_eq!(w.clubs[cid].nation, league.nation);
            }
        }
    }

    #[test]
    fn nation_names_are_distinct() {
        let w = WorldGenesis::generate(123);
        let mut names: Vec<&str> = w.nations.iter().map(|n| n.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), NUM_NATIONS, "nation names must be unique");
    }

    #[test]
    fn club_league_lookup_is_consistent() {
        let w = WorldGenesis::generate(5);
        for league in &w.leagues {
            for &cid in &league.clubs {
                assert_eq!(w.club_league(cid), league.id);
            }
        }
    }
}
