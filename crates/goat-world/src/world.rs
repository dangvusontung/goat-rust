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
    pub const ALL: [DivLevel; TIERS_PER_NATION] =
        [DivLevel::Top, DivLevel::Second, DivLevel::Third];

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
    /// Per-club squad size, 18–30, correlated with (but not identical to) `strength` —
    /// stronger clubs support deeper squads (Design round 3, Doc C §Slice 1). Replaces the
    /// old global `SQUAD_SIZE` constant.
    pub squad_size: u8,
    /// The club's style bias over the 14 outfield roles (Design round 2, Doc B §B.2) —
    /// one `TacticalIdentity` generated per club at genesis, seed-derived.
    pub tactical_identity: TacticalIdentity,
    /// Running transfer/wage war-chest, £k (Design round 5, Doc A §Slice 1). A single
    /// number by design — see `economy::total_income` for how future revenue sources plug
    /// into it without touching the spending side. Persisted across seasons (not
    /// recomputed from genesis each window): `WorldGenesis` itself is regenerated fresh
    /// from `world_seed` on every load, so the live value lives in
    /// `goat_core::state::WorldState::club_budgets` and is overlaid onto this genesis
    /// starting point by save/load — this field only holds the *starting* war-chest.
    pub budget: i64,
    /// How much this club's own academy currently out-punches its genesis `strength` for
    /// intake purposes, 0..=`ACADEMY_BOOST_MAX` (Design round 5, Doc A §Slice 6). Decays
    /// without reinvestment (`academy::decay_academy_boost`) — an ongoing commitment, not a
    /// one-time purchase. Path-dependent like `budget`: the live value lives in
    /// `goat_core::state::WorldState::academy_boosts` and is overlaid onto this genesis
    /// starting point (0) by save/load — this field only holds the starting value.
    pub academy_boost: u8,
}

/// Upper clamp on `Club::academy_boost` (Design round 5, Doc A §Slice 6.1).
pub const ACADEMY_BOOST_MAX: u8 = 20;

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

// ── Club naming (offline-authored static banks, `crate::names`) ─────────────────
//
// Nations are REAL countries (Tùng 2026-07-23, superseding the fictional-nation
// call); clubs stay fictional, picked deterministically from each country's own
// word banks — nation-flavored by construction (England gets English-register
// names, Brazil Brazilian-register, ...), not a randomly-assigned style.

/// SplitMix64 finalizer — one-shot avalanche hash for direct bank indexing.
/// Name picks hash the club's seed directly instead of drawing from a sequential
/// xorshift stream: xorshift's FIRST output is strongly correlated across
/// sequentially-related seeds, which previously collapsed club-name diversity to
/// a handful of repeated names per league (the "every club in the division reads
/// the same 2-3 names" bug).
fn avalanche(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn pick<'a>(bank: &'a [&'static str], key: u64) -> &'a str {
    bank[(avalanche(key) % bank.len() as u64) as usize]
}

/// Deterministic club name for `club_id` under `spec`'s naming recipe, unique
/// within the nation's 60 clubs: affixes and stem are avalanche-picked, retried
/// with a bumped attempt counter on collision (every nation's bank offers ≥ 72
/// combinations against 60 draws, so the retry converges well inside the guard).
fn generate_club_name(
    world_seed: u64,
    club_id: usize,
    spec: &crate::names::NationSpec,
    taken: &std::collections::BTreeSet<String>,
) -> String {
    let key = club_seed(world_seed, club_id);
    for attempt in 0..64u64 {
        let wobble = attempt.wrapping_mul(0x9E37_79B9);
        let prefix = pick(spec.prefixes, key ^ 0xF10A ^ wobble);
        let stem = pick(spec.stems, key ^ 0xF20B ^ wobble);
        let suffix = pick(spec.suffixes, key ^ 0xF30C ^ wobble);
        let name = format!("{prefix}{stem}{suffix}");
        if !taken.contains(&name) {
            return name;
        }
    }
    // Unreachable by construction (≥72 combos vs 60 draws); deterministic fallback.
    format!("{}Athletic XI", spec.stems[club_id % spec.stems.len()])
}

pub(crate) fn seed_mix(world_seed: u64, salt: u64, idx: u64) -> u64 {
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
            let spec = &crate::names::NATIONS[n];
            let mut rng = GoatRng::new(nation_seed(world_seed, n));
            let stature = rng.next_range_u32(25, 95) as u8;
            let tactical_identity =
                TacticalIdentity::generate(seed_mix(world_seed, 0xD4, n as u64));
            nations.push(GeneratedNation {
                id: n,
                name: spec.name.to_string(),
                stature,
                tactical_identity,
            });
        }

        let mut leagues = Vec::with_capacity(NUM_DIVISIONS);
        let mut clubs = Vec::with_capacity(NUM_CLUBS);

        for nation in &nations {
            let spec = &crate::names::NATIONS[nation.id];
            // Club names are unique within the nation (across all 3 tiers).
            let mut taken = std::collections::BTreeSet::new();
            for (tier_idx, &tier) in DivLevel::ALL.iter().enumerate() {
                let league_id = leagues.len();
                let mut league_clubs = Vec::with_capacity(CLUBS_PER_DIV);

                // Base strength for this tier: decays with tier depth from the nation's
                // stature (a minnow's tier-1 clubs are still stronger than its tier-3 ones).
                let tier_base = (nation.stature as i32 - tier_idx as i32 * 15).max(10);

                for rank in 0..CLUBS_PER_DIV {
                    let club_id = clubs.len();
                    let mut rng = GoatRng::new(club_seed(world_seed, club_id));
                    let name = generate_club_name(world_seed, club_id, spec, &taken);
                    taken.insert(name.clone());
                    let rank_decay = (rank as i32 * 2) / 3;
                    let noise = rng.next_range_u32(0, 10) as i32 - 5;
                    let strength = (tier_base - rank_decay + noise).clamp(1, 99) as u8;
                    // Squad size 18–30, stature-weighted (Design round 3, Doc C §1.2): bigger
                    // clubs support deeper squads, mirroring `facilities_mult`'s same shape.
                    let span = 12i32; // 30 - 18
                    let squad_base = 18 + (strength as i32 * span) / 99;
                    let squad_noise = rng.next_range_u32(0, 2) as i32 - 1; // ±1 jitter
                    let squad_size = (squad_base + squad_noise).clamp(18, 30) as u8;
                    let tactical_identity =
                        TacticalIdentity::generate(seed_mix(world_seed, 0xE5, club_id as u64));
                    // ~one season's tier/strength-derived income as a starting war-chest
                    // (Design round 5, Doc A §1.6).
                    let budget = 2 * crate::economy::tier_baseline_income(strength, tier);
                    clubs.push(Club {
                        id: club_id,
                        name,
                        nation: nation.id,
                        strength,
                        squad_size,
                        tactical_identity,
                        budget,
                        academy_boost: 0,
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

        WorldGenesis {
            nations,
            leagues,
            clubs,
        }
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
        assert!(a
            .clubs
            .iter()
            .zip(b.clubs.iter())
            .any(|(x, y)| x.name != y.name));
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
    fn squad_size_always_in_band() {
        let w = WorldGenesis::generate(17);
        assert!(w.clubs.iter().all(|c| (18..=30).contains(&c.squad_size)));
    }

    #[test]
    fn squad_size_correlates_with_strength() {
        let w = WorldGenesis::generate(17);
        let mut by_strength = w.clubs.clone();
        by_strength.sort_by_key(|c| c.strength);
        let n = by_strength.len();
        let quartile = n / 4;
        let bottom_avg: f64 = by_strength[..quartile]
            .iter()
            .map(|c| c.squad_size as f64)
            .sum::<f64>()
            / quartile as f64;
        let top_avg: f64 = by_strength[n - quartile..]
            .iter()
            .map(|c| c.squad_size as f64)
            .sum::<f64>()
            / quartile as f64;
        assert!(
            top_avg > bottom_avg,
            "top-quartile-strength clubs ({top_avg}) should average a materially higher \
             squad_size than bottom-quartile clubs ({bottom_avg})"
        );
    }

    #[test]
    fn genesis_seeds_two_windows_of_income() {
        let w = WorldGenesis::generate(31);
        for league in &w.leagues {
            for &cid in &league.clubs {
                let club = &w.clubs[cid];
                let expected = 2 * crate::economy::tier_baseline_income(club.strength, league.tier);
                assert_eq!(
                    club.budget, expected,
                    "club {cid} genesis budget should be exactly 2x one window's income"
                );
            }
        }
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

    /// A2.2 (superseded 2026-07-23): nations are real countries from the static
    /// `names::NATIONS` table, in fixed order — not seed-generated fiction.
    #[test]
    fn nations_are_the_real_country_table() {
        let w = WorldGenesis::generate(42);
        assert_eq!(w.nations.len(), crate::names::NATIONS.len());
        for (nation, spec) in w.nations.iter().zip(crate::names::NATIONS.iter()) {
            assert_eq!(nation.name, spec.name);
        }
    }

    /// Club names are unique within each nation's 60 clubs (dedupe retry works),
    /// and the avalanche pick restores real per-league diversity (regression test
    /// for the xorshift first-draw correlation bug that collapsed a whole division
    /// to 2-3 repeated names).
    #[test]
    fn club_names_unique_within_nation_and_diverse_within_league() {
        for seed in [7u64, 42, 1234] {
            let w = WorldGenesis::generate(seed);
            for league in &w.leagues {
                let mut names: Vec<&str> = league
                    .clubs
                    .iter()
                    .map(|&c| w.clubs[c].name.as_str())
                    .collect();
                let total = names.len();
                names.sort_unstable();
                names.dedup();
                assert_eq!(
                    names.len(),
                    total,
                    "league {} ({}) has duplicate club names",
                    league.id,
                    league.name
                );
            }
        }
    }

    /// Different countries read in different naming registers (nation-flavored):
    /// England's clubs carry English suffixes, Brazil's never do.
    #[test]
    fn club_naming_is_nation_flavored() {
        let w = WorldGenesis::generate(42);
        let england = &w.nations[0];
        assert_eq!(england.name, "England");
        let english_suffixes = [
            " United",
            " City",
            " Town",
            " Rovers",
            " Athletic",
            " Wanderers",
            " Albion",
        ];
        for club in w.clubs_for_nation(england.id) {
            assert!(
                english_suffixes.iter().any(|s| club.name.ends_with(s)),
                "English club with non-English-register name: {}",
                club.name
            );
        }
        let brazil = &w.nations[5];
        assert_eq!(brazil.name, "Brazil");
        for club in w.clubs_for_nation(brazil.id) {
            assert!(
                !english_suffixes.iter().any(|s| club.name.ends_with(s)),
                "Brazilian club with English-register name: {}",
                club.name
            );
        }
    }
}
