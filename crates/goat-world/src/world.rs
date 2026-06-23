//! Static mini-world data: 2 nations, 2 divisions each, 16 clubs per division.
//!
//! Club names and strengths are fixed. Player populations are derived from the
//! world seed in Phase 9. For Phase 5, clubs carry only aggregate strength.

use goat_fixed::Fixed;

/// Index into `CLUBS`.
pub type ClubId = usize;

/// Nation identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nation {
    England = 0,
    Brazil = 1,
}

impl Nation {
    pub fn name(self) -> &'static str {
        match self {
            Nation::England => "England",
            Nation::Brazil => "Brazil",
        }
    }
    pub fn from_idx(i: usize) -> Option<Self> {
        match i {
            0 => Some(Self::England),
            1 => Some(Self::Brazil),
            _ => None,
        }
    }
}

/// Division level within a nation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DivLevel {
    Top = 0,
    Second = 1,
}

/// A club in the mini-world.
#[derive(Debug, Clone, Copy)]
pub struct Club {
    pub id: ClubId,
    pub name: &'static str,
    pub nation: Nation,
    pub div_level: DivLevel,
    /// Aggregate team strength 1–99.
    pub strength: u8,
}

impl Club {
    /// Facilities development multiplier (stronger clubs invest more in youth).
    pub fn facilities_mult(self) -> Fixed {
        let pct = 700 + (self.strength as i32 - 50) * 10; // 0.700 at str 50, scales ±
        Fixed::raw(pct.clamp(500, 1_300))
    }
}

// ── Club data ─────────────────────────────────────────────────────────────────

pub const NUM_CLUBS: usize = 64;
pub const CLUBS_PER_DIV: usize = 16;
pub const NUM_DIVISIONS: usize = 4; // ENG_TOP, ENG_SEC, BRA_TOP, BRA_SEC

/// Division indices for use with `DIV_CLUBS`.
pub const DIV_ENG_TOP: usize = 0;
pub const DIV_ENG_SEC: usize = 1;
pub const DIV_BRA_TOP: usize = 2;
pub const DIV_BRA_SEC: usize = 3;

pub const DIV_NAMES: [&str; NUM_DIVISIONS] =
    ["Premier League", "Championship", "Série A", "Série B"];

pub const DIV_NATIONS: [Nation; NUM_DIVISIONS] = [
    Nation::England,
    Nation::England,
    Nation::Brazil,
    Nation::Brazil,
];

pub const DIV_LEVELS: [DivLevel; NUM_DIVISIONS] = [
    DivLevel::Top,
    DivLevel::Second,
    DivLevel::Top,
    DivLevel::Second,
];

/// Club IDs per division, in division rank order.
pub const DIV_CLUBS: [[ClubId; CLUBS_PER_DIV]; NUM_DIVISIONS] = [
    // ENG_TOP (0..15)
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    // ENG_SEC (16..31)
    [
        16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
    ],
    // BRA_TOP (32..47)
    [
        32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
    ],
    // BRA_SEC (48..63)
    [
        48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63,
    ],
];

macro_rules! club {
    ($id:expr, $name:expr, $nat:expr, $div:expr, $str:expr) => {
        Club {
            id: $id,
            name: $name,
            nation: $nat,
            div_level: $div,
            strength: $str,
        }
    };
}

pub const CLUBS: [Club; NUM_CLUBS] = [
    // ── England Premier (0–15) ────────────────────────────────────────────────
    club!(0, "Manchester City", Nation::England, DivLevel::Top, 90),
    club!(1, "Liverpool", Nation::England, DivLevel::Top, 88),
    club!(2, "Arsenal", Nation::England, DivLevel::Top, 85),
    club!(3, "Chelsea", Nation::England, DivLevel::Top, 83),
    club!(4, "Tottenham", Nation::England, DivLevel::Top, 82),
    club!(5, "Manchester United", Nation::England, DivLevel::Top, 80),
    club!(6, "Newcastle", Nation::England, DivLevel::Top, 76),
    club!(7, "Aston Villa", Nation::England, DivLevel::Top, 74),
    club!(8, "West Ham", Nation::England, DivLevel::Top, 72),
    club!(9, "Brighton", Nation::England, DivLevel::Top, 70),
    club!(10, "Brentford", Nation::England, DivLevel::Top, 68),
    club!(11, "Fulham", Nation::England, DivLevel::Top, 67),
    club!(12, "Crystal Palace", Nation::England, DivLevel::Top, 65),
    club!(13, "Everton", Nation::England, DivLevel::Top, 64),
    club!(14, "Wolves", Nation::England, DivLevel::Top, 63),
    club!(15, "Nottm Forest", Nation::England, DivLevel::Top, 62),
    // ── England Championship (16–31) ──────────────────────────────────────────
    club!(16, "Leeds United", Nation::England, DivLevel::Second, 58),
    club!(17, "Sunderland", Nation::England, DivLevel::Second, 56),
    club!(
        18,
        "Sheffield United",
        Nation::England,
        DivLevel::Second,
        55
    ),
    club!(19, "Burnley", Nation::England, DivLevel::Second, 54),
    club!(20, "Middlesbrough", Nation::England, DivLevel::Second, 53),
    club!(21, "Watford", Nation::England, DivLevel::Second, 52),
    club!(22, "Ipswich", Nation::England, DivLevel::Second, 51),
    club!(23, "Norwich", Nation::England, DivLevel::Second, 50),
    club!(24, "QPR", Nation::England, DivLevel::Second, 50),
    club!(25, "Stoke City", Nation::England, DivLevel::Second, 49),
    club!(26, "Hull City", Nation::England, DivLevel::Second, 48),
    club!(27, "Millwall", Nation::England, DivLevel::Second, 48),
    club!(28, "Coventry", Nation::England, DivLevel::Second, 47),
    club!(29, "Bristol City", Nation::England, DivLevel::Second, 47),
    club!(30, "Cardiff", Nation::England, DivLevel::Second, 46),
    club!(31, "Swansea", Nation::England, DivLevel::Second, 45),
    // ── Brazil Série A (32–47) ────────────────────────────────────────────────
    club!(32, "Flamengo", Nation::Brazil, DivLevel::Top, 82),
    club!(33, "Palmeiras", Nation::Brazil, DivLevel::Top, 80),
    club!(34, "Corinthians", Nation::Brazil, DivLevel::Top, 76),
    club!(35, "São Paulo", Nation::Brazil, DivLevel::Top, 74),
    club!(36, "Santos", Nation::Brazil, DivLevel::Top, 72),
    club!(37, "Cruzeiro", Nation::Brazil, DivLevel::Top, 70),
    club!(38, "Grêmio", Nation::Brazil, DivLevel::Top, 68),
    club!(39, "Internacional", Nation::Brazil, DivLevel::Top, 67),
    club!(40, "Athletico PR", Nation::Brazil, DivLevel::Top, 65),
    club!(41, "Fluminense", Nation::Brazil, DivLevel::Top, 64),
    club!(42, "Botafogo", Nation::Brazil, DivLevel::Top, 63),
    club!(43, "Atlético MG", Nation::Brazil, DivLevel::Top, 62),
    club!(44, "Bahia", Nation::Brazil, DivLevel::Top, 60),
    club!(45, "Fortaleza", Nation::Brazil, DivLevel::Top, 59),
    club!(46, "Vasco", Nation::Brazil, DivLevel::Top, 58),
    club!(47, "Cuiabá", Nation::Brazil, DivLevel::Top, 55),
    // ── Brazil Série B (48–63) ────────────────────────────────────────────────
    club!(48, "América MG", Nation::Brazil, DivLevel::Second, 48),
    club!(49, "Chapecoense", Nation::Brazil, DivLevel::Second, 46),
    club!(50, "Coritiba", Nation::Brazil, DivLevel::Second, 45),
    club!(51, "Goiás", Nation::Brazil, DivLevel::Second, 44),
    club!(52, "Guarani", Nation::Brazil, DivLevel::Second, 43),
    club!(53, "Ituano", Nation::Brazil, DivLevel::Second, 42),
    club!(54, "Londrina", Nation::Brazil, DivLevel::Second, 42),
    club!(55, "Mirassol", Nation::Brazil, DivLevel::Second, 41),
    club!(56, "Novorizontino", Nation::Brazil, DivLevel::Second, 41),
    club!(57, "Operário", Nation::Brazil, DivLevel::Second, 40),
    club!(58, "Ponte Preta", Nation::Brazil, DivLevel::Second, 39),
    club!(59, "Sport", Nation::Brazil, DivLevel::Second, 39),
    club!(60, "Tombense", Nation::Brazil, DivLevel::Second, 38),
    club!(61, "Avaí", Nation::Brazil, DivLevel::Second, 36),
    club!(62, "Sampaio Corrêa", Nation::Brazil, DivLevel::Second, 34),
    club!(63, "CSA", Nation::Brazil, DivLevel::Second, 32),
];

/// Return which division index a club belongs to.
pub fn club_division(club_id: ClubId) -> usize {
    for (div_idx, ids) in DIV_CLUBS.iter().enumerate() {
        if ids.contains(&club_id) {
            return div_idx;
        }
    }
    panic!("club_id {club_id} not in any division");
}

/// Return the index of a club within its division (0-based).
pub fn club_div_pos(club_id: ClubId) -> usize {
    let div = club_division(club_id);
    DIV_CLUBS[div].iter().position(|&c| c == club_id).unwrap()
}

/// Clubs available to a nation, both divisions, sorted by div then strength.
pub fn clubs_for_nation(nation: Nation) -> impl Iterator<Item = &'static Club> {
    CLUBS.iter().filter(move |c| c.nation == nation)
}
