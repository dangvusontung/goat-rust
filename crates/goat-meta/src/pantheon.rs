//! The four schools of thought and the seeded canon of past greats.
//!
//! Schools deliberately disagree: a career that delights the Trophy Cabinet
//! might leave the Stats Purists cold. Divergence is a feature, not a bug.

use goat_fixed::Fixed;

use crate::legacy::LegacyAxes;

// ── Schools ───────────────────────────────────────────────────────────────────

/// One of four competing frameworks for evaluating greatness.
#[derive(Debug, Clone, Copy)]
pub struct School {
    pub name: &'static str,
    pub tagline: &'static str,
    /// Percentage weights for each axis (must sum to 100).
    /// Order: [winning, accolades, output, longevity, decisive, loyalty, icon, h2h]
    pub weights: [i32; 8],
}

impl School {
    /// Compute a 0–100 score for the given axes under this school's weighting.
    pub fn score(&self, axes: &LegacyAxes) -> Fixed {
        let arr = axes.as_array();
        let weighted: i32 = self
            .weights
            .iter()
            .zip(arr.iter())
            .map(|(&w, ax)| w * ax.to_int())
            .sum::<i32>()
            / 100;
        Fixed::from_int(weighted.clamp(0, 100))
    }
}

pub const NUM_SCHOOLS: usize = 4;

pub const SCHOOLS: [School; NUM_SCHOOLS] = [
    School {
        name: "The Trophy Cabinet",
        tagline: "Winners fill stadiums. Nothing else matters.",
        weights: [35, 15, 10, 5, 20, 5, 5, 5],
    },
    School {
        name: "The Eye-Test Romantics",
        tagline: "One transcendent moment outlasts a decade of solid passing.",
        weights: [10, 20, 15, 5, 35, 5, 7, 3],
    },
    School {
        name: "The Stats Purists",
        tagline: "Over a thousand games the truth comes out. Always.",
        weights: [10, 15, 40, 20, 5, 5, 3, 2],
    },
    School {
        name: "The Loyalty Traditionalists",
        tagline: "A one-club legend is priceless. No fee in the world changes that.",
        weights: [15, 10, 10, 15, 10, 35, 3, 2],
    },
];

// ── Canon: past greats ────────────────────────────────────────────────────────

/// A seeded past great in the pantheon.
#[derive(Debug, Clone, Copy)]
pub struct PastGreat {
    pub name: &'static str,
    pub era: &'static str,
    pub nationality: &'static str,
    pub axes: LegacyAxes,
}

macro_rules! great {
    ($name:expr, $era:expr, $nat:expr,
     $win:expr, $acc:expr, $out:expr, $lon:expr, $dec:expr, $loy:expr) => {
        PastGreat {
            name: $name,
            era: $era,
            nationality: $nat,
            axes: LegacyAxes {
                winning: Fixed::from_int($win),
                accolades: Fixed::from_int($acc),
                output: Fixed::from_int($out),
                longevity: Fixed::from_int($lon),
                decisive: Fixed::from_int($dec),
                loyalty: Fixed::from_int($loy),
                icon: LegacyAxes::STUB_50,
                head_to_head: LegacyAxes::STUB_50,
            },
        }
    };
}

pub const NUM_CANON: usize = 10;

/// The seeded canon of past greats. Handcrafted to ensure realistic school disagreement.
///
/// Deliberately diverse profiles — some all-rounders, some controversial:
/// - "Dominguez" is a trophy king with low output
/// - "Keane" is a stats god who won little
/// - "Pires" won nothing but has max decisive moments
/// - "Andersen" is the perfect loyalty icon
pub const CANON: [PastGreat; NUM_CANON] = [
    // All-time consensus greats
    great!(
        "Cavalcanti",
        "2000s–2015",
        "Brazilian",
        90,
        85,
        88,
        85,
        75,
        60
    ),
    great!(
        "Van der Berg",
        "1995s–2010",
        "Dutch",
        80,
        75,
        82,
        90,
        65,
        85
    ),
    // Trophy machines, moderate output
    great!("Dominguez", "2005–2018", "Spanish", 95, 80, 65, 70, 70, 35),
    great!("Petrov", "2003–2016", "Russian", 75, 60, 70, 80, 50, 75),
    // Stats gods, light on trophies
    great!("Keane", "1998–2014", "Irish", 30, 35, 95, 95, 40, 90),
    great!("Nakamura", "2002–2017", "Japanese", 20, 30, 90, 90, 35, 95),
    // Decisive-moment legends
    great!("Pires", "1999–2012", "French", 25, 45, 75, 70, 100, 40),
    great!("Muñoz", "2007–2020", "Argentine", 50, 55, 80, 65, 90, 45),
    // Loyalty icons
    great!("Andersen", "2001–2016", "Danish", 40, 30, 70, 85, 45, 100),
    great!("Okonkwo", "2004–2019", "Nigerian", 45, 40, 75, 80, 55, 95),
];

// ── Ranking ───────────────────────────────────────────────────────────────────

/// Rank the PC among the canon under a given school (1-based, lower = better).
///
/// Returns (pc_rank, total_in_canon+1) where total = NUM_CANON + 1 (PC included).
pub fn rank_in_canon(pc_axes: &LegacyAxes, school_idx: usize) -> (usize, usize) {
    let school = &SCHOOLS[school_idx];
    let pc_score = school.score(pc_axes);
    let rank = 1 + CANON
        .iter()
        .filter(|g| school.score(&g.axes) > pc_score)
        .count();
    (rank, NUM_CANON + 1)
}

/// Score comparisons for all schools. Returns [(pc_score, rank); NUM_SCHOOLS].
pub fn all_rankings(pc_axes: &LegacyAxes) -> [(Fixed, usize, usize); NUM_SCHOOLS] {
    let mut result = [(Fixed::ZERO, 0usize, 0usize); NUM_SCHOOLS];
    for (i, entry) in result.iter_mut().enumerate() {
        let score = SCHOOLS[i].score(pc_axes);
        let (rank, total) = rank_in_canon(pc_axes, i);
        *entry = (score, rank, total);
    }
    result
}
